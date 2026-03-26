#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    single_efforts::sinlge_effort::{Effort, SingleEffort},
    workers::worker::{WORKER_ID_ZERO, WorkerId},
};

#[derive(Serialize, Deserialize)]
pub struct SingleEffortWeek {
    pub worker_id: HashMap<WorkerId, SingleEffort>,
}

impl std::iter::Sum for Effort {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Effort(iter.map(|e| e.0).sum())
    }
}

impl SingleEffortWeek {
    pub fn new() -> Self {
        Self {
            worker_id: HashMap::new(),
        }
    }

    pub fn remove_zero(&mut self) {
        self.worker_id.remove(&WORKER_ID_ZERO);
    }

    // pub fn reset_effort(&mut self) {
    //     self.worker_id.clear();
    // }

    pub fn has_worker_zero(&self) -> bool {
        self.worker_id.contains_key(&WorkerId(0))
    }

    pub fn effort_tot(&self) -> Effort {
        self.worker_id.values().map(|f| f.get_effort()).sum()
    }

    pub fn effort(&self, worker_id: WorkerId) -> Effort {
        self.worker_id
            .get(&worker_id)
            .map_or(Effort(0), |f| f.get_effort())
    }

    pub fn add(&mut self, id_worker: WorkerId, effort: Effort) {
        self.worker_id
            .entry(id_worker)
            .and_modify(|e| e.set_effort(effort))
            .or_insert_with(|| SingleEffort::new(effort));
    }

    pub fn set_note(&mut self, id_worker: WorkerId, note: &str) {
        if let Some(single_effort) = self.worker_id.get_mut(&id_worker) {
            single_effort.set_note(note);
        }
    }

    pub fn get_workers(&self) -> Vec<&WorkerId> {
        self.worker_id.keys().collect()
    }

    pub fn num_workers(&self) -> usize {
        self.worker_id.len()
    }
}
