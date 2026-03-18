use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::workers::WorkerId;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Default)]
pub struct Effort(pub usize);

#[derive(Serialize, Deserialize)]
pub struct SingleEffort {
    effort: Effort,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl SingleEffort {
    pub fn new(effort: Effort) -> Self {
        Self { effort, note: None }
    }

    pub fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
    }

    pub fn set_note(&mut self, note: &str) {
        self.note = Some(note.to_string());
    }

    pub fn get_effort(&self) -> usize {
        self.effort.0
    }
}

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

    // pub fn reset_effort(&mut self) {
    //     self.worker_id.clear();
    // }

    pub fn has_worker_zero(&self) -> bool {
        self.worker_id.contains_key(&WorkerId(0))
    }

    pub fn effort_tot(&self) -> Effort {
        self.worker_id.values().map(|f| f.effort).sum()
    }

    pub fn effort(&self, worker_id: WorkerId) -> Effort {
        self.worker_id
            .get(&worker_id)
            .map_or(Effort(0), |f| f.effort)
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
        self.worker_id.iter().count()
    }
}
