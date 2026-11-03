//! FIFO worker pool that reserves no more than the configured active capacity.

use crate::run_store::RunStore;
use factorsbt_contracts::Uuid;
use std::{future::Future, sync::Arc};
use tokio::task::JoinHandle;

pub struct RunDispatcher {
    workers: Vec<JoinHandle<()>>,
}

impl RunDispatcher {
    /// Start a fixed pool. Handler panics are isolated to their task, and the
    /// worker always releases its run slot before accepting another job.
    pub fn start<F, Fut>(store: RunStore, handler: F) -> Self
    where
        F: Fn(Uuid) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let worker_count = store.limits().max_active_runs;
        let workers = (0..worker_count)
            .map(|_| {
                let store = store.clone();
                let handler = Arc::clone(&handler);
                tokio::spawn(async move {
                    loop {
                        let run_id = store.next_reserved_run().await;
                        let task = tokio::spawn(handler(run_id.clone()));
                        let _ = task.await;
                        store.finish_active_run(&run_id);
                    }
                })
            })
            .collect();
        Self { workers }
    }

    /// Stop the worker loops. The service owns run status and graceful shutdown
    /// behavior; this low-level primitive only joins its worker tasks.
    pub async fn shutdown(self) {
        for worker in &self.workers {
            worker.abort();
        }
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
}
