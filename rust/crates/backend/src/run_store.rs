//! Process-local queue occupancy state; run and submission records are added
//! with the admission operations that begin using them.

use crate::backend_config::QueueLimits;
use factorsbt_contracts::Uuid;
use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
pub struct RunStore {
    limits: QueueLimits,
    state: Arc<Mutex<QueueState>>,
}

#[derive(Debug, Default)]
struct QueueState {
    queued: VecDeque<Uuid>,
    active: HashSet<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueSnapshot {
    pub active_runs: usize,
    pub queued_runs: usize,
    pub limits: QueueLimits,
}

impl RunStore {
    pub fn new(limits: QueueLimits) -> Self {
        Self {
            limits,
            state: Arc::new(Mutex::new(QueueState::default())),
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
    }
}
