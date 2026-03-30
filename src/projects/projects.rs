#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    dev_utils::dev::DevId,
    projects::project::{Enable, Project, ProjectId},
    single_dev::single_dev::{SingleDev, WeekId},
    single_efforts::sinlge_effort::Effort,
    workers::worker::WorkerId,
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
        items.sort_by_key(|(id, _)| *id);
        items
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
        items.sort_by_key(|(id, _, _)| *id);
        items
    }

    pub fn set_enable(&mut self, project_id: ProjectId, enable: Enable) {
        if let Some(p) = self.projects.get_mut(&project_id) {
            p.set_enable(enable);
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
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .set_note(id_dev, week, id_worker, note);
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
}
