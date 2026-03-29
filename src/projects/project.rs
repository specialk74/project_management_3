#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    devs::dev::DevId,
    single_dev::single_dev::{SingleDev, WeekId},
    single_efforts::sinlge_effort::Effort,
    workers::worker::WorkerId,
};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct ProjectId(pub usize);

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct Enable(pub bool);

#[derive(Serialize, Deserialize)]
pub struct Project {
    info: String,
    enable: Enable,
    dev_id: HashMap<DevId, SingleDev>,
}

impl Project {
    pub fn new(info: &str) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
            enable: Enable(true),
        }
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
        self.dev_id.keys().copied().collect()
    }

    pub fn get_week_with_max_worker(&self, id_dev: DevId) -> Option<WeekId> {
        self.dev_id.get(&id_dev).unwrap().get_week_with_max_worker()
    }

    pub fn get_info(&self) -> String {
        self.info.clone()
    }

    pub fn get_enable(&self) -> Enable {
        self.enable
    }

    pub fn set_enable(&mut self, enable: Enable) {
        self.enable = enable;
    }

    pub fn get_keys(
        &self,
    ) -> std::iter::Cloned<std::collections::hash_map::Keys<'_, DevId, SingleDev>> {
        self.dev_id.keys().cloned()
    }

    pub fn get_dev_id(&self, dev_id: &DevId) -> std::option::Option<&SingleDev> {
        self.dev_id.get(&dev_id)
    }
}
