#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    categories::CategoryId,
    dev_utils::dev::DevId,
    milestones::MilestoneId,
    project_utils::project::{Enable, OverflowResolution, Project, ProjectId},
    single_dev_utils::single_dev::{SingleDev, WeekId},
    single_effort_utils::sinlge_effort::Effort,
    workers_utils::worker::WorkerId,
};

#[derive(Serialize, Deserialize, Clone)]
pub struct Projects {
    last_id: ProjectId,
    projects: HashMap<ProjectId, Project>,
}

impl Projects {
    pub fn new() -> Self {
        Self {
            last_id: ProjectId(0),
            projects: HashMap::new(),
        }
    }

    pub fn list(&self) -> Vec<(ProjectId, String)> {
        let mut items: Vec<(ProjectId, String)> = self
            .projects
            .iter()
            .map(|(&id, p)| (id, p.get_info()))
            .collect();
        items.sort_by_key(|(id, _)| (self.order_of(*id), *id));
        items
    }

    /// Posizione di ordinamento del progetto (0 se assente).
    fn order_of(&self, id: ProjectId) -> usize {
        self.projects.get(&id).map_or(0, |p| p.get_order())
    }

    /// ProjectId nell'ordine di visualizzazione corrente (order, poi id).
    fn ordered_ids(&self) -> Vec<ProjectId> {
        let mut ids: Vec<ProjectId> = self.projects.keys().copied().collect();
        ids.sort_by_key(|id| (self.order_of(*id), *id));
        ids
    }

    /// Riassegna `order` = posizione, normalizzando i valori sull'ordine dato.
    fn reindex(&mut self, ids: &[ProjectId]) {
        for (pos, id) in ids.iter().enumerate() {
            if let Some(p) = self.projects.get_mut(id) {
                p.set_order(pos);
            }
        }
    }

    /// Sposta il progetto di una posizione verso l'alto. Ritorna `true` se ha
    /// effettivamente cambiato l'ordine (no-op se già in cima).
    pub fn move_up(&mut self, id: ProjectId) -> bool {
        let mut ids = self.ordered_ids();
        match ids.iter().position(|&p| p == id) {
            Some(pos) if pos > 0 => {
                ids.swap(pos, pos - 1);
                self.reindex(&ids);
                true
            }
            _ => false,
        }
    }

    /// Sposta il progetto di una posizione verso il basso. Ritorna `true` se ha
    /// effettivamente cambiato l'ordine (no-op se già in fondo).
    pub fn move_down(&mut self, id: ProjectId) -> bool {
        let mut ids = self.ordered_ids();
        match ids.iter().position(|&p| p == id) {
            Some(pos) if pos + 1 < ids.len() => {
                ids.swap(pos, pos + 1);
                self.reindex(&ids);
                true
            }
            _ => false,
        }
    }

    pub fn del_row(&mut self, project_id: ProjectId, id_dev: DevId) {
        self.projects.get_mut(&project_id).unwrap().del_row(id_dev);
    }

    /// Returns (id, name, visible, enable)
    pub fn list_full(&self) -> Vec<(ProjectId, String, Enable)> {
        let mut items: Vec<(ProjectId, String, Enable)> = self
            .projects
            .iter()
            .map(|(&id, p)| (id, p.get_info(), p.get_enable()))
            .collect();
        items.sort_by_key(|(id, _, _)| (self.order_of(*id), *id));
        items
    }

