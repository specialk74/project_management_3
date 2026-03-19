use serde::{Deserialize, Serialize};
use std::fs;

use crate::devs::Devs;
use crate::projects::Projects;
use crate::workers::Workers;

pub const SAVE_PATH: &str = "workers.ron";

#[derive(Serialize, Deserialize)]
pub struct App {
    pub start_week: usize,
    pub workers: Workers,
    pub devs: Devs,
    pub projects: Projects,
}

impl App {
    pub fn new() -> Self {
        Self {
            start_week: 1,
            workers: Workers::new(),
            devs: Devs::new(),
            projects: Projects::new(),
        }
    }

    pub fn save(&self, path: &str) {
        let content =
            ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap();
        let _ = fs::write(path, content);
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(ron::from_str(&content)?)
    }
}
