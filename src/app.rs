use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Write, path::Path};

use crate::{
    categories::Categories,
    date_utils::dates::get_default_weeks,
    dev_utils::devs::Devs,
    milestones::Milestones,
    project_utils::projects::Projects,
    single_dev_utils::single_dev::WeekId,
    single_effort_utils::sinlge_effort::Effort,
    workers_utils::{
        worker::{DEFAULT_MAX_HOURS, WorkerId},
        workers::Workers,
    },
};

pub const SAVE_PATH: &str = "workers.ron";

fn default_projects() -> Projects {
    Projects::new()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct App {
    pub start_week: WeekId,
    #[serde(skip)]
    pub end_week: WeekId,
    #[serde(skip)]
    pub n_week: WeekId,
    pub workers: Workers,
    pub devs: Devs,
    #[serde(default = "default_projects")]
    pub projects: Projects,
    #[serde(default)]
    pub categories: Categories,
    #[serde(default)]
    pub milestones: Milestones,
    #[serde(default)]
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
            categories: Categories::new(),
            milestones: Milestones::new(),
            holidays: Vec::new(),
            sovra: HashMap::new(),
        }
    }

    pub fn save(&self, path: &str) {
        let content = match ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Serializzazione RON fallita: {e}");
                return;
            }
        };
        if let Err(e) = Self::write_atomic(path, content.as_bytes()) {
            eprintln!("Errore salvataggio '{path}': {e}");
        }
    }

    /// Scrittura **atomica**: scrive su un file temporaneo nella stessa cartella,
    /// forza il flush su disco (`fsync`) e poi rinomina sul nome finale. Il
    /// `rename` sullo stesso filesystem è atomico, quindi il file finale è sempre
    /// completo: se due salvataggi si sovrappongono (es. cartella condivisa) non
    /// si ottiene mai un file scritto a metà. Il temporaneo include il PID per non
    /// collidere tra istanze/colleghi diversi.
    fn write_atomic(path: &str, bytes: &[u8]) -> std::io::Result<()> {
        let final_path = Path::new(path);
        let dir = final_path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let stem = final_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| SAVE_PATH.to_string());
        let tmp = dir.join(format!(".{stem}.{}.tmp", std::process::id()));

        // Scrivi tutto il contenuto e forzalo su disco prima di rinominare.
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        // Rimpiazzo atomico del file finale (rename sostituisce anche su Windows).
        match fs::rename(&tmp, final_path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = fs::remove_file(&tmp); // niente temporanei orfani
                Err(e)
            }
        }
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Self::from_ron_str(&content)
    }

    /// Serializza lo stato in stringa RON (stesso formato di `save`). Usato per
    /// lo snapshot "base" del rilevamento modifiche esterne.
    pub fn to_ron_string(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    /// Costruisce un `App` da una stringa RON, ricalcolando i campi derivati
    /// (range settimane, `enable` dai `closed`).
    pub fn from_ron_str(s: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut app: App = ron::from_str(s)?;
        app.recompute_week_range();
        // `enable` è un filtro transitorio non persistito: ricalcolalo da `closed`.
        app.projects.reset_enable_from_closed();
        Ok(app)
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

    /// Recomputes the grid week range from project start weeks (or falls back to
    /// the existing app.start_week for backward compatibility with old save files).
    pub fn recompute_week_range(&mut self) {
        let min_start = self
            .projects
            .min_start_week()
            .map(|w| w.0 as i32)
            .or(Some(self.start_week.0 as i32));
        let (n_week, start_week, end_week) = get_default_weeks(min_start);
        self.n_week = WeekId(n_week as usize);
        self.start_week = WeekId(start_week as usize);
        self.end_week = WeekId(end_week as usize);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_save_round_trip_and_no_orphan_tmp() {
        let mut path = std::env::temp_dir();
        path.push(format!("pjm_atomic_{}.ron", std::process::id()));
        let path_str = path.to_string_lossy().to_string();

        let app = App::new();
        app.save(&path_str);

        // Il file finale esiste, è completo e ricaricabile identico.
        let reloaded = App::load(&path_str).expect("il file salvato deve essere leggibile");
        assert_eq!(app.to_ron_string(), reloaded.to_ron_string());

        // Nessun file temporaneo orfano accanto al file finale.
        let tmp = std::env::temp_dir().join(format!(
            ".pjm_atomic_{}.ron.{}.tmp",
            std::process::id(),
            std::process::id()
        ));
        assert!(!tmp.exists(), "il temporaneo deve essere stato rinominato");

        let _ = fs::remove_file(&path_str);
    }
}
