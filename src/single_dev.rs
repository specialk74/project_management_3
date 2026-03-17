use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{sinlge_effort::Effort, sinlge_effort::SingleEffortWeek, workers::WorkerId};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub struct WeekId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct SingleDev {
    weeks: HashMap<WeekId, SingleEffortWeek>,
    effort: Effort,
}

impl SingleDev {
    pub fn new() -> Self {
        Self {
            weeks: HashMap::new(),
            effort: Effort(0),
        }
    }

    pub fn get_week_with_max_worker(&self) -> Option<WeekId> {
        if self.weeks.values().any(|s| s.has_worker_zero()) {
            return None;
        }

        self.weeks
            .iter()
            .max_by_key(|(_, s)| s.num_workers())
            .map(|(week, _)| *week)
    }

    pub fn reset_effort(&mut self, week: WeekId) {
        self.weeks.remove(&week);
    }

    pub fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
    }

    pub fn max_num_efforts(&self) -> usize {
        let mut max = 0;
        for (_, s) in self.weeks.iter() {
            if s.num_workers() > max {
                max = s.num_workers();
            }
        }
        max
    }

    pub fn get_effort_tot(&self) -> Effort {
        self.weeks.values().map(|single| single.effort_tot()).sum()
    }

    pub fn get_effort(&self, week: WeekId, worker_id: WorkerId) -> Effort {
        self.weeks
            .get(&week)
            .map_or(Effort(0), |f| f.effort(worker_id))
    }

    pub fn add(&mut self, week: WeekId, id_worker: WorkerId, effort: Effort) {
        self.weeks
            .entry(week)
            .or_insert_with(SingleEffortWeek::new)
            .add(id_worker, effort);
    }

    pub fn planned_effort(&self) -> Effort {
        self.effort
    }

    pub fn get_weeks(&self) -> Vec<WeekId> {
        let mut weeks: Vec<WeekId> = self.weeks.keys().cloned().collect();
        weeks.sort();
        weeks
    }

    pub fn set_note(&mut self, week: WeekId, id_worker: WorkerId, note: &str) {
        if let Some(single) = self.weeks.get_mut(&week) {
            single.set_note(id_worker, note);
        }
    }

    pub fn get_workers(&self, week: WeekId) -> Vec<&WorkerId> {
        self.weeks
            .get(&week)
            .map(|s| s.get_workers())
            .unwrap_or_default()
    }
}
