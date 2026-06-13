#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    categories::CategoryId,
    dev_utils::dev::DevId,
    project_utils::project::{Enable, Project, ProjectId},
    single_dev_utils::single_dev::{SingleDev, WeekId},
    single_effort_utils::sinlge_effort::Effort,
    workers_utils::worker::WorkerId,
};

#[derive(Serialize, Deserialize)]
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

    pub fn get_enable(&self, project_id: &ProjectId) -> Enable {
        self.projects
            .get(project_id)
            .map_or(Enable(false), |f| f.get_enable())
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

    pub fn clear_category_from_all(&mut self, category_id: CategoryId) {
        for project in self.projects.values_mut() {
            if project.get_category() == Some(category_id) {
                project.set_category(None);
            }
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
