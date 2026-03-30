use serde::{Deserialize, Serialize};
use std::fs;

use crate::{
    date_utils::dates::get_default_weeks, dev_utils::devs::Devs, projects::projects::Projects,
    single_dev::single_dev::WeekId, workers::workers::Workers,
};

pub const SAVE_PATH: &str = "workers.ron";

#[derive(Serialize, Deserialize)]
pub struct App {
    pub start_week: WeekId,
    #[serde(skip)]
    pub end_week: WeekId,
    #[serde(skip)]
    pub n_week: WeekId,
    pub workers: Workers,
    pub devs: Devs,
    pub projects: Projects,
    pub holidays: Vec<WeekId>,
}

impl App {
    pub fn new() -> Self {
        let (n_week, start, end) = get_default_weeks(None);
        println!("new -> {} {} {}", n_week, start, end);
        Self {
            start_week: WeekId(start as usize),
            end_week: WeekId(end as usize),
            n_week: WeekId(n_week as usize),
            workers: Workers::new(),
            devs: Devs::new(),
            projects: Projects::new(),
            holidays: Vec::new(),
        }
    }

    pub fn save(&self, path: &str) {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap();
        let _ = fs::write(path, content);
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let mut app: App = ron::from_str(&content)?;

        let (n_week, start_week, end_week) = get_default_weeks(Some(app.start_week.0 as i32));

        app.n_week.0 = n_week as usize;
        app.start_week.0 = start_week as usize;
        app.end_week.0 = end_week as usize;
        Ok(app)
    }
}
