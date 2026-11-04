//! Process-local queue occupancy state; run and submission records are added
//! with the admission operations that begin using them.

use crate::backend_config::QueueLimits;
use crate::ids::new_uuid_v4;
use crate::replay_types::{ReplayAcknowledgement, ReplayFailure, ReplayReport, ReplayStatus};
use factorsbt_contracts::{
    outcomes::{
        RunFailure, RunLifecycle, RunProgress, RunResult, RunStatus, SubmissionAccepted,
        SubmissionStatus,
    },
    run_configuration::RunConfiguration,
    UtcTimestamp, Uuid,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct RunStore {
    limits: QueueLimits,
    state: Arc<Mutex<QueueState>>,
    ready: Arc<Notify>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JobId {
    Run(Uuid),
    Replay(Uuid),
}

#[derive(Debug)]
struct QueueState {
    accepting: bool,
    queued: VecDeque<JobId>,
    starting: VecDeque<JobId>,
    active: HashSet<JobId>,
    submissions: HashMap<Uuid, StoredSubmission>,
    runs: HashMap<Uuid, StoredRun>,
    replays: HashMap<Uuid, StoredReplay>,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            accepting: true,
            queued: VecDeque::new(),
            starting: VecDeque::new(),
            active: HashSet::new(),
            submissions: HashMap::new(),
            runs: HashMap::new(),
            replays: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredSubmission {
    pub configuration: RunConfiguration,
    pub acknowledgement: SubmissionAccepted,
}

#[derive(Debug, Clone)]
pub struct StoredRun {
    pub submission_id: Uuid,
    pub configuration: RunConfiguration,
    pub status: RunStatus,
    pub result: Option<RunResult>,
}

#[derive(Debug, Clone)]
pub struct StoredReplay {
    pub status: ReplayStatus,
    pub report: Option<ReplayReport>,
}

#[derive(Debug, Clone)]
pub struct ReplaySubmissionOutcome {
    pub acknowledgement: ReplayAcknowledgement,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionOutcome {
    pub acknowledgement: SubmissionAccepted,
    pub created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionStoreError {
    SubmissionIdReused,
    QueueFull,
    BackendUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStoreTransitionError {
    NotFound,
    InvalidTransition,
    ResultRunMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResultLookupError {
    NotFound,
    NotSucceeded,
    Failed,
    ResultUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueSnapshot {
    pub active_runs: usize,
    pub queued_runs: usize,
    pub limits: QueueLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreRecordCounts {
    pub submissions: usize,
    pub runs: usize,
}

impl RunStore {
    pub fn new(limits: QueueLimits) -> Self {
        Self {
            limits,
            state: Arc::new(Mutex::new(QueueState::default())),
            ready: Arc::new(Notify::new()),
        }
    }

    pub fn limits(&self) -> QueueLimits {
        self.limits
    }

    pub fn queue_snapshot(&self) -> QueueSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        QueueSnapshot {
            active_runs: state.active.len(),
            queued_runs: state.queued.len(),
            limits: self.limits,
        }
    }

    pub fn record_counts(&self) -> StoreRecordCounts {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        StoreRecordCounts {
            submissions: state.submissions.len(),
            runs: state.runs.len(),
        }
    }

    pub fn get_submission(&self, submission_id: &Uuid) -> Option<SubmissionStatus> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let submission = state.submissions.get(submission_id)?;
        let run = state.runs.get(&submission.acknowledgement.run_id)?;
        Some(SubmissionStatus {
            submission_id: submission.acknowledgement.submission_id.clone(),
            run_id: submission.acknowledgement.run_id.clone(),
            status: run.status.status,
            submitted_at: submission.acknowledgement.submitted_at.clone(),
        })
    }

    pub fn get_run(&self, run_id: &Uuid) -> Option<StoredRun> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .runs
            .get(run_id)
            .cloned()
    }

    pub fn get_replay(&self, replay_id: &Uuid) -> Option<StoredReplay> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .replays
            .get(replay_id)
            .cloned()
    }

    /// The idempotency key is also the replay ID, allowing lookup after an
    /// uncertain POST acknowledgement without adding a separate submission API.
    pub fn accept_replay(
        &self,
        replay_id: Uuid,
        source_run_id: Uuid,
        submitted_at: UtcTimestamp,
    ) -> Result<ReplaySubmissionOutcome, SubmissionStoreError> {
        let mut notify_count = 0;
        let outcome = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(existing) = state.replays.get(&replay_id) {
                return if existing.status.source_run_id == source_run_id {
                    Ok(ReplaySubmissionOutcome {
                        acknowledgement: replay_acknowledgement(&existing.status),
                        created: false,
                    })
                } else {
                    Err(SubmissionStoreError::SubmissionIdReused)
                };
            }
            if !state.accepting {
                return Err(SubmissionStoreError::BackendUnavailable);
            }
            notify_count += promote_waiting_runs(&mut state, self.limits);
            if state.active.len() < self.limits.max_active_runs {
                let job = JobId::Replay(replay_id.clone());
                state.active.insert(job.clone());
                state.starting.push_back(job);
                notify_count += 1;
            } else if state.queued.len() < self.limits.max_queued_runs {
                state.queued.push_back(JobId::Replay(replay_id.clone()));
            } else {
                return Err(SubmissionStoreError::QueueFull);
            }
            let status = ReplayStatus {
                contract_version: "1.0.0".to_owned(),
                replay_id: replay_id.clone(),
                source_run_id,
                status: "queued".to_owned(),
                submitted_at,
                started_at: None,
                finished_at: None,
                failure: None,
            };
            let acknowledgement = replay_acknowledgement(&status);
            state.replays.insert(
                replay_id,
                StoredReplay {
                    status,
                    report: None,
                },
            );
            ReplaySubmissionOutcome {
                acknowledgement,
                created: true,
            }
        };
        self.notify_workers(notify_count);
        Ok(outcome)
    }

    pub fn mark_replay_running(
        &self,
        replay_id: &Uuid,
        at: UtcTimestamp,
    ) -> Result<(), RunStoreTransitionError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.active.contains(&JobId::Replay(replay_id.clone())) {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        let replay = state
            .replays
            .get_mut(replay_id)
            .ok_or(RunStoreTransitionError::NotFound)?;
        if replay.status.status != "queued" {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        replay.status.status = "running".to_owned();
        replay.status.started_at = Some(at);
        Ok(())
    }

    pub fn mark_replay_terminal(
        &self,
        replay_id: &Uuid,
        at: UtcTimestamp,
        report: Option<ReplayReport>,
        failure: Option<ReplayFailure>,
    ) -> Result<(), RunStoreTransitionError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.active.contains(&JobId::Replay(replay_id.clone())) {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        let replay = state
            .replays
            .get_mut(replay_id)
            .ok_or(RunStoreTransitionError::NotFound)?;
        if replay.status.status != "running" {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        replay.status.status = if failure.is_none() {
            "succeeded"
        } else {
            "failed"
        }
        .to_owned();
        replay.status.finished_at = Some(at);
        replay.status.failure = failure;
        replay.report = report;
        Ok(())
    }

    pub fn get_result(&self, run_id: &Uuid) -> Result<RunResult, RunResultLookupError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let run = state
            .runs
            .get(run_id)
            .ok_or(RunResultLookupError::NotFound)?;
        match run.status.status {
            RunLifecycle::Queued | RunLifecycle::Running => Err(RunResultLookupError::NotSucceeded),
            RunLifecycle::Failed => Err(RunResultLookupError::Failed),
            RunLifecycle::Succeeded => run
                .result
                .clone()
                .ok_or(RunResultLookupError::ResultUnavailable),
        }
    }

    pub fn mark_running(
        &self,
        run_id: &Uuid,
        started_at: UtcTimestamp,
    ) -> Result<(), RunStoreTransitionError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.active.contains(&JobId::Run(run_id.clone())) {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        let run = state
            .runs
            .get_mut(run_id)
            .ok_or(RunStoreTransitionError::NotFound)?;
        if run.status.status != RunLifecycle::Queued {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        run.status.status = RunLifecycle::Running;
        run.status.started_at = Some(started_at);
        Ok(())
    }

    pub fn update_progress(
        &self,
        run_id: &Uuid,
        progress: RunProgress,
    ) -> Result<(), RunStoreTransitionError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let run = state
            .runs
            .get_mut(run_id)
            .ok_or(RunStoreTransitionError::NotFound)?;
        if run.status.status != RunLifecycle::Running {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        run.status.progress = Some(progress);
        Ok(())
    }

    pub fn mark_succeeded(
        &self,
        run_id: &Uuid,
        finished_at: UtcTimestamp,
        result: RunResult,
    ) -> Result<(), RunStoreTransitionError> {
        if &result.run_id != run_id {
            return Err(RunStoreTransitionError::ResultRunMismatch);
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let run = state
            .runs
            .get_mut(run_id)
            .ok_or(RunStoreTransitionError::NotFound)?;
        if run.status.status != RunLifecycle::Running {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        run.status.status = RunLifecycle::Succeeded;
        run.status.finished_at = Some(finished_at);
        run.status.failure = None;
        run.result = Some(result);
        Ok(())
    }

    pub fn mark_failed(
        &self,
        run_id: &Uuid,
        finished_at: UtcTimestamp,
        failure: RunFailure,
    ) -> Result<(), RunStoreTransitionError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.active.contains(&JobId::Run(run_id.clone())) {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        let run = state
            .runs
            .get_mut(run_id)
            .ok_or(RunStoreTransitionError::NotFound)?;
        if !matches!(
            run.status.status,
            RunLifecycle::Queued | RunLifecycle::Running
        ) {
            return Err(RunStoreTransitionError::InvalidTransition);
        }
        run.status.status = RunLifecycle::Failed;
        run.status.finished_at = Some(finished_at);
        run.status.failure = Some(failure);
        run.result = None;
        Ok(())
    }

    /// Atomically bind one caller-supplied submission ID to a typed config and
    /// create one queued run. The dispatcher applies active/queued capacity.
    pub fn accept_submission(
        &self,
        submission_id: Uuid,
        configuration: RunConfiguration,
        submitted_at: UtcTimestamp,
    ) -> Result<SubmissionOutcome, SubmissionStoreError> {
        let mut notify_count = 0;
        let outcome = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(existing) = state.submissions.get(&submission_id) {
                return if existing.configuration == configuration {
                    Ok(SubmissionOutcome {
                        acknowledgement: existing.acknowledgement.clone(),
                        created: false,
                    })
                } else {
                    Err(SubmissionStoreError::SubmissionIdReused)
                };
            }
            if !state.accepting {
                return Err(SubmissionStoreError::BackendUnavailable);
            }

            notify_count += promote_waiting_runs(&mut state, self.limits);
            let run_id = new_uuid_v4();
            let acknowledgement = SubmissionAccepted {
                submission_id: submission_id.clone(),
                run_id: run_id.clone(),
                status: factorsbt_contracts::outcomes::QueuedStatus::Queued,
                submitted_at: submitted_at.clone(),
            };
            if state.active.len() < self.limits.max_active_runs {
                let job = JobId::Run(run_id.clone());
                state.active.insert(job.clone());
                state.starting.push_back(job);
                notify_count += 1;
            } else if state.queued.len() < self.limits.max_queued_runs {
                state.queued.push_back(JobId::Run(run_id.clone()));
            } else {
                return Err(SubmissionStoreError::QueueFull);
            }
            state.submissions.insert(
                submission_id.clone(),
                StoredSubmission {
                    configuration: configuration.clone(),
                    acknowledgement: acknowledgement.clone(),
                },
            );
            state.runs.insert(
                run_id.clone(),
                StoredRun {
                    submission_id,
                    configuration,
                    status: RunStatus {
                        run_id,
                        status: RunLifecycle::Queued,
                        submitted_at,
                        started_at: None,
                        finished_at: None,
                        progress: None,
                        failure: None,
                    },
                    result: None,
                },
            );
            SubmissionOutcome {
                acknowledgement,
                created: true,
            }
        };
        self.notify_workers(notify_count);
        Ok(outcome)
    }

    /// Stop admitting new identities. Existing idempotent submissions still
    /// resolve to their original acknowledgement while workers drain.
    pub fn stop_accepting(&self) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.accepting = false;
        }
        self.ready.notify_waiters();
    }

    pub async fn next_reserved_run(&self) -> Option<JobId> {
        loop {
            let notified = self.ready.notified();
            let next = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Some(run_id) = state.starting.pop_front() {
                    Some(Some(run_id))
                } else if !state.accepting && state.active.is_empty() && state.queued.is_empty() {
                    Some(None)
                } else {
                    None
                }
            };
            if let Some(next) = next {
                return next;
            }
            notified.await;
        }
    }

    pub fn finish_active_run(&self, job_id: &JobId) -> bool {
        let (promoted, drained) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !state.active.remove(job_id) {
                return false;
            }
            let promoted = promote_waiting_runs(&mut state, self.limits);
            let drained = !state.accepting && state.active.is_empty() && state.queued.is_empty();
            (promoted, drained)
        };
        self.notify_workers(promoted);
        if drained {
            self.ready.notify_waiters();
        }
        true
    }

    fn notify_workers(&self, count: usize) {
        for _ in 0..count {
            self.ready.notify_one();
        }
    }
}

fn promote_waiting_runs(state: &mut QueueState, limits: QueueLimits) -> usize {
    let mut promoted = 0;
    while state.active.len() < limits.max_active_runs {
        let Some(job_id) = state.queued.pop_front() else {
            break;
        };
        state.active.insert(job_id.clone());
        state.starting.push_back(job_id);
        promoted += 1;
    }
    promoted
}

fn replay_acknowledgement(status: &ReplayStatus) -> ReplayAcknowledgement {
    ReplayAcknowledgement {
        contract_version: status.contract_version.clone(),
        replay_id: status.replay_id.clone(),
        source_run_id: status.source_run_id.clone(),
        status: "queued",
        submitted_at: status.submitted_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend_config::QueueLimits;

    #[test]
    fn cloned_store_handles_share_one_empty_process_local_queue() {
        let store = RunStore::new(QueueLimits::new(3, 7).unwrap());
        let clone = store.clone();
        assert_eq!(store.limits(), QueueLimits::new(3, 7).unwrap());
        assert_eq!(
            clone.queue_snapshot(),
            QueueSnapshot {
                active_runs: 0,
                queued_runs: 0,
                limits: QueueLimits::new(3, 7).unwrap(),
            }
        );
        assert_eq!(
            clone.record_counts(),
            StoreRecordCounts {
                submissions: 0,
                runs: 0
            }
        );
    }

    fn configuration() -> RunConfiguration {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap()
    }

    fn submission_id() -> Uuid {
        Uuid::new("a0c0e095-c4b1-4a65-a05a-1b5689911ac0").unwrap()
    }

    fn timestamp() -> UtcTimestamp {
        UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap()
    }

    #[tokio::test]
    async fn run_and_replay_ids_with_same_uuid_remain_distinct_fifo_jobs() {
        let store = RunStore::new(QueueLimits::new(1, 1).unwrap());
        let run = store
            .accept_submission(submission_id(), configuration(), timestamp())
            .unwrap();
        let collision = run.acknowledgement.run_id.clone();
        let replay = store
            .accept_replay(
                collision.clone(),
                Uuid::new("b1c0e095-c4b1-4a65-a05a-1b5689911ac0").unwrap(),
                timestamp(),
            )
            .unwrap();
        assert!(replay.created);
        assert_eq!(store.queue_snapshot().active_runs, 1);
        assert_eq!(store.queue_snapshot().queued_runs, 1);
        assert!(
            !store
                .accept_replay(
                    collision.clone(),
                    replay.acknowledgement.source_run_id.clone(),
                    timestamp()
                )
                .unwrap()
                .created
        );
        assert!(matches!(
            store.accept_replay(collision.clone(), submission_id(), timestamp()),
            Err(SubmissionStoreError::SubmissionIdReused)
        ));
        assert_eq!(
            store.next_reserved_run().await,
            Some(JobId::Run(run.acknowledgement.run_id.clone()))
        );
        assert!(store.finish_active_run(&JobId::Run(run.acknowledgement.run_id)));
        assert_eq!(
            store.next_reserved_run().await,
            Some(JobId::Replay(collision.clone()))
        );
        store.mark_replay_running(&collision, timestamp()).unwrap();
        store
            .mark_replay_terminal(
                &collision,
                timestamp(),
                None,
                Some(ReplayFailure {
                    code: "TEST_FAILURE".into(),
                    message: "controlled".into(),
                }),
            )
            .unwrap();
        assert_eq!(
            store.get_replay(&collision).unwrap().status.status,
            "failed"
        );
    }

    #[test]
    fn returns_the_original_ack_for_equal_config_and_conflicts_for_changed_config() {
        let store = RunStore::new(QueueLimits::default());
        let first = store
            .accept_submission(submission_id(), configuration(), timestamp())
            .unwrap();
        assert!(first.created);
        let repeated = store
            .accept_submission(submission_id(), configuration(), timestamp())
            .unwrap();
        assert!(!repeated.created);
        assert_eq!(repeated.acknowledgement, first.acknowledgement);
        assert_eq!(store.queue_snapshot().active_runs, 1);
        assert_eq!(store.queue_snapshot().queued_runs, 0);

        let mut changed = configuration();
        changed.model.model_version = factorsbt_contracts::Identifier::new("v2").unwrap();
        assert_eq!(
            store.accept_submission(submission_id(), changed, timestamp()),
            Err(SubmissionStoreError::SubmissionIdReused)
        );
        assert_eq!(store.queue_snapshot().active_runs, 1);
        assert_eq!(store.queue_snapshot().queued_runs, 0);
        assert_eq!(
            store.record_counts(),
            StoreRecordCounts {
                submissions: 1,
                runs: 1,
            }
        );
    }

    #[test]
    fn object_member_order_does_not_change_typed_configuration_equality() {
        let original_json =
            include_str!("../../../../contracts/examples/instruments/minimal-valid-spot-run.json");
        let original_value: serde_json::Value = serde_json::from_str(original_json).unwrap();
        let mut entries = original_value
            .as_object()
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        entries.reverse();
        let reordered_json = format!(
            "{{{}}}",
            entries
                .into_iter()
                .map(|(key, value)| format!(
                    "{}:{}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string(value).unwrap()
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
        let original: RunConfiguration = serde_json::from_str(original_json).unwrap();
        let reordered: RunConfiguration = serde_json::from_str(&reordered_json).unwrap();
        assert_eq!(original, reordered);

        let store = RunStore::new(QueueLimits::default());
        let first = store
            .accept_submission(submission_id(), original, timestamp())
            .unwrap();
        let second = store
            .accept_submission(submission_id(), reordered, timestamp())
            .unwrap();
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.acknowledgement, second.acknowledgement);
    }

    #[test]
    fn concurrent_equal_submissions_create_only_one_queued_run() {
        let store = RunStore::new(QueueLimits::default());
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let store = store.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .accept_submission(submission_id(), configuration(), timestamp())
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|outcome| outcome.created).count(), 1);
        assert!(outcomes
            .iter()
            .all(|outcome| outcome.acknowledgement == outcomes[0].acknowledgement));
        assert_eq!(store.queue_snapshot().active_runs, 1);
        assert_eq!(store.queue_snapshot().queued_runs, 0);
        assert_eq!(store.record_counts().runs, 1);
    }

    #[test]
    fn exposes_queued_running_progress_and_terminal_failure_transitions() {
        let store = RunStore::new(QueueLimits::default());
        let accepted = store
            .accept_submission(submission_id(), configuration(), timestamp())
            .unwrap()
            .acknowledgement;
        assert_eq!(
            store.get_submission(&submission_id()).unwrap().status,
            RunLifecycle::Queued
        );
        let started_at = UtcTimestamp::new("2026-01-05T16:00:01Z").unwrap();
        store
            .mark_running(&accepted.run_id, started_at.clone())
            .unwrap();
        let progress = RunProgress {
            scenario_id: factorsbt_contracts::Identifier::new("baseline").unwrap(),
            step_id: factorsbt_contracts::Identifier::new("step-0001").unwrap(),
            simulation_time: UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap(),
            completed_steps: 1,
            total_steps: 3,
        };
        store
            .update_progress(&accepted.run_id, progress.clone())
            .unwrap();
        let running = store.get_run(&accepted.run_id).unwrap().status;
        assert_eq!(running.status, RunLifecycle::Running);
        assert_eq!(running.started_at, Some(started_at));
        assert_eq!(running.progress, Some(progress));

        let finished_at = UtcTimestamp::new("2026-01-05T16:00:03Z").unwrap();
        store
            .mark_failed(
                &accepted.run_id,
                finished_at.clone(),
                RunFailure {
                    code: "MODEL_FAILURE".to_owned(),
                    message: "controlled model failure".to_owned(),
                    scenario_id: Some(factorsbt_contracts::Identifier::new("baseline").unwrap()),
                    step_id: Some(factorsbt_contracts::Identifier::new("step-0001").unwrap()),
                    last_valid_state_ref: Some("evidence/summary.json".to_owned()),
                },
            )
            .unwrap();
        let failed = store.get_run(&accepted.run_id).unwrap().status;
        assert_eq!(failed.status, RunLifecycle::Failed);
        assert_eq!(failed.finished_at, Some(finished_at));
        assert_eq!(failed.failure.unwrap().code, "MODEL_FAILURE");
        assert_eq!(
            store.get_submission(&submission_id()).unwrap().status,
            RunLifecycle::Failed
        );
        assert_eq!(
            store.get_result(&accepted.run_id),
            Err(RunResultLookupError::Failed)
        );
        assert_eq!(store.queue_snapshot().active_runs, 1);
        assert!(store.mark_running(&accepted.run_id, timestamp()).is_err());
    }

    #[test]
    fn result_is_visible_only_after_a_successful_terminal_transition() {
        let store = RunStore::new(QueueLimits::default());
        let accepted = store
            .accept_submission(submission_id(), configuration(), timestamp())
            .unwrap()
            .acknowledgement;
        assert_eq!(
            store.get_result(&accepted.run_id),
            Err(RunResultLookupError::NotSucceeded)
        );
        store.mark_running(&accepted.run_id, timestamp()).unwrap();
        let mut result: RunResult = serde_json::from_str(include_str!(
            "../../../../contracts/examples/results/successful-run.valid.json"
        ))
        .unwrap();
        result.run_id = accepted.run_id.clone();
        store
            .mark_succeeded(
                &accepted.run_id,
                UtcTimestamp::new("2026-01-05T16:01:00Z").unwrap(),
                result.clone(),
            )
            .unwrap();
        assert_eq!(store.get_result(&accepted.run_id), Ok(result));
    }
}
