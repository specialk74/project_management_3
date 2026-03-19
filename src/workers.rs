use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const WORKER_ID_ZERO: WorkerId = WorkerId(0);

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct WorkerId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct Worker {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<usize>,
}

impl Worker {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bg_color: None,
            font_color: None,
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_bg_color(&mut self, color: Option<usize>) {
        self.bg_color = color;
    }
    pub fn set_font_color(&mut self, color: Option<usize>) {
        self.font_color = color;
    }
}

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
            .map(|(&id, w)| (id, w.name.clone()))
            .collect();
        items.sort_by_key(|(id, _)| id.0);
        items
    }
}
