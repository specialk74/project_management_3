//! Merge a 3 vie dello stato, usato quando il file .ron condiviso (es. su
//! OneDrive) viene modificato da un collega mentre l'utente ha modifiche locali
//! non ancora salvate.
//!
//! Granularità **progetto → dev**: cambiamenti su progetti (o dev) diversi si
//! fondono automaticamente; c'è conflitto solo se lo *stesso* dev — oppure i
//! *dati generali* dello stesso progetto — è stato modificato da entrambi.
//!
//! Le collezioni globali (worker, dev, milestone, categorie, holidays) sono
//! gestite in blocco: se le ho modificate io tengo le mie, altrimenti prendo
//! quelle del collega.
//!
//! Sui punti in conflitto **vince la versione locale** (l'utente poi decide via
//! dialog se ricaricare scartando le proprie modifiche).

use std::collections::{BTreeSet, HashMap};

use crate::app::App;
use crate::dev_utils::dev::DevId;
use crate::project_utils::project::{Project, ProjectId};
use crate::project_utils::projects::Projects;
use crate::single_dev_utils::single_dev::SingleDev;

/// Esito del merge: stato risultante + elenco (testuale) dei conflitti.
pub struct MergeOutcome {
    pub app: App,
    /// Vuoto = fuso senza conflitti.
    pub conflicts: Vec<String>,
}

/// Etichetta leggibile di un progetto (tripletta, altrimenti prima riga info).
fn project_label(p: &Project) -> String {
    match p.get_tripletta() {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => p.get_info().lines().next().unwrap_or("").to_string(),
    }
}

fn opt_content_eq(a: Option<&Project>, b: Option<&Project>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.content_eq(y),
        _ => false,
    }
}

/// Fonde due versioni dello stesso progetto (entrambe modificate rispetto alla
/// base) a livello di intestazione e di singolo dev.
fn merge_one_project(
    base: &Project,
    mine: &Project,
    theirs: &Project,
    dev_name: &dyn Fn(DevId) -> String,
    conflicts: &mut Vec<String>,
) -> Project {
    // Intestazione (dati generali): se cambiata da entrambi in modo diverso è un
    // conflitto e si tiene la mia; altrimenti si prende quella che è cambiata.
    let h_mine = !mine.header_eq(base);
    let h_theirs = !theirs.header_eq(base);
    let header_src = if h_mine && h_theirs && !mine.header_eq(theirs) {
        conflicts.push(format!(
            "Progetto «{}» — dati generali",
            project_label(mine)
        ));
        mine
    } else if h_theirs {
        theirs
    } else {
        mine
    };

    // Dev: 3 vie per ciascun DevId.
    let mut merged: HashMap<DevId, SingleDev> = HashMap::new();
    let mut dev_ids: BTreeSet<DevId> = BTreeSet::new();
    dev_ids.extend(base.devs().keys().copied());
    dev_ids.extend(mine.devs().keys().copied());
    dev_ids.extend(theirs.devs().keys().copied());
    for did in dev_ids {
        let db = base.devs().get(&did);
        let dm = mine.devs().get(&did);
        let dt = theirs.devs().get(&did);
        let mine_changed = dm != db;
        let theirs_changed = dt != db;
        let chosen = if !theirs_changed {
            dm // il collega non ha toccato questo dev → tengo il mio
        } else if !mine_changed {
            dt // solo il collega → prendo il suo
        } else {
            // Entrambi hanno cambiato lo stesso dev: conflitto, tengo il mio.
            if dm != dt {
                conflicts.push(format!(
                    "Progetto «{}» — dev {}",
                    project_label(mine),
                    dev_name(did)
                ));
            }
            dm
        };
        if let Some(sd) = chosen {
            merged.insert(did, sd.clone());
        }
    }

    let mut result = header_src.clone();
    result.set_devs(merged);
    result
}

/// Decide il progetto risultante per un dato id. `None` = da rimuovere.
fn decide_project(
    base: Option<&Project>,
    mine: Option<&Project>,
    theirs: Option<&Project>,
    dev_name: &dyn Fn(DevId) -> String,
    conflicts: &mut Vec<String>,
) -> Option<Project> {
    let mine_changed = !opt_content_eq(mine, base);
    let theirs_changed = !opt_content_eq(theirs, base);
    if !theirs_changed {
        return mine.cloned(); // il collega non ha toccato il progetto → tengo il mio
    }
    if !mine_changed {
        return theirs.cloned(); // solo il collega → prendo il suo
    }
    match (base, mine, theirs) {
        (Some(b), Some(m), Some(t)) => Some(merge_one_project(b, m, t, dev_name, conflicts)),
        _ => {
            // Aggiunta/rimozione dello stesso progetto da entrambi: conflitto,
            // tengo la mia versione (che può essere una rimozione).
            let label = mine
                .or(theirs)
                .or(base)
                .map(project_label)
                .unwrap_or_default();
            conflicts.push(format!("Progetto «{label}» (aggiunto/rimosso da entrambi)"));
            mine.cloned()
        }
    }
}

