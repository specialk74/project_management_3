#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    single_efforts::{sinlge_effort::Effort, sinlge_effort_week::SingleEffortWeek},
    workers::worker::WorkerId,
};

#[derive(
    Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug, Default,
)]
pub struct WeekId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct SingleDev {
    weeks: HashMap<WeekId, SingleEffortWeek>,
    effort: Effort,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl SingleDev {
    pub fn new() -> Self {
        Self {
            weeks: HashMap::new(),
            effort: Effort(0),
            note: None,
        }
    }

    pub fn del_row(&mut self) {
        self.weeks.iter_mut().for_each(|(_, s)| s.remove_zero());
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

    pub fn get_effort_by_week(&self, week: WeekId) -> Effort {
        self.weeks.get(&week).map_or(Effort(0), |f| f.effort_tot())
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

    pub fn set_dev_note(&mut self, note: &str) {
        self.note = if note.is_empty() {
            None
        } else {
            Some(note.to_string())
        };
    }

    pub fn get_note(&self) -> String {
        self.note.clone().unwrap_or(String::from(""))
    }

    pub fn get_all(&self, week: WeekId) -> Option<&SingleEffortWeek> {
        self.weeks.get(&week)
    }

    pub fn has_worker(&self, worker_id: WorkerId) -> bool {
        self.weeks
            .values()
            .any(|week| week.worker_id.contains_key(&worker_id))
    }

    pub fn get_workers(&self, week: WeekId) -> Vec<&WorkerId> {
        self.weeks
            .get(&week)
            .map(|s| s.get_workers())
            .unwrap_or_default()
    }
}
