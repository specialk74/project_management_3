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

    pub fn add(&mut self, name: &str) -> usize {
        match self
            .dev_id
            .iter_mut()
            .find(|(_, dev)| dev.get_name() == name)
        {
            Some((id, dev)) => {
                dev.set_name(name);
                *id
            }
            None => {
                let id_dev = self.last_id;
                self.dev_id.insert(self.last_id, Dev::new(name));
                self.last_id += 1;
                id_dev
            }
        }
    }
}