fn merge_projects(
    base: &Projects,
    mine: &Projects,
    theirs: &Projects,
    dev_name: &dyn Fn(DevId) -> String,
    conflicts: &mut Vec<String>,
) -> Projects {
    // Partiamo dalla copia del collega (conserva last_id e i suoi progetti);
    // sovrascriviamo/rimuoviamo dove il merge lo richiede.
    let mut result = theirs.clone();
    let mut ids: BTreeSet<ProjectId> = BTreeSet::new();
    ids.extend(base.ids());
    ids.extend(mine.ids());
    ids.extend(theirs.ids());
    for id in ids {
        match decide_project(
            base.get(id),
            mine.get(id),
            theirs.get(id),
            dev_name,
            conflicts,
        ) {
            Some(p) => result.set_project(id, p),
            None => result.del(id),
        }
    }
    result
}

fn globals_changed(mine: &App, base: &App) -> bool {
    mine.workers != base.workers
        || mine.devs != base.devs
        || mine.milestones != base.milestones
        || mine.categories != base.categories
        || mine.holidays != base.holidays
}

/// Fonde `mine` (stato locale) e `theirs` (file su disco appena caricato) usando
/// `base` come antenato comune. `dev_name` risolve il nome di un dev per i
/// messaggi di conflitto.
pub fn merge(
    base: &App,
    mine: &App,
    mut theirs: App,
    dev_name: &dyn Fn(DevId) -> String,
) -> MergeOutcome {
    let mut conflicts = Vec::new();

    theirs.projects = merge_projects(
        &base.projects,
        &mine.projects,
        &theirs.projects,
        dev_name,
        &mut conflicts,
    );

    // Globali: se li ho modificati io li tengo, altrimenti resto su quelli del
    // collega (già presenti in `theirs`).
    if globals_changed(mine, base) {
        theirs.workers = mine.workers.clone();
        theirs.devs = mine.devs.clone();
        theirs.milestones = mine.milestones.clone();
        theirs.categories = mine.categories.clone();
        theirs.holidays = mine.holidays.clone();
    }

    theirs.recompute_week_range();
    theirs.projects.reset_enable_from_closed();
    theirs.compute_sovra();

    MergeOutcome {
        app: theirs,
        conflicts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::sinlge_effort::Effort;
    use crate::workers_utils::worker::WorkerId;

    fn no_names(_: DevId) -> String {
        "?".to_string()
    }

    /// App base con un progetto e due dev con effort.
    fn base_app() -> (App, ProjectId, DevId, DevId) {
        let mut app = App::new();
        let pid = app
            .projects
            .add("Progetto", Some("ABC"), Some(WeekId(20000)));
        let d1 = app.devs.add("Frontend");
        let d2 = app.devs.add("Backend");
        app.projects.add_dev(pid, d1);
        app.projects.add_dev(pid, d2);
        app.projects
            .add_effort(pid, d1, WeekId(20000), WorkerId(0), Effort(8));
        app.projects
            .add_effort(pid, d2, WeekId(20000), WorkerId(0), Effort(8));
        (app, pid, d1, d2)
    }

    #[test]
    fn no_changes_no_conflicts() {
        let (base, ..) = base_app();
        let mine = base.clone();
        let theirs = base.clone();
        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.conflicts.is_empty());
    }

    #[test]
    fn different_devs_merge_without_conflict() {
        let (base, pid, d1, d2) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        // io cambio d1, il collega cambia d2
        mine.projects
            .add_effort(pid, d1, WeekId(20001), WorkerId(0), Effort(5));
        theirs
            .projects
            .add_effort(pid, d2, WeekId(20001), WorkerId(0), Effort(7));

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(
            out.conflicts.is_empty(),
            "conflitti inattesi: {:?}",
            out.conflicts
        );
        // il merge contiene entrambe le modifiche
        let sd1 = out.app.projects.get_single_dev(pid, d1).unwrap();
        let sd2 = out.app.projects.get_single_dev(pid, d2).unwrap();
        assert_eq!(sd1.get_effort_by_week(WeekId(20001)), Effort(5));
        assert_eq!(sd2.get_effort_by_week(WeekId(20001)), Effort(7));
    }

    #[test]
    fn same_dev_changed_by_both_is_conflict_and_keeps_mine() {
        let (base, pid, d1, _) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects
            .add_effort(pid, d1, WeekId(20002), WorkerId(0), Effort(3));
        theirs
            .projects
            .add_effort(pid, d1, WeekId(20002), WorkerId(0), Effort(9));

        let out = merge(&base, &mine, theirs, &no_names);
        assert_eq!(out.conflicts.len(), 1, "atteso un conflitto sul dev");
        // vince la mia versione
        let sd1 = out.app.projects.get_single_dev(pid, d1).unwrap();
        assert_eq!(sd1.get_effort_by_week(WeekId(20002)), Effort(3));
    }

    #[test]
    fn different_projects_merge_without_conflict() {
        let (base, pid, d1, _) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        // io modifico il progetto esistente
        mine.projects
            .add_effort(pid, d1, WeekId(20003), WorkerId(0), Effort(4));
        // il collega aggiunge un progetto nuovo (id diverso)
        let pid2 = theirs
            .projects
            .add("Nuovo", Some("XYZ"), Some(WeekId(20010)));

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(
            out.conflicts.is_empty(),
            "conflitti inattesi: {:?}",
            out.conflicts
        );
        // sono presenti sia la mia modifica sia il progetto del collega
        assert_eq!(
            out.app
                .projects
                .get_single_dev(pid, d1)
                .unwrap()
                .get_effort_by_week(WeekId(20003)),
            Effort(4)
        );
        assert!(out.app.projects.get(pid2).is_some());
    }

    #[test]
    fn colleague_only_change_is_taken() {
        let (base, pid, d1, _) = base_app();
        let mine = base.clone();
        let mut theirs = base.clone();
        theirs
            .projects
            .add_effort(pid, d1, WeekId(20004), WorkerId(0), Effort(6));

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.conflicts.is_empty());
        assert_eq!(
            out.app
                .projects
                .get_single_dev(pid, d1)
                .unwrap()
                .get_effort_by_week(WeekId(20004)),
            Effort(6)
        );
    }

    // ── Intestazione del progetto (dati generali) ───────────────────────────

    /// Se i "dati generali" cambiano da entrambe le parti in modo diverso è un
    /// conflitto e vince la mia versione: il collega non deve poter sovrascrivere
    /// in silenzio la tripletta o le date che sto modificando.
    #[test]
    fn header_changed_by_both_is_conflict_and_keeps_mine() {
        let (base, pid, ..) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects.set_tripletta(pid, "MIA");
        theirs.projects.set_tripletta(pid, "SUA");

        let out = merge(&base, &mine, theirs, &no_names);
        assert_eq!(out.conflicts.len(), 1);
        assert!(
            out.conflicts[0].contains("dati generali"),
            "il messaggio deve dire che il conflitto è sull'intestazione: {:?}",
            out.conflicts[0]
        );
        assert_eq!(out.app.projects.get_tripletta(pid), "MIA");
    }

    /// Stessa modifica identica su entrambi i lati: nessun conflitto (non c'è
    /// nulla da scegliere).
    #[test]
    fn identical_header_change_on_both_sides_is_not_a_conflict() {
        let (base, pid, ..) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects.set_tripletta(pid, "UGUALE");
        theirs.projects.set_tripletta(pid, "UGUALE");

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.conflicts.is_empty(), "{:?}", out.conflicts);
        assert_eq!(out.app.projects.get_tripletta(pid), "UGUALE");
    }

    /// Il collega cambia l'intestazione, io un dev dello stesso progetto:
    /// si tengono **entrambe** le modifiche, senza conflitto.
    #[test]
    fn colleague_header_and_my_dev_both_survive() {
        let (base, pid, d1, _) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects
            .add_effort(pid, d1, WeekId(20005), WorkerId(0), Effort(4));
        theirs
            .projects
            .set_project_end_week(pid, Some(WeekId(30000)));

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.conflicts.is_empty(), "{:?}", out.conflicts);
        assert_eq!(
            out.app.projects.get_project_end_week(pid),
            Some(WeekId(30000)),
            "l'intestazione del collega deve essere applicata"
        );
        assert_eq!(
            out.app
                .projects
                .get_single_dev(pid, d1)
                .unwrap()
                .get_effort_by_week(WeekId(20005)),
            Effort(4),
            "la mia modifica al dev non deve andare persa"
        );
    }

    // ── Aggiunta / rimozione di progetti ────────────────────────────────────

    /// Progetto cancellato solo dal collega: la cancellazione si propaga.
    #[test]
    fn project_deleted_only_by_colleague_is_removed() {
        let (base, pid, ..) = base_app();
        let mine = base.clone();
        let mut theirs = base.clone();
        theirs.projects.del(pid);

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.conflicts.is_empty(), "{:?}", out.conflicts);
        assert!(out.app.projects.get(pid).is_none());
    }

    /// Io cancello il progetto, il collega lo modifica: conflitto, e vince la mia
    /// versione — cioè la cancellazione.
    #[test]
    fn delete_vs_change_is_conflict_and_my_delete_wins() {
        let (base, pid, d1, _) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects.del(pid);
        theirs
            .projects
            .add_effort(pid, d1, WeekId(20006), WorkerId(0), Effort(9));

        let out = merge(&base, &mine, theirs, &no_names);
        assert_eq!(out.conflicts.len(), 1);
        assert!(
            out.conflicts[0].contains("aggiunto/rimosso"),
            "messaggio inatteso: {:?}",
            out.conflicts[0]
        );
        assert!(
            out.app.projects.get(pid).is_none(),
            "vince la mia versione, che è la rimozione"
        );
    }

    // ── Dev aggiunti / rimossi dentro lo stesso progetto ────────────────────

    /// Il collega aggiunge un dev nuovo a un progetto che sto modificando io:
    /// il dev nuovo arriva, il mio lavoro resta.
    #[test]
    fn dev_added_by_colleague_survives_my_change() {
        let (base, pid, d1, _) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects
            .add_effort(pid, d1, WeekId(20007), WorkerId(0), Effort(2));
        let d3 = theirs.devs.add("QA");
        theirs.projects.add_dev(pid, d3);
        theirs
            .projects
            .add_effort(pid, d3, WeekId(20007), WorkerId(0), Effort(5));

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.conflicts.is_empty(), "{:?}", out.conflicts);
        assert_eq!(
            out.app
                .projects
                .get_single_dev(pid, d1)
                .unwrap()
                .get_effort_by_week(WeekId(20007)),
            Effort(2)
        );
        assert!(
            out.app.projects.get_single_dev(pid, d3).is_some(),
            "il dev aggiunto dal collega deve esserci"
        );
    }

    /// Il nome del dev in conflitto arriva dal risolutore passato dalla UI, e
    /// l'etichetta del progetto è la tripletta.
    #[test]
    fn conflict_message_names_project_and_dev() {
        let (base, pid, d1, _) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.projects
            .add_effort(pid, d1, WeekId(20008), WorkerId(0), Effort(1));
        theirs
            .projects
            .add_effort(pid, d1, WeekId(20008), WorkerId(0), Effort(2));

        let names = |d: DevId| {
            if d == d1 {
                "Frontend".to_string()
            } else {
                "?".to_string()
            }
        };
        let out = merge(&base, &mine, theirs, &names);
        assert_eq!(out.conflicts.len(), 1);
        assert!(
            out.conflicts[0].contains("ABC") && out.conflicts[0].contains("Frontend"),
            "il messaggio deve identificare progetto e dev: {:?}",
            out.conflicts[0]
        );
    }

    // ── Collezioni globali (worker, dev, milestone, categorie, holidays) ────

    /// Le globali sono gestite in blocco: se le ho toccate io vincono le mie
    /// **per intero**, quindi un worker aggiunto dal collega si perde. È una
    /// semplificazione voluta — questo test la fissa perché non cambi per sbaglio.
    #[test]
    fn my_globals_win_wholesale_when_i_changed_them() {
        let (base, ..) = base_app();
        let mut mine = base.clone();
        let mut theirs = base.clone();
        mine.workers.add("Alice");
        theirs.workers.add("Bob");

        let out = merge(&base, &mine, theirs, &no_names);
        let names: Vec<String> = out.app.workers.list().into_iter().map(|(_, n)| n).collect();
        assert!(names.iter().any(|n| n == "Alice"));
        assert!(
            !names.iter().any(|n| n == "Bob"),
            "comportamento noto: le globali si prendono in blocco, non si fondono"
        );
    }

    /// Se io non ho toccato le globali, arrivano quelle del collega.
    #[test]
    fn colleague_globals_are_taken_when_i_changed_none() {
        let (base, ..) = base_app();
        let mine = base.clone();
        let mut theirs = base.clone();
        theirs.workers.add("Bob");
        let cat = theirs.categories.add("Manutenzione");

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.app.workers.list().into_iter().any(|(_, n)| n == "Bob"));
        assert_eq!(out.app.categories.get_name(cat), Some("Manutenzione"));
    }

    /// Dopo il merge lo stato è ricalcolato: i progetti chiusi restano non
    /// visibili (`enable = false`) come dopo un caricamento normale.
    #[test]
    fn merged_state_is_recomputed_closed_projects_stay_disabled() {
        let (base, pid, ..) = base_app();
        let mine = base.clone();
        let mut theirs = base.clone();
        theirs.projects.set_closed(pid, true);

        let out = merge(&base, &mine, theirs, &no_names);
        assert!(out.app.projects.is_closed(pid));
        assert!(!out.app.projects.get_enable(&pid).0);
    }
}