    pub fn set_enable(&mut self, project_id: ProjectId, enable: Enable) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_enable(enable);
        }
    }

    pub fn add(&mut self, info: &str, tripletta: Option<&str>, start_week: Option<WeekId>) -> ProjectId {
        let id = self.last_id;
        let mut project = Project::new_with_start(info, start_week);
        if let Some(t) = tripletta {
            project.set_tripletta(t);
        }
        // In fondo all'ordinamento corrente.
        let next_order = self.projects.values().map(|p| p.get_order()).max().map_or(0, |m| m + 1);
        project.set_order(next_order);
        self.projects.insert(id, project);
        self.last_id.0 += 1;
        id
    }

    pub fn get_info(&self, project_id: ProjectId) -> String {
        self.projects
            .get(&project_id)
            .map(|p| p.get_info())
            .unwrap_or_default()
    }

    pub fn get_tripletta(&self, project_id: ProjectId) -> String {
        self.projects
            .get(&project_id)
            .and_then(|p| p.get_tripletta())
            .unwrap_or("")
            .to_string()
    }

    pub fn min_start_week(&self) -> Option<WeekId> {
        self.projects
            .values()
            .filter_map(|p| p.get_start_week())
            .min()
    }

    pub fn get_project_start_week(&self, id: ProjectId) -> Option<WeekId> {
        self.projects.get(&id)?.get_start_week()
    }

    pub fn get_project_end_week(&self, id: ProjectId) -> Option<WeekId> {
        self.projects.get(&id)?.get_end_week()
    }

    pub fn set_project_end_week(&mut self, id: ProjectId, week: Option<WeekId>) {
        if let Some(p) = self.projects.get_mut(&id) {
            p.set_end_week(week);
        }
    }

    pub fn set_project_start_week(&mut self, id: ProjectId, week: Option<WeekId>) {
        if let Some(p) = self.projects.get_mut(&id) {
            p.set_start_week(week);
        }
    }

    pub fn del(&mut self, id_project: ProjectId) {
        self.projects.remove(&id_project);
    }

    pub fn add_project_milestone(&mut self, id: ProjectId, mid: MilestoneId, week: WeekId) {
        if let Some(p) = self.projects.get_mut(&id) {
            p.add_milestone(mid, week);
        }
    }

    pub fn remove_project_milestone(&mut self, id: ProjectId, mid: MilestoneId) {
        if let Some(p) = self.projects.get_mut(&id) {
            p.remove_milestone(mid);
        }
    }

    /// Rimuove la milestone da tutti i progetti (chiamata quando viene eliminata
    /// globalmente).
    pub fn purge_milestone(&mut self, mid: MilestoneId) {
        for p in self.projects.values_mut() {
            p.purge_milestone(mid);
        }
    }

    pub fn list_project_milestones(&self, id: ProjectId) -> Vec<(MilestoneId, WeekId)> {
        self.projects.get(&id).map(|p| p.list_milestones()).unwrap_or_default()
    }

    pub fn project_milestones_at_week(&self, id: ProjectId, week: WeekId) -> Vec<MilestoneId> {
        self.projects
            .get(&id)
            .map(|p| p.milestones_at_week(week))
            .unwrap_or_default()
    }

    /// Dev del progetto con almeno una settimana di effort.
    pub fn devs_with_effort(&self, id: ProjectId) -> Vec<DevId> {
        self.projects.get(&id).map(|p| p.devs_with_effort()).unwrap_or_default()
    }

    /// Blocco contiguo di settimane con effort di un dev, attorno a `week`.
    pub fn dev_contiguous_block(&self, id: ProjectId, dev: DevId, week: WeekId) -> Vec<WeekId> {
        self.projects
            .get(&id)
            .map(|p| p.dev_contiguous_block(dev, week))
            .unwrap_or_default()
    }

    /// Tutte le settimane con effort di un dev del progetto.
    pub fn dev_effort_weeks(&self, id: ProjectId, dev: DevId) -> Vec<WeekId> {
        self.projects
            .get(&id)
            .map(|p| p.dev_effort_weeks(dev))
            .unwrap_or_default()
    }

    /// Lato di sforamento (`Some(true)`=fine, `Some(false)`=inizio) per uno
    /// spostamento di `delta_weeks`, oppure `None` se non sfora.
    pub fn move_overflow_side(
        &self,
        id: ProjectId,
        moves: &[(DevId, Vec<WeekId>)],
        delta_weeks: i64,
    ) -> Option<bool> {
        self.projects
            .get(&id)
            .and_then(|p| p.move_overflow_side(moves, delta_weeks))
    }

    /// Sposta blocchi di effort (ed eventuali milestone) di `delta_weeks`.
    pub fn move_effort(
        &mut self,
        id: ProjectId,
        moves: &[(DevId, Vec<WeekId>)],
        delta_weeks: i64,
        milestones: &[MilestoneId],
        resolution: OverflowResolution,
    ) {
        if let Some(p) = self.projects.get_mut(&id) {
            p.move_effort(moves, delta_weeks, milestones, resolution);
        }
    }

    pub fn add_dev(&mut self, id_project: ProjectId, id_dev: DevId) {
        self.projects.get_mut(&id_project).unwrap().add_dev(id_dev);
    }

    pub fn del_dev(&mut self, id_project: ProjectId, id_dev: DevId) {
        self.projects.get_mut(&id_project).unwrap().del_dev(id_dev);
    }

    pub fn list_devs(&self, id_project: ProjectId) -> Vec<DevId> {
        self.projects.get(&id_project).unwrap().list_dev_id()
    }

    pub fn set_dev_note(&mut self, id_project: ProjectId, id_dev: DevId, note: &str) {
        if let Some(p) = self.projects.get_mut(&id_project) {
            p.set_dev_note(id_dev, note);
        }
    }

    pub fn set_note(
        &mut self,
        id_project: ProjectId,
        id_dev: DevId,
        week: WeekId,
        id_worker: WorkerId,
        note: &str,
    ) {
        if let Some(p) = self.projects.get_mut(&id_project) {
            p.set_note(id_dev, week, id_worker, note);
        }
    }

    pub fn reset_effort(&mut self, id_project: ProjectId, id_dev: DevId, week: WeekId) {
        if let Some(p) = self.projects.get_mut(&id_project) {
            p.reset_effort(id_dev, week);
        }
    }

    pub fn add_effort(
        &mut self,
        id_project: ProjectId,
        id_dev: DevId,
        week: WeekId,
        id_worker: WorkerId,
        effort: Effort,
    ) {
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .add_effort(id_dev, week, id_worker, effort);
    }

    pub fn add_dev_effort(&mut self, id_project: ProjectId, id_dev: DevId, effort: Effort) {
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .add_dev_effort(id_dev, effort);
    }

    /// Registra la % dichiarata dal dev per la settimana `week` (storico).
    /// Ritorna `true` se lo storico è cambiato.
    pub fn set_dev_declared_pct(
        &mut self,
        id_project: ProjectId,
        id_dev: DevId,
        week: WeekId,
        pct: u8,
    ) -> bool {
        self.projects
            .get_mut(&id_project)
            .is_some_and(|p| p.set_dev_declared_pct(id_dev, week, pct))
    }

    /// Avanzamento complessivo del progetto (earned value). Vedi
    /// `Project::progress_pct`. `None` se il progetto non esiste o non ha
    /// alcun effort pianificato.
    pub fn project_progress_pct(&self, id: ProjectId) -> Option<u8> {
        self.projects.get(&id).and_then(|p| p.progress_pct())
    }

    /// Avanzamento "presunto" del progetto (budget consumato fino a `today`).
    /// Vedi `Project::presumed_progress_pct`.
    pub fn project_presumed_progress_pct(&self, id: ProjectId, today: WeekId) -> Option<u32> {
        self.projects
            .get(&id)
            .and_then(|p| p.presumed_progress_pct(today))
    }

    /// Numeri che compongono le percentuali di avanzamento (per il tooltip):
    /// `(usato_fino_a_today, pianificato, Σ pianificato·dichiarata)`.
    pub fn project_progress_breakdown(&self, id: ProjectId, today: WeekId) -> Option<(u64, u64, u64)> {
        self.projects.get(&id).and_then(|p| p.progress_breakdown(today))
    }

    pub fn set_project_info(&mut self, project_id: ProjectId, info: &str) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_info(info);
        }
    }

    pub fn set_tripletta(&mut self, project_id: ProjectId, tripletta: &str) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_tripletta(tripletta);
        }
    }

    pub fn get_notes(&self, project_id: ProjectId) -> HashMap<WeekId, String> {
        self.projects
            .get(&project_id)
            .map(|p| p.get_notes().clone())
            .unwrap_or_default()
    }

    pub fn set_notes(&mut self, project_id: ProjectId, notes: HashMap<WeekId, String>) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_notes(notes);
        }
    }

    pub fn get_dev_ids(&self, project_id: ProjectId) -> Vec<DevId> {
        self.projects
            .get(&project_id)
            .map(|p| {
                let mut ids: Vec<DevId> = p.get_keys().collect();
                ids.sort();
                ids
            })
            .unwrap_or_default()
    }

    pub fn get_single_dev(&self, project_id: ProjectId, dev_id: DevId) -> Option<&SingleDev> {
        self.projects.get(&project_id)?.get_dev_id(&dev_id)
    }

    /// Accesso al progetto (per il merge a 3 vie).
    pub fn get(&self, id: ProjectId) -> Option<&Project> {
        self.projects.get(&id)
    }

    /// Tutti i ProjectId presenti (non ordinati).
    pub fn ids(&self) -> Vec<ProjectId> {
        self.projects.keys().copied().collect()
    }

    /// Inserisce o sostituisce un progetto (usato dal merge).
    pub fn set_project(&mut self, id: ProjectId, project: Project) {
        self.projects.insert(id, project);
    }

    pub fn get_enable(&self, project_id: &ProjectId) -> Enable {
        self.projects
            .get(project_id)
            .map_or(Enable(false), |f| f.get_enable())
    }

    pub fn is_closed(&self, project_id: ProjectId) -> bool {
        self.projects
            .get(&project_id)
            .map_or(false, |p| p.is_closed())
    }

    /// Ricalcola lo stato `enable` (filtro) di tutti i progetti a partire da
    /// `closed`: aperti = abilitati, chiusi = disabilitati. Chiamato dopo il
    /// caricamento, dato che `enable` non è persistito.
    pub fn reset_enable_from_closed(&mut self) {
        for p in self.projects.values_mut() {
            p.set_enable(Enable(!p.is_closed()));
        }
    }

    pub fn set_closed(&mut self, project_id: ProjectId, closed: bool) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_closed(closed);
        }
    }

    pub fn get_week_with_max_worker(&self, project_id: ProjectId, id_dev: DevId) -> Option<WeekId> {
        self.projects
            .get(&project_id)?
            .get_week_with_max_worker(id_dev)
    }

    pub fn set_dev_hide_effort(&mut self, id_project: ProjectId, id_dev: DevId, hide: bool) {
        if let Some(p) = self.projects.get_mut(&id_project) {
            p.set_dev_hide_effort(id_dev, hide);
        }
    }

    pub fn get_category(&self, project_id: ProjectId) -> Option<CategoryId> {
        self.projects.get(&project_id)?.get_category()
    }

    pub fn set_category(&mut self, project_id: ProjectId, category: Option<CategoryId>) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_category(category);
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(ps: &Projects) -> Vec<String> {
        ps.list().into_iter().map(|(_, n)| n).collect()
    }

    fn three() -> (Projects, ProjectId, ProjectId, ProjectId) {
        let mut ps = Projects::new();
        let a = ps.add("A", None, None);
        let b = ps.add("B", None, None);
        let c = ps.add("C", None, None);
        (ps, a, b, c)
    }

    #[test]
    fn list_default_order_is_insertion() {
        let (ps, ..) = three();
        assert_eq!(names(&ps), vec!["A", "B", "C"]);
    }

    #[test]
    fn move_up_swaps_with_previous() {
        let (mut ps, _, _, c) = three();
        assert!(ps.move_up(c));
        assert_eq!(names(&ps), vec!["A", "C", "B"]);
    }

    #[test]
    fn move_down_swaps_with_next() {
        let (mut ps, a, ..) = three();
        assert!(ps.move_down(a));
        assert_eq!(names(&ps), vec!["B", "A", "C"]);
    }

    #[test]
    fn move_up_at_top_is_noop() {
        let (mut ps, a, ..) = three();
        assert!(!ps.move_up(a));
        assert_eq!(names(&ps), vec!["A", "B", "C"]);
    }

    #[test]
    fn move_down_at_bottom_is_noop() {
        let (mut ps, _, _, c) = three();
        assert!(!ps.move_down(c));
        assert_eq!(names(&ps), vec!["A", "B", "C"]);
    }

    #[test]
    fn new_project_appended_after_reorder() {
        let (mut ps, a, ..) = three();
        ps.move_down(a); // B, A, C
        let d = ps.add("D", None, None);
        assert_eq!(names(&ps), vec!["B", "A", "C", "D"]);
    }
}

