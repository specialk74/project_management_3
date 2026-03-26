#![allow(unused)]
#![allow(dead_code)]

use crate::workers::{
    worker::{WORKER_ID_ZERO, Worker, WorkerId},
    *,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Workers {
    last_id: WorkerId,
    worker_id: HashMap<WorkerId, Worker>,
}

impl Workers {
    pub fn new() -> Self {
        Self {
            last_id: WorkerId(1),
            worker_id: HashMap::from([(WORKER_ID_ZERO, Worker::new(""))]),
        }
    }

    pub fn get_id_by_name(&self, name: &str) -> Option<WorkerId> {
        self.worker_id
            .iter()
            .find(|(_, w)| w.name == name)
            .map(|(id, _)| *id)
    }

    pub fn get_name_by_id(&self, worker_id: WorkerId) -> &str {
        self.worker_id
            .get(&worker_id)
            .map(|s| s.name.as_str())
            .unwrap_or("")
    }

    pub fn add(&mut self, name: &str) -> WorkerId {
        self.get_id_by_name(name).unwrap_or_else(|| {
            let id = self.last_id;
            self.worker_id.insert(self.last_id, Worker::new(name));
            self.last_id.0 += 1;
            id
        })
    }

    pub fn del(&mut self, id: WorkerId) {
        self.worker_id.remove(&id);
    }

    pub fn list(&self) -> Vec<(WorkerId, String)> {
        let mut items: Vec<(WorkerId, String)> = self
            .worker_id
            .iter()
            .filter(|(w_id, _)| w_id != &&WORKER_ID_ZERO)
            .map(|(&id, w)| (id, w.name.clone()))
            .collect();
        items.sort_by_key(|(_, n)| n.clone());
        items
    }
}
