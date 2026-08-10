#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    single_effort_utils::sinlge_effort::{Effort, SingleEffort},
    workers_utils::worker::{WORKER_ID_ZERO, WorkerId},
};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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

    pub fn num_workers(&self) -> usize {
        self.worker_id.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week_with(pairs: &[(usize, usize)]) -> SingleEffortWeek {
        let mut w = SingleEffortWeek::new();
        for (wid, e) in pairs {
            w.add(WorkerId(*wid), Effort(*e));
        }
        w
    }

    /// `add` sullo **stesso** worker sostituisce le ore, non le somma: la cella
    /// della griglia mostra un valore, non un accumulo.
    #[test]
    fn add_replaces_the_effort_of_the_same_worker() {
        let mut w = week_with(&[(1, 8)]);
        w.add(WorkerId(1), Effort(3));
        assert_eq!(w.num_workers(), 1);
        assert_eq!(w.effort_tot(), Effort(3));
    }

    /// Worker diversi nella stessa settimana convivono e si sommano nel totale.
    #[test]
    fn different_workers_add_up_in_the_week_total() {
        let w = week_with(&[(1, 8), (2, 4), (3, 0)]);
        assert_eq!(w.num_workers(), 3);
        assert_eq!(w.effort_tot(), Effort(12));
    }

    /// Sostituendo un effort la **nota** del worker sopravvive: si sta correggendo
    /// il numero di ore, non cancellando quello che era stato annotato.
    #[test]
    fn replacing_the_effort_keeps_the_note() {
        let mut w = week_with(&[(1, 8)]);
        w.set_note(WorkerId(1), "serve conferma");
        w.add(WorkerId(1), Effort(2));
        assert_eq!(w.worker_id[&WorkerId(1)].get_note(), "serve conferma");
        assert_eq!(w.effort_tot(), Effort(2));
    }

    /// Una nota su un worker assente non crea la voce (non si annota il nulla).
    #[test]
    fn note_on_a_missing_worker_is_ignored() {
        let mut w = SingleEffortWeek::new();
        w.set_note(WorkerId(7), "ciao");
        assert_eq!(w.num_workers(), 0);
    }

    /// Il worker "zero" è il segnaposto della riga vuota: `remove_zero` lo toglie
    /// lasciando intatti gli altri.
    #[test]
    fn remove_zero_drops_only_the_placeholder_worker() {
        let mut w = week_with(&[(0, 0), (1, 8)]);
        assert!(w.has_worker_zero());
        w.remove_zero();
        assert!(!w.has_worker_zero());
        assert_eq!(w.num_workers(), 1);
        assert_eq!(w.effort_tot(), Effort(8));
    }
}
