use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::sinlge_effort::SingleEffortWeek;

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
        self.weeks
            .iter()
            .map(|(_, single)| single.effort_tot())
            .sum()
    }

    pub fn get_effort(&self, week: usize, worker_id: usize) -> usize {
        self.weeks.get(&week).map_or(0, |f| f.effort(worker_id))
    }

    pub fn add(&mut self, week: usize, id_worker: usize, effort: usize) {
        match self.weeks.get_mut(&week) {
            Some(single) => single.add(id_worker, effort),
            None => {
                let mut single = SingleEffortWeek::new();
                single.add(id_worker, effort);
                self.weeks.insert(week, single);
            }
        }
    }

    pub fn set_note(&mut self, week: usize, id_worker: usize, note: &str) {
        if let Some(single) = self.weeks.get_mut(&week) {
            single.set_note(id_worker, note);
        }
    }
}
