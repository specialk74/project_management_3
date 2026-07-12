#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    single_effort_utils::{sinlge_effort::Effort, sinlge_effort_week::SingleEffortWeek},
    workers_utils::worker::WorkerId,
};

#[derive(
    Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug, Default,
)]
pub struct WeekId(pub usize);

/// Un `WeekId` è in realtà un numero di giorno assoluto; settimane consecutive
/// distano 7 (vedi `weeks_vec`, che genera le colonne con `step_by(7)`). Serve
/// per riconoscere le settimane adiacenti e per convertire "n settimane" in
/// giorni negli spostamenti di effort.
pub const WEEK_STEP: usize = 7;

fn is_false(b: &bool) -> bool {
    !*b
}

/// Una voce dello storico della percentuale dichiarata dallo sviluppatore:
/// il valore `pct` e la **settimana** in cui è stato registrato (`WeekId`, come
/// per la griglia). Lo storico serve per l'andamento nel tempo (una voce per
/// settimana, l'ultimo valore).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DeclaredPoint {
    pub week: WeekId,
    pub pct: u8,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SingleDev {
    weeks: HashMap<WeekId, SingleEffortWeek>,
    effort: Effort,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    hide_effort: bool,
    /// Storico della percentuale dichiarata dallo sviluppatore: `(settimana,
    /// valore)` ordinato per settimana, una voce per settimana (l'ultima è il
    /// valore corrente). Vuoto = mai dichiarata (corrente = 0).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    declared_history: Vec<DeclaredPoint>,
}

