use crate::{
    devs::DevId,
    single_dev::{SingleDev, WeekId},
    sinlge_effort::Effort,
    workers::WorkerId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct ProjectId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct Project {
    info: String,
    visible: bool,
    enable: bool,
    dev_id: HashMap<DevId, SingleDev>,
}

impl Project {
    pub fn new(info: &str) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
            visible: true,
            enable: true,
        }
    }

    pub fn set_info(&mut self, info: &str) {
        self.info = info.to_string();
    }

    pub fn add_dev(&mut self, id_dev: DevId) {
        if !self.dev_id.contains_key(&id_dev) {
            self.dev_id.insert(id_dev, SingleDev::new());
        }
    }

    pub fn add_dev_effort(&mut self, id_dev: DevId, effort: Effort) {
        self.add_dev(id_dev);
        self.dev_id.get_mut(&id_dev).unwrap().set_effort(effort);
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
        self.dev_id.keys().copied().collect()
    }
}

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
            .map(|(&id, p)| (id, p.info.clone()))
            .collect();
        items.sort_by_key(|(id, _)| *id);
        items
    }

    /// Returns (id, name, visible, enable)
    pub fn list_full(&self) -> Vec<(ProjectId, String, bool, bool)> {
        let mut items: Vec<(ProjectId, String, bool, bool)> = self
            .projects
            .iter()
            .map(|(&id, p)| (id, p.info.clone(), p.visible, p.enable))
            .collect();
        items.sort_by_key(|(id, _, _, _)| *id);
        items
    }

    pub fn set_visible(&mut self, project_id: ProjectId, visible: bool) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.visible = visible;
        }
    }

    pub fn set_enable(&mut self, project_id: ProjectId, enable: bool) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.enable = enable;
        }
    }

    pub fn add(&mut self, info: &str) -> ProjectId {
        let id = self.last_id;
        self.projects.insert(id, Project::new(info));
        self.last_id.0 += 1;
        id
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

    pub fn set_note(
        &mut self,
        id_project: ProjectId,
        id_dev: DevId,
        week: WeekId,
        id_worker: WorkerId,
        note: &str,
    ) {
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .set_note(id_dev, week, id_worker, note);
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

    pub fn get_dev_ids(&self, project_id: ProjectId) -> Vec<DevId> {
        self.projects
            .get(&project_id)
            .map(|p| {
                let mut ids: Vec<DevId> = p.dev_id.keys().cloned().collect();
                ids.sort();
                ids
            })
            .unwrap_or_default()
    }

    pub fn get_single_dev(&self, project_id: ProjectId, dev_id: DevId) -> Option<&SingleDev> {
        self.projects.get(&project_id)?.dev_id.get(&dev_id)
    }
}
