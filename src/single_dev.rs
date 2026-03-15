use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{sinlge_effort::SingleEffortWeek, workers::WorkerId};

#[derive(Serialize, Deserialize)]
pub struct SingleDev {
    weeks: HashMap<usize, SingleEffortWeek>,
    effort: usize,
}

impl SingleDev {
    pub fn new() -> Self {
        Self {
            weeks: HashMap::new(),
            effort: 0,
        }
    }

    pub fn set_effort(&mut self, effort: usize) {
        self.effort = effort;
    }

    pub fn get_effort_tot(&self) -> usize {
        self.weeks.values().map(|single| single.effort_tot()).sum()
    }

    pub fn get_effort(&self, week: usize, worker_id: WorkerId) -> usize {
        self.weeks.get(&week).map_or(0, |f| f.effort(worker_id))
    }

    pub fn add(&mut self, week: usize, id_worker: WorkerId, effort: usize) {
        self.weeks
            .entry(week)
            .or_insert_with(SingleEffortWeek::new)
            .add(id_worker, effort);
    }

    pub fn planned_effort(&self) -> usize {
        self.effort
    }

    pub fn get_weeks(&self) -> Vec<usize> {
        let mut weeks: Vec<usize> = self.weeks.keys().cloned().collect();
        weeks.sort();
        weeks
    }

    pub fn set_note(&mut self, week: usize, id_worker: WorkerId, note: &str) {
        if let Some(single) = self.weeks.get_mut(&week) {
            single.set_note(id_worker, note);
        }
    }
}
