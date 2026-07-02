#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    categories::CategoryId,
    dev_utils::dev::DevId,
    milestones::MilestoneId,
    single_dev_utils::single_dev::{SingleDev, WeekId},
    single_effort_utils::sinlge_effort::Effort,
    workers_utils::worker::WorkerId,
};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct ProjectId(pub usize);

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct Enable(pub bool);

/// Default di `enable`: abilitato. Usato quando il campo non è serializzato
/// (è uno stato di filtro transitorio, ricalcolato da `closed` al caricamento).
fn enable_default() -> Enable {
    Enable(true)
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    info: String,
    // `enable` è lo stato del filtro "Progetti ▼": puramente di visualizzazione,
    // non va salvato né riletto dal file. Al load viene ricalcolato da `closed`.
    #[serde(skip, default = "enable_default")]
    enable: Enable,
    dev_id: HashMap<DevId, SingleDev>,
    #[serde(default)]
    pub start_week: Option<WeekId>,
    #[serde(default)]
    pub end_week: Option<WeekId>,
    #[serde(default)]
    tripletta: Option<String>,
    #[serde(default)]
    category: Option<CategoryId>,
    /// Posizione di ordinamento nella colonna sinistra. I file vecchi (senza
    /// campo) partono tutti da 0 → tie-break per ProjectId = ordine attuale.
    #[serde(default)]
    order: usize,
    /// Progetto chiuso/archiviato. Optional per retro-compatibilità con i file
    /// .ron privi del campo. Un progetto chiuso è automaticamente non-enabled.
    #[serde(default)]
    closed: Option<bool>,
    /// Milestone collocate nel progetto: id milestone → settimana. La chiave
    /// garantisce che la stessa milestone non compaia più volte nel progetto.
    #[serde(default)]
    milestones: HashMap<MilestoneId, WeekId>,
}

