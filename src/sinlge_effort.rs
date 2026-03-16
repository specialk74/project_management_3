use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::workers::WorkerId;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
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
}

#[derive(Serialize, Deserialize)]
pub struct SingleEffortWeek {
    worker_id: HashMap<WorkerId, SingleEffort>,
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

    pub fn reset(&mut self) {
        self.worker_id.clear();
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
}
