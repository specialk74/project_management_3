//! Dove sono le differenze fra due stati dell'`App` (Annulla/Ripeti): la UI
//! ci porta la griglia e le fa brillare. Si confronta per progetto, dev,
//! settimana e worker; il resto (worker, dev, categorie, milestone,
//! impostazioni) non ha un posto nella griglia e conta solo come «altro».
//!
//! Il confronto è strutturale (`PartialEq`), **non** sul testo RON: i dati
//! contengono `HashMap`, che si serializzano in un ordine ogni volta diverso,
//! e due stati uguali risulterebbero diversi.

use std::collections::BTreeSet;

use crate::app::App;
use crate::dev_utils::dev::DevId;
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::WeekId;
use crate::workers_utils::worker::WorkerId;

/// Punti cambiati fra due stati.
#[derive(Default, Debug, PartialEq)]
pub struct Changes {
    /// Celle (progetto, dev, settimana, worker) diverse: assegnazione, ore o
    /// nota del worker in quella settimana.
    pub cells: BTreeSet<(ProjectId, DevId, WeekId, WorkerId)>,
    /// Dev diversi senza celle diverse: effort pianificato, nota, % dichiarata,
    /// dev aggiunto al progetto…
    pub devs: BTreeSet<(ProjectId, DevId)>,
    /// Progetti diversi senza dev diversi: nome, date, categoria, stato,
    /// milestone, note, dev tolto, progetto ricomparso…
    pub projects: BTreeSet<ProjectId>,
    /// Qualcosa fuori dalla griglia (anagrafiche, milestone, impostazioni) o
    /// un progetto sparito.
    pub other: bool,
}

impl Changes {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.devs.is_empty() && self.projects.is_empty() && !self.other
    }
}

/// Le differenze fra `before` e `after` (lo stato che si vedrà).
pub fn changes(before: &App, after: &App) -> Changes {
    let mut out = Changes::default();
    let mut ids: BTreeSet<ProjectId> = before.projects.ids().into_iter().collect();
    ids.extend(after.projects.ids());
    for pid in ids {
        let (Some(pb), Some(pa)) = (before.projects.get(pid), after.projects.get(pid)) else {
            if after.projects.get(pid).is_some() {
                out.projects.insert(pid); // ricomparso
            } else {
                out.other = true; // sparito: non c'è dove andare
            }
            continue;
        };
        if pb == pa {
            continue;
        }
        let mut devs: BTreeSet<DevId> = before.projects.list_devs(pid).into_iter().collect();
        devs.extend(after.projects.list_devs(pid));
        let mut dev_changed = false;
        for dev in devs {
            let (sb, sa) = (
                before.projects.get_single_dev(pid, dev),
                after.projects.get_single_dev(pid, dev),
            );
            if sb == sa {
                continue;
            }
            dev_changed = true;
            let Some(sa) = sa else {
                out.projects.insert(pid); // dev tolto: si vede sul progetto
                continue;
            };
            let mut weeks: BTreeSet<WeekId> = sa.get_weeks().into_iter().collect();
            if let Some(sb) = sb {
                weeks.extend(sb.get_weeks());
            }
            let mut cell = false;
            for wk in weeks {
                let (wb, wa) = (sb.and_then(|s| s.get_all(wk)), sa.get_all(wk));
                if wb == wa {
                    continue;
                }
                let mut workers: BTreeSet<WorkerId> = BTreeSet::new();
                workers.extend(wb.into_iter().flat_map(|w| w.worker_id.keys().copied()));
                workers.extend(wa.into_iter().flat_map(|w| w.worker_id.keys().copied()));
                for w in workers {
                    let before = wb.and_then(|e| e.worker_id.get(&w));
                    let after = wa.and_then(|e| e.worker_id.get(&w));
                    if before != after {
                        out.cells.insert((pid, dev, wk, w));
                        cell = true;
                    }
                }
            }
            if !cell {
                out.devs.insert((pid, dev));
            }
        }
        if !dev_changed {
            out.projects.insert(pid);
        }
    }
    let same = before.workers == after.workers
        && before.devs == after.devs
        && before.categories == after.categories
        && before.priorities == after.priorities
        && before.milestones == after.milestones
        && before.holidays == after.holidays
        && before.rules == after.rules
        && before.month_colors == after.month_colors
        && before.month_tint_pct == after.month_tint_pct
        && before.corner_pct == after.corner_pct;
    out.other = out.other || !same;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_effort_utils::single_effort::Effort;

    #[test]
    fn finds_cells_devs_projects_and_other_changes() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("x", Some("P1"), None);
        let q = app.projects.add("y", Some("P2"), None);
        app.projects.add_dev(p, fe);
        app.projects.add_dev(q, be);
        app.projects.add_effort(p, fe, WeekId(700), mario, Effort(8));
        let before = app.clone();
        assert!(changes(&before, &app).is_empty());
        // Stesso stato riletto dal file (le HashMap cambiano ordine): uguale.
        let reread = App::from_ron_str(&app.to_ron_string()).unwrap();
        assert!(changes(&before, &reread).is_empty(), "{:?}", changes(&before, &reread));

        // Una cella cambiata e una nuova.
        app.projects.add_effort(p, fe, WeekId(700), mario, Effort(4));
        app.projects.add_effort(p, fe, WeekId(707), mario, Effort(2));
        let c = changes(&before, &app);
        assert_eq!(c.cells, [(p, fe, WeekId(700), mario), (p, fe, WeekId(707), mario)].into());
        assert!(c.devs.is_empty() && c.projects.is_empty() && !c.other);

        // Effort pianificato: il dev, non una cella.
        let mut app = before.clone();
        app.projects.add_dev_effort(q, be, Effort(100));
        let c = changes(&before, &app);
        assert_eq!(c.devs, [(q, be)].into());
        assert!(c.cells.is_empty());

        // Stato del progetto: il progetto.
        let mut app = before.clone();
        app.projects.set_status(q, crate::project_utils::project::ProjectStatus::OnHold);
        assert_eq!(changes(&before, &app).projects, [q].into());

        // Nome di un worker: fuori dalla griglia.
        let mut app = before.clone();
        app.workers.rename(mario, "Mario Rossi").unwrap();
        let c = changes(&before, &app);
        assert!(c.other && c.cells.is_empty() && c.projects.is_empty());
    }
}
