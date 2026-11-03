//! FIFO worker pool that reserves no more than the configured active capacity.

use crate::run_store::RunStore;
use factorsbt_contracts::Uuid;
use std::{future::Future, sync::Arc};
use tokio::task::JoinHandle;

pub struct RunDispatcher {
    workers: Vec<JoinHandle<()>>,
    store: RunStore,
}

impl RunDispatcher {
    /// Start a fixed pool. Handler panics are isolated to their task, and the
    /// worker always releases its run slot before accepting another job.
    pub fn start<F, Fut>(store: RunStore, handler: F) -> Self
    where
        F: Fn(Uuid) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self::start_with_failure_handler(store, handler, |_| {})
    }

    /// Start workers with an isolated panic callback. A callback panic cannot
    /// prevent the worker from releasing its reserved run slot.
    pub fn start_with_failure_handler<F, Fut, P>(store: RunStore, handler: F, on_panic: P) -> Self
    where
        F: Fn(Uuid) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
        P: Fn(Uuid) + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        let on_panic = Arc::new(on_panic);
        let worker_count = store.limits().max_active_runs;
        let workers = (0..worker_count)
            .map(|_| {
                let store = store.clone();
                let handler = Arc::clone(&handler);
                let on_panic = Arc::clone(&on_panic);
                tokio::spawn(async move {
                    loop {
                        let Some(run_id) = store.next_reserved_run().await else {
                            return;
                        };
                        let task = tokio::spawn(handler(run_id.clone()));
                        if task.await.is_err() {
                            let on_panic = Arc::clone(&on_panic);
                            let failed_run_id = run_id.clone();
                            let _ = tokio::spawn(async move { on_panic(failed_run_id) }).await;
                        }
                        store.finish_active_run(&run_id);
                    }
                })
            })
            .collect();
        Self { workers, store }
    }

    /// Stop new submissions and drain all accepted work before joining workers.
    pub async fn shutdown(self) {
        self.store.stop_accepting();
        for worker in self.workers {
            let _ = worker.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend_config::QueueLimits, run_store::SubmissionStoreError};
    use factorsbt_contracts::{run_configuration::RunConfiguration, UtcTimestamp};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{mpsc, Semaphore};

    fn configuration() -> RunConfiguration {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap()
    }

    fn timestamp() -> UtcTimestamp {
        UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap()
    }

    fn submission_id(index: u128) -> Uuid {
        Uuid::new(format!("a0c0e095-c4b1-4a65-a05a-{index:012x}")).unwrap()
    }

    #[tokio::test]
    async fn enforces_active_and_queue_limits_and_starts_waiters_in_fifo_order() {
        let store = RunStore::new(QueueLimits::new(2, 2).unwrap());
        let first = store
            .accept_submission(submission_id(1), configuration(), timestamp())
            .unwrap()
            .acknowledgement
            .run_id;
        let second = store
            .accept_submission(submission_id(2), configuration(), timestamp())
            .unwrap()
            .acknowledgement
            .run_id;
        let third = store
            .accept_submission(submission_id(3), configuration(), timestamp())
            .unwrap()
            .acknowledgement
            .run_id;
        let fourth = store
            .accept_submission(submission_id(4), configuration(), timestamp())
            .unwrap()
            .acknowledgement
            .run_id;
        assert_eq!(
            store.queue_snapshot().active_runs,
            2,
            "two start slots are reserved immediately"
        );
        assert_eq!(store.queue_snapshot().queued_runs, 2);
        assert_eq!(
            store.accept_submission(submission_id(5), configuration(), timestamp()),
            Err(SubmissionStoreError::QueueFull)
        );
        let retry = store
            .accept_submission(submission_id(4), configuration(), timestamp())
            .unwrap();
        assert!(!retry.created);
        assert_eq!(retry.acknowledgement.run_id, fourth);
        assert_eq!(store.record_counts().runs, 4);

        let (started_tx, mut started_rx) = mpsc::unbounded_channel();
        let (finished_tx, mut finished_rx) = mpsc::unbounded_channel();
        let release = Arc::new(Semaphore::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let handler = {
            let release = Arc::clone(&release);
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            move |run_id: Uuid| {
                let release = Arc::clone(&release);
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                let started_tx = started_tx.clone();
                let finished_tx = finished_tx.clone();
                async move {
                    let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(count, Ordering::SeqCst);
                    started_tx.send(run_id.clone()).unwrap();
                    let permit = release.acquire().await.unwrap();
                    permit.forget();
                    active.fetch_sub(1, Ordering::SeqCst);
                    finished_tx.send(run_id).unwrap();
                }
            }
        };
        let dispatcher = RunDispatcher::start(store.clone(), handler);

        let initial_a = started_rx.recv().await.unwrap();
        let initial_b = started_rx.recv().await.unwrap();
        assert_ne!(initial_a, initial_b);
        assert!([first.clone(), second.clone()].contains(&initial_a));
        assert!([first.clone(), second.clone()].contains(&initial_b));
        assert_eq!(max_active.load(Ordering::SeqCst), 2);

        release.add_permits(1);
        finished_rx.recv().await.unwrap();
        assert_eq!(started_rx.recv().await.unwrap(), third);
        release.add_permits(1);
        finished_rx.recv().await.unwrap();
        assert_eq!(started_rx.recv().await.unwrap(), fourth);
        release.add_permits(2);
        finished_rx.recv().await.unwrap();
        finished_rx.recv().await.unwrap();

        assert_eq!(max_active.load(Ordering::SeqCst), 2);
        assert_eq!(store.queue_snapshot().active_runs, 0);
        assert_eq!(store.queue_snapshot().queued_runs, 0);
        dispatcher.shutdown().await;
    }

    #[tokio::test]
    async fn panic_callback_runs_and_shutdown_drains_queued_work() {
        let store = RunStore::new(QueueLimits::new(1, 1).unwrap());
        let first = store
            .accept_submission(submission_id(11), configuration(), timestamp())
            .unwrap()
            .acknowledgement
            .run_id;
        let second = store
            .accept_submission(submission_id(12), configuration(), timestamp())
            .unwrap()
            .acknowledgement
            .run_id;
        let panic_seen = Arc::new(AtomicUsize::new(0));
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();
        let panic_run_id = first.clone();
        let handler = move |run_id: Uuid| {
            let started_tx = started_tx.clone();
            let panic_run_id = panic_run_id.clone();
            async move {
                started_tx.send(run_id.clone()).unwrap();
                if run_id == panic_run_id {
                    panic!("controlled worker panic");
                }
            }
        };
        let panic_seen_callback = Arc::clone(&panic_seen);
        let callback_store = store.clone();
        let dispatcher =
            RunDispatcher::start_with_failure_handler(store.clone(), handler, move |run_id| {
                panic_seen_callback.fetch_add(1, Ordering::SeqCst);
                callback_store
                    .mark_failed(
                        &run_id,
                        timestamp(),
                        factorsbt_contracts::outcomes::RunFailure {
                            code: "RUN_WORKER_PANICKED".to_owned(),
                            message: "The run worker stopped unexpectedly.".to_owned(),
                            scenario_id: None,
                            step_id: None,
                            last_valid_state_ref: None,
                        },
                    )
                    .unwrap();
            });

        assert_eq!(started_rx.recv().await.unwrap(), first);
        assert_eq!(started_rx.recv().await.unwrap(), second);
        dispatcher.shutdown().await;
        assert_eq!(panic_seen.load(Ordering::SeqCst), 1);
        assert_eq!(
            store.get_run(&first).unwrap().status.status,
            factorsbt_contracts::outcomes::RunLifecycle::Failed
        );
        assert_eq!(store.queue_snapshot().active_runs, 0);
        assert_eq!(store.queue_snapshot().queued_runs, 0);
        let prior = store
            .accept_submission(submission_id(11), configuration(), timestamp())
            .unwrap();
        assert!(!prior.created);
        assert_eq!(prior.acknowledgement.run_id, first);
        assert_eq!(
            store.accept_submission(submission_id(13), configuration(), timestamp()),
            Err(SubmissionStoreError::BackendUnavailable)
        );
    }
}
