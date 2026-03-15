mod devs;
mod projects;
mod single_dev;
mod sinlge_effort;
mod workers;

use serde::{Deserialize, Serialize};

use crate::devs::Devs;
use crate::projects::Projects;
use crate::workers::Workers;
use std::fs;

#[derive(Serialize, Deserialize)]
struct App {
    workers: Workers,
    devs: Devs,
    projects: Projects,
}

impl App {
    fn save(&self, path: &str) {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .unwrap();
        let _ = fs::write(path, content);
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(ron::from_str(&content)?)
    }
}

fn main() {
    let mut app = App {
        workers: Workers::new(),
        devs: Devs::new(),
        projects: Projects::new(),
    };
}
