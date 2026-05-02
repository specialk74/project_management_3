use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

use crate::{
    date_utils::dates::get_default_weeks,
    dev_utils::devs::Devs,
    projects::projects::Projects,
    single_dev::single_dev::WeekId,
    single_efforts::sinlge_effort::Effort,
    workers::{worker::{WorkerId, DEFAULT_MAX_HOURS}, workers::Workers},
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
    #[serde(skip)]
    pub sovra: HashMap<(WeekId, WorkerId), Effort>,
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
            sovra: HashMap::new(),
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

    /// Global max hours for the worker at sorted index `idx` (None → 40).
    pub fn get_worker_max_hours_by_idx(&self, idx: usize) -> u32 {
        self.workers
            .list()
            .get(idx)
            .map(|(id, _)| self.workers.get_max_hours(*id))
            .unwrap_or(DEFAULT_MAX_HOURS)
    }

    /// Effective max hours (per-week override → global → 40) for worker at sorted index.
    pub fn get_effective_max_hours_by_idx(&self, idx: usize, week: usize) -> u32 {
        self.workers
            .list()
            .get(idx)
            .map(|(id, _)| self.workers.get_effective_max_hours(*id, week))
            .unwrap_or(DEFAULT_MAX_HOURS)
    }

    pub fn set_worker_max_hours_by_idx(&mut self, idx: usize, hours: u32) {
        let workers = self.workers.list();
        if let Some((id, _)) = workers.get(idx) {
            self.workers.set_max_hours(*id, hours);
        }
    }

    /// Applies a bulk week limit to all workers: min(hours, effective_current) wins.
    /// If hours >= 40, removes the week override for every worker (reset).
    pub fn set_bulk_week_limit(&mut self, week: usize, hours: u32) {
        let worker_ids: Vec<_> = self.workers.list().iter().map(|(id, _)| *id).collect();
        if hours >= DEFAULT_MAX_HOURS {
            for wid in worker_ids {
                let global_max = self.workers.get_max_hours(wid);
                self.workers.set_week_override(wid, week, global_max);
            }
        } else {
            for wid in worker_ids {
                let current_eff = self.workers.get_effective_max_hours(wid, week);
                let new_val = hours.min(current_eff);
                self.workers.set_week_override(wid, week, new_val);
            }
        }
    }

    pub fn set_worker_week_override_by_idx(&mut self, idx: usize, week: usize, hours: u32) {
        let workers = self.workers.list();
        if let Some((id, _)) = workers.get(idx) {
            self.workers.set_week_override(*id, week, hours);
        }
    }

    pub fn compute_sovra(&mut self) {
        self.sovra.clear();
        let projects = self.projects.list();
        let devs = self.devs.list();
        for (proj_id, _) in &projects {
            for (dev_id, _) in &devs {
                if let Some(sd) = self.projects.get_single_dev(*proj_id, *dev_id) {
                    for week in sd.get_weeks() {
                        if let Some(sew) = sd.get_all(week) {
                            for (worker_id, single_effort) in &sew.worker_id {
                                let e = self.sovra.entry((week, *worker_id)).or_insert(Effort(0));
                                e.0 += single_effort.get_effort().0;
                            }
                        }
                    }
                }
            }
        }
    }
}