impl Project {
    pub fn new(info: &str) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
            enable: Enable(true),
            start_week: None,
            end_week: None,
            tripletta: None,
            category: None,
            order: 0,
            closed: None,
            milestones: HashMap::new(),
        }
    }

    pub fn new_with_start(info: &str, start_week: Option<WeekId>) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
            enable: Enable(true),
            start_week,
            end_week: None,
            tripletta: None,
            category: None,
            order: 0,
            closed: None,
            milestones: HashMap::new(),
        }
    }

    pub fn get_start_week(&self) -> Option<WeekId> {
        self.start_week
    }
    pub fn set_start_week(&mut self, week: Option<WeekId>) {
        self.start_week = week;
    }
    pub fn get_end_week(&self) -> Option<WeekId> {
        self.end_week
    }
    pub fn set_end_week(&mut self, week: Option<WeekId>) {
        self.end_week = week;
    }

    pub fn del_row(&mut self, id_dev: DevId) {
        self.dev_id.get_mut(&id_dev).unwrap().del_row();
    }

    pub fn reset_effort(&mut self, id_dev: DevId, week: WeekId) {
        self.dev_id.get_mut(&id_dev).unwrap().reset_effort(week);
    }

    pub fn set_info(&mut self, info: &str) {
        self.info = info.to_string();
    }

    pub fn add_dev(&mut self, id_dev: DevId) {
        self.dev_id.entry(id_dev).or_insert_with(SingleDev::new);
        // if !self.dev_id.contains_key(&id_dev) {
        //     self.dev_id.insert(id_dev, SingleDev::new());
        // }
    }

    pub fn add_dev_effort(&mut self, id_dev: DevId, effort: Effort) {
        self.add_dev(id_dev);
        self.dev_id.get_mut(&id_dev).unwrap().set_effort(effort);
    }

    pub fn set_dev_note(&mut self, id_dev: DevId, note: &str) {
        if let Some(sd) = self.dev_id.get_mut(&id_dev) {
            sd.set_dev_note(note);
        }
    }

    pub fn set_note(&mut self, id_dev: DevId, week: WeekId, id_worker: WorkerId, note: &str) {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .set_note(week, id_worker, note);
    }

    pub fn add_effort(&mut self, id_dev: DevId, week: WeekId, id_worker: WorkerId, effort: Effort) {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .add(week, id_worker, effort);
    }

    pub fn del_dev(&mut self, id_dev: DevId) {
        self.dev_id.remove(&id_dev);
    }

    pub fn list_dev_id(&self) -> Vec<DevId> {
        let mut ids: Vec<DevId> = self.dev_id.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn get_week_with_max_worker(&self, id_dev: DevId) -> Option<WeekId> {
        self.dev_id.get(&id_dev).unwrap().get_week_with_max_worker()
    }

    pub fn get_tripletta(&self) -> Option<&str> {
        self.tripletta.as_deref()
    }

    pub fn set_tripletta(&mut self, tripletta: &str) {
        self.tripletta = if tripletta.is_empty() { None } else { Some(tripletta.to_string()) };
    }

    pub fn get_category(&self) -> Option<CategoryId> {
        self.category
    }

    pub fn set_category(&mut self, category: Option<CategoryId>) {
        self.category = category;
    }

    pub fn get_info(&self) -> String {
        self.info.clone()
    }

    pub fn get_order(&self) -> usize {
        self.order
    }

    pub fn set_order(&mut self, order: usize) {
        self.order = order;
    }

    pub fn get_enable(&self) -> Enable {
        self.enable
    }

    pub fn set_enable(&mut self, enable: Enable) {
        self.enable = enable;
    }

    pub fn is_closed(&self) -> bool {
        self.closed.unwrap_or(false)
    }

    /// Imposta lo stato "chiuso". Chiudere un progetto lo rende anche
    /// automaticamente non-enabled; riaprirlo lo rende di nuovo enabled.
    pub fn set_closed(&mut self, closed: bool) {
        self.closed = if closed { Some(true) } else { None };
        self.enable = Enable(!closed);
    }

    pub fn get_keys(
        &self,
    ) -> std::iter::Cloned<std::collections::hash_map::Keys<'_, DevId, SingleDev>> {
        self.dev_id.keys().cloned()
    }

    pub fn get_dev_id(&self, dev_id: &DevId) -> std::option::Option<&SingleDev> {
        self.dev_id.get(dev_id)
    }

    pub fn set_dev_hide_effort(&mut self, id_dev: DevId, hide: bool) {
        if let Some(sd) = self.dev_id.get_mut(&id_dev) {
            sd.set_hide_effort(hide);
        }
    }

    pub fn get_dev_hide_effort(&self, id_dev: DevId) -> bool {
        self.dev_id.get(&id_dev).map(|sd| sd.get_hide_effort()).unwrap_or(false)
    }

    /// Colloca (o sposta) una milestone in una settimana. La stessa milestone
    /// non può comparire più volte: se già presente, ne aggiorna la settimana.
    pub fn add_milestone(&mut self, id: MilestoneId, week: WeekId) {
        self.milestones.insert(id, week);
    }

    pub fn remove_milestone(&mut self, id: MilestoneId) {
        self.milestones.remove(&id);
    }

    pub fn has_milestone(&self, id: MilestoneId) -> bool {
        self.milestones.contains_key(&id)
    }

    /// Rimuove ogni collocazione della milestone (usato quando viene eliminata
    /// globalmente).
    pub fn purge_milestone(&mut self, id: MilestoneId) {
        self.milestones.remove(&id);
    }

    /// Elenco (milestone, settimana) collocate nel progetto.
    pub fn list_milestones(&self) -> Vec<(MilestoneId, WeekId)> {
        let mut v: Vec<(MilestoneId, WeekId)> =
            self.milestones.iter().map(|(&id, &w)| (id, w)).collect();
        v.sort_by_key(|(id, _)| *id);
        v
    }

    /// Milestone collocate nella settimana indicata (ordinate per id).
    pub fn milestones_at_week(&self, week: WeekId) -> Vec<MilestoneId> {
        let mut v: Vec<MilestoneId> = self
            .milestones
            .iter()
            .filter(|(_, w)| **w == week)
            .map(|(&id, _)| id)
            .collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_not_serialized_and_defaults_true() {
        let p = Project::new("demo");
        let s = ron::ser::to_string(&p).unwrap();
        assert!(!s.contains("enable"), "enable non deve essere serializzato: {s}");

        // File "vecchio" che contiene ancora enable:(false): il campo (skip) va
        // ignorato e il valore ripristinato al default (abilitato).
        let old = s.replacen('(', "(enable:(false),", 1);
        let back: Project = ron::from_str(&old).expect("il vecchio formato deve caricarsi");
        assert!(back.get_enable().0, "enable deve tornare al default (abilitato)");
    }
}
