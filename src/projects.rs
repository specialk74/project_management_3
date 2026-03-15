use crate::single_dev::SingleDev;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Project {
    info: String,
    dev_id: HashMap<usize, SingleDev>,
}

impl Project {
    pub fn new(info: &str) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
        }
    }

    pub fn set_info(&mut self, info: &str) {
        self.info = info.to_string();
    }

    pub fn add_dev(&mut self, id_dev: usize) {
        if !self.dev_id.contains_key(&id_dev) {
            self.dev_id.insert(id_dev, SingleDev::new());
        }
    }

    pub fn add_dev_effort(&mut self, id_dev: usize, effort: usize) {
        self.add_dev(id_dev);
        self.dev_id.get_mut(&id_dev).unwrap().set_effort(effort);
    }

    pub fn set_note(&mut self, id_dev: usize, week: usize, id_worker: usize, note: &str) {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .set_note(week, id_worker, note);
    }

    pub fn add_effort(&mut self, id_dev: usize, week: usize, id_worker: usize, effort: usize) {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .add(week, id_worker, effort);
    }

    pub fn del_dev(&mut self, id_dev: usize) {
        self.dev_id.remove(&id_dev);
    }
}

#[derive(Serialize, Deserialize)]
pub struct Projects {
    last_id: usize,
    projects: HashMap<usize, Project>,
}

impl Projects {
    pub fn new() -> Self {
        Self {
            last_id: 0,
            projects: HashMap::new(),
        }
    }

    pub fn add(&mut self, info: &str) -> usize {
        let id = self.last_id;
        self.projects.insert(id, Project::new(info));
        self.last_id += 1;
        id
    }

    pub fn del(&mut self, id_project: usize) {
        self.projects.remove(&id_project);
    }

    pub fn add_dev(&mut self, id_project: usize, id_dev: usize) {
        self.projects.get_mut(&id_project).unwrap().add_dev(id_dev);
    }

    pub fn set_note(
        &mut self,
        id_project: usize,
        id_dev: usize,
        week: usize,
        id_worker: usize,
        note: &str,
    ) {
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .set_note(id_dev, week, id_worker, note);
    }

    pub fn add_effort(
        &mut self,
        id_project: usize,
        id_dev: usize,
        week: usize,
        id_worker: usize,
        effort: usize,
    ) {
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .add_effort(id_dev, week, id_worker, effort);
    }

    pub fn add_dev_effort(&mut self, id_project: usize, id_dev: usize, effort: usize) {
        self.projects
            .get_mut(&id_project)
            .unwrap()
            .add_dev_effort(id_dev, effort);
    }
}
