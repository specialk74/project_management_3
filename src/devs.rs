use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Dev {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bg_color: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_color: Option<usize>,
}

impl Dev {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bg_color: None,
            font_color: None,
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    pub fn set_bg_color(&mut self, color: Option<usize>) {
        self.bg_color = color;
    }

    pub fn set_font_color(&mut self, color: Option<usize>) {
        self.font_color = color;
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
}

#[derive(Serialize, Deserialize)]
pub struct Devs {
    last_id: usize,
    dev_id: HashMap<usize, Dev>,
}

impl Devs {
    pub fn new() -> Self {
        Self {
            last_id: 0,
            dev_id: HashMap::new(),
        }
    }

    pub fn del(&mut self, id: usize) {
        self.dev_id.remove(&id);
    }

    pub fn list(&self) -> Vec<(usize, String)> {
        let mut items: Vec<(usize, String)> = self
            .dev_id
            .iter()
            .map(|(&id, d)| (id, d.name.clone()))
            .collect();
        items.sort_by_key(|(id, _)| *id);
        items
    }

    pub fn add(&mut self, name: &str) -> usize {
        if let Some((&id, _)) = self.dev_id.iter().find(|(_, dev)| dev.get_name() == name) {
            return id;
        }
        let id = self.last_id;
        self.dev_id.insert(id, Dev::new(name));
        self.last_id += 1;
        id
    }
}