impl SingleDev {
    pub fn new() -> Self {
        Self {
            weeks: HashMap::new(),
            effort: Effort(0),
            note: None,
            hide_effort: false,
            declared_history: Vec::new(),
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

    /// Effort registrato nelle settimane fino a (e inclusa) `week` — ossia
    /// l'effort "usato" a una certa data. Usato per la % sotto il nome del dev.
    pub fn effort_up_to(&self, week: WeekId) -> Effort {
        self.weeks
            .iter()
            .filter(|(w, _)| **w <= week)
            .map(|(_, s)| s.effort_tot())
            .sum()
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

    /// Prima e ultima settimana in cui il dev ha effort registrato (>0).
    /// `None` se non c'è alcun effort. Usato per la barra Gantt nel PDF.
    pub fn effort_span(&self) -> Option<(WeekId, WeekId)> {
        let mut min: Option<WeekId> = None;
        let mut max: Option<WeekId> = None;
        for (week, s) in self.weeks.iter() {
            if s.effort_tot().0 == 0 {
                continue;
            }
            min = Some(min.map_or(*week, |m| m.min(*week)));
            max = Some(max.map_or(*week, |m| m.max(*week)));
        }
        Some((min?, max?))
    }

    /// Settimane con effort effettivo (>0), ordinate.
    pub fn effort_weeks(&self) -> Vec<WeekId> {
        let mut v: Vec<WeekId> = self
            .weeks
            .iter()
            .filter(|(_, s)| s.effort_tot().0 > 0)
            .map(|(w, _)| *w)
            .collect();
        v.sort();
        v
    }

    /// Blocco contiguo di settimane con effort (>0) che contiene `around`
    /// (settimane consecutive per numero). Vuoto se `around` non ha effort.
    pub fn contiguous_block(&self, around: WeekId) -> Vec<WeekId> {
        let has = |w: usize| {
            self.weeks
                .get(&WeekId(w))
                .is_some_and(|s| s.effort_tot().0 > 0)
        };
        if !has(around.0) {
            return Vec::new();
        }
        let mut lo = around.0;
        while lo >= WEEK_STEP && has(lo - WEEK_STEP) {
            lo -= WEEK_STEP;
        }
        let mut hi = around.0;
        while has(hi + WEEK_STEP) {
            hi += WEEK_STEP;
        }
        (lo..=hi).step_by(WEEK_STEP).map(WeekId).collect()
    }

    /// Sposta le settimane indicate di `delta` (positivo = a destra). Le
    /// settimane che finirebbero sotto 0, oppure — se il relativo limite è
    /// `Some` — fuori dai confini `trunc_start`/`trunc_end`, vengono perse.
    /// In caso di collisione con una settimana già presente, gli effort dei
    /// worker vengono uniti.
    pub fn shift_weeks(
        &mut self,
        weeks: &[WeekId],
        delta: i64,
        trunc_start: Option<i64>,
        trunc_end: Option<i64>,
    ) {
        let mut moved: Vec<(i64, SingleEffortWeek)> = Vec::new();
        for w in weeks {
            if let Some(v) = self.weeks.remove(w) {
                moved.push((w.0 as i64 + delta, v));
            }
        }
        for (nw, v) in moved {
            if nw < 0 {
                continue;
            }
            if let Some(ts) = trunc_start {
                if nw < ts {
                    continue;
                }
            }
            if let Some(te) = trunc_end {
                if nw > te {
                    continue;
                }
            }
            let key = WeekId(nw as usize);
            match self.weeks.get_mut(&key) {
                Some(existing) => existing.worker_id.extend(v.worker_id),
                None => {
                    self.weeks.insert(key, v);
                }
            }
        }
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

    pub fn set_hide_effort(&mut self, hide: bool) {
        self.hide_effort = hide;
    }

    pub fn get_hide_effort(&self) -> bool {
        self.hide_effort
    }

    /// Percentuale dichiarata **corrente** (l'ultima voce dello storico), o 0 se
    /// mai dichiarata.
    pub fn declared_pct(&self) -> u8 {
        self.declared_history.last().map_or(0, |p| p.pct)
    }

    /// Storico completo `(settimana, valore)` ordinato per settimana. Usato per
    /// il futuro grafico dell'andamento nel tempo.
    pub fn declared_history(&self) -> &[DeclaredPoint] {
        &self.declared_history
    }

    /// Registra il valore dichiarato per la settimana `week`. Una voce per
    /// settimana (se esiste già quella settimana, ne aggiorna il valore),
    /// altrimenti la aggiunge mantenendo l'ordine. Non registra nulla se il
    /// valore coincide con quello corrente (nessuna modifica). Ritorna `true`
    /// solo se lo storico è cambiato.
    pub fn set_declared_pct(&mut self, week: WeekId, pct: u8) -> bool {
        let pct = pct.min(100);
        if pct == self.declared_pct() {
            return false;
        }
        if let Some(p) = self.declared_history.iter_mut().find(|p| p.week == week) {
            p.pct = pct;
        } else {
            self.declared_history.push(DeclaredPoint { week, pct });
            self.declared_history.sort_by_key(|p| p.week);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workers_utils::worker::WorkerId;

    #[test]
    fn effort_up_to_only_counts_weeks_at_or_before() {
        let mut sd = SingleDev::new();
        let w = WorkerId(1);
        sd.add(WeekId(0), w, Effort(10)); // fino a oggi
        sd.add(WeekId(7), w, Effort(20)); // fino a oggi (== soglia)
        sd.add(WeekId(14), w, Effort(30)); // futura → esclusa

        assert_eq!(sd.effort_up_to(WeekId(7)).0, 30);
        assert_eq!(sd.get_effort_tot().0, 60);
        assert_eq!(sd.effort_up_to(WeekId(14)).0, 60);
        assert_eq!(sd.effort_up_to(WeekId(0)).0, 10);
    }

    #[test]
    fn declared_pct_defaults_zero_and_clamps_to_100() {
        let mut sd = SingleDev::new();
        assert_eq!(sd.declared_pct(), 0);
        assert!(sd.declared_history().is_empty());
        assert!(sd.set_declared_pct(WeekId(700), 60));
        assert_eq!(sd.declared_pct(), 60);
        // Nuova settimana con valore diverso → clamp e nuova voce.
        assert!(sd.set_declared_pct(WeekId(707), 200));
        assert_eq!(sd.declared_pct(), 100);
        assert_eq!(sd.declared_history().len(), 2);
    }

    #[test]
    fn old_declared_pct_field_is_ignored_on_load() {
        // File "vecchio" con il campo `declared_pct`: deve caricarsi ignorandolo
        // (storico vuoto), non dare errore — non retro-compatibile ma non rompe
        // il load degli altri dati.
        let sd: SingleDev = ron::from_str("(weeks: {}, effort: (0), declared_pct: 60)")
            .expect("il vecchio campo declared_pct va ignorato");
        assert!(sd.declared_history().is_empty());
        assert_eq!(sd.declared_pct(), 0);
    }

    #[test]
    fn declared_history_one_entry_per_week_last_value() {
        let mut sd = SingleDev::new();
        // Due modifiche nella stessa settimana → una sola voce, l'ultimo valore.
        assert!(sd.set_declared_pct(WeekId(700), 40));
        assert!(sd.set_declared_pct(WeekId(700), 55));
        assert_eq!(sd.declared_history().len(), 1);
        assert_eq!(sd.declared_pct(), 55);
        // Stesso valore corrente → nessuna nuova voce, ritorna false.
        assert!(!sd.set_declared_pct(WeekId(707), 55));
        assert_eq!(sd.declared_history().len(), 1);
        // Settimana successiva con valore diverso → seconda voce.
        assert!(sd.set_declared_pct(WeekId(707), 70));
        assert_eq!(
            sd.declared_history(),
            &[
                DeclaredPoint { week: WeekId(700), pct: 55 },
                DeclaredPoint { week: WeekId(707), pct: 70 },
            ]
        );
    }
}
