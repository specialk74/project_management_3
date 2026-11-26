//! Domanda per dev e per mese (MIGLIORAMENTI #60, fase 1): quanto lavoro c'è
//! per ogni dev (ruolo), mese per mese, diviso in **assegnato** (ore già in
//! griglia) e **da assegnare** (il residuo del dev, pianificato − ore in
//! griglia, spalmato in parti uguali sulle settimane che restano al
//! progetto). Dati della finestra Vista ▸ Domanda per dev…; la capacità per
//! dev arriverà con la fase 2.

use std::collections::BTreeSet;

use chrono::Datelike;

use crate::app::App;
use crate::categories::CategoryId;
use crate::date_utils::dates::days_to_local;
use crate::dev_utils::dev::DevId;
use crate::priorities::PriorityId;
use crate::project_utils::project::{ProjectId, ProjectStatus};
use crate::single_dev_utils::single_dev::{WEEK_STEP, WeekId};

/// Una colonna della tabella.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemandCol {
    /// Un mese (le settimane contano nel mese del loro lunedì).
    Month { year: i32, month: u32 },
    /// Residuo dei progetti **senza data di fine**: non si sa quando cade.
    NoEnd,
    /// Residuo dei progetti con la fine **prima della settimana corrente**.
    Late,
}

/// La parte di un progetto in una cella.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjDemand {
    pub proj: ProjectId,
    pub assigned: usize,
    pub unassigned: f64,
}

/// Una cella: ore assegnate e da assegnare, con i progetti che le compongono.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemandCell {
    pub assigned: usize,
    pub unassigned: f64,
    pub projects: Vec<ProjDemand>,
}

impl DemandCell {
    pub fn total(&self) -> f64 {
        self.assigned as f64 + self.unassigned
    }

    pub fn is_empty(&self) -> bool {
        self.assigned == 0 && self.unassigned <= 0.0
    }

    fn add(&mut self, proj: ProjectId, assigned: usize, unassigned: f64) {
        self.assigned += assigned;
        self.unassigned += unassigned;
        match self.projects.iter_mut().find(|p| p.proj == proj) {
            Some(p) => {
                p.assigned += assigned;
                p.unassigned += unassigned;
            }
            None => self.projects.push(ProjDemand { proj, assigned, unassigned }),
        }
    }

    fn merge(&mut self, other: &DemandCell) {
        for p in &other.projects {
            self.add(p.proj, p.assigned, p.unassigned);
        }
    }

    /// Progetti dal più pesante al più leggero (per il tooltip).
    pub fn projects_by_weight(&self) -> Vec<&ProjDemand> {
        let mut v: Vec<&ProjDemand> = self.projects.iter().collect();
        v.sort_by(|a, b| (b.assigned as f64 + b.unassigned).total_cmp(&(a.assigned as f64 + a.unassigned)));
        v
    }
}

/// Una riga: un dev, una cella per colonna.
#[derive(Clone, Debug, PartialEq)]
pub struct DemandRow {
    pub dev: DevId,
    pub cells: Vec<DemandCell>,
}

impl DemandRow {
    /// Somma di tutte le colonne.
    pub fn sum(&self) -> DemandCell {
        let mut c = DemandCell::default();
        for x in &self.cells {
            c.merge(x);
        }
        c
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemandData {
    pub cols: Vec<DemandCol>,
    /// Solo i dev con domanda, nell'ordine dei dev.
    pub rows: Vec<DemandRow>,
    /// Somma dei dev, colonna per colonna.
    pub total: Vec<DemandCell>,
}

/// Mese (anno, mese) del giorno `days`.
pub fn month_of_day(days: i32) -> (i32, u32) {
    let d = days_to_local(days);
    (d.year(), d.month())
}

/// I mesi da quello di `from` a quello di `to` (inclusi).
pub fn months(from: i32, to: i32) -> Vec<(i32, u32)> {
    let (mut y, mut m) = month_of_day(from);
    let last = month_of_day(to);
    let mut out = Vec::new();
    while (y, m) <= last {
        out.push((y, m));
        (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    }
    out
}

/// Settimane in cui va spalmato il residuo di un dev: da `now` (il lunedì
/// della settimana corrente), o dall'inizio del progetto se è dopo, alla fine
/// del progetto (inclusa). `None` senza data di fine; vuoto se la fine è già
/// passata.
pub fn spread_weeks(start: Option<WeekId>, end: Option<WeekId>, now: WeekId) -> Option<Vec<WeekId>> {
    let end = end?;
    let first = start.map_or(now, |s| s.max(now));
    Some((first.0..=end.0).step_by(WEEK_STEP).map(WeekId).collect())
}

/// Costruisce la tabella per il periodo `[from, to]` (giorni; una settimana
/// conta se il suo lunedì ci cade dentro). `now` = lunedì della settimana
/// corrente. Filtri come Matrice e Torte (vuoto = tutti): categorie, stati,
/// priorità e dev (solo le righe di quei dev). Le colonne «Senza fine» e «In ritardo» ci sono solo se servono
/// e non dipendono dal periodo.
#[allow(clippy::too_many_arguments)]
pub fn build(
    app: &App,
    from: i32,
    to: i32,
    now: WeekId,
    cats: &BTreeSet<Option<CategoryId>>,
    statuses: &BTreeSet<ProjectStatus>,
    pris: &BTreeSet<Option<PriorityId>>,
    dev_filter: &BTreeSet<DevId>,
) -> DemandData {
    let months = months(from, to);
    let mut cols: Vec<DemandCol> = months.iter().map(|&(year, month)| DemandCol::Month { year, month }).collect();
    let (no_end, late) = (cols.len(), cols.len() + 1);
    cols.push(DemandCol::NoEnd);
    cols.push(DemandCol::Late);
    let col_of = |days: i32| -> Option<usize> {
        if days < from || days > to {
            return None;
        }
        let m = month_of_day(days);
        months.iter().position(|x| *x == m)
    };

    let devs: Vec<DevId> = app
        .devs
        .list()
        .into_iter()
        .map(|(d, _)| d)
        .filter(|d| dev_filter.is_empty() || dev_filter.contains(d))
        .collect();
    let mut rows: Vec<DemandRow> =
        devs.iter().map(|&dev| DemandRow { dev, cells: vec![DemandCell::default(); cols.len()] }).collect();
    for (proj, _) in app.projects.list() {
        if !cats.is_empty() && !cats.contains(&app.projects.get_category(proj)) {
            continue;
        }
        if !statuses.is_empty() && !statuses.contains(&app.projects.status(proj)) {
            continue;
        }
        if !pris.is_empty() && !pris.contains(&app.projects.get_priority(proj)) {
            continue;
        }
        let spread = spread_weeks(
            app.projects.get_project_start_week(proj),
            app.projects.get_project_end_week(proj),
            now,
        );
        for dev in app.projects.list_devs(proj) {
            let Some(sd) = app.projects.get_single_dev(proj, dev) else { continue };
            let Some(row) = rows.iter_mut().find(|r| r.dev == dev) else { continue };
            // Assegnate: le ore in griglia, nel mese del lunedì.
            for wk in sd.get_weeks() {
                let h = sd.get_all(wk).map_or(0, |sew| sew.effort_tot().0);
                if h > 0
                    && let Some(i) = col_of(wk.0 as i32)
                {
                    row.cells[i].add(proj, h, 0.0);
                }
            }
            // Da assegnare: il residuo, spalmato sulle settimane che restano.
            let residual = sd.planned_effort().0.saturating_sub(sd.effort_up_to(WeekId(usize::MAX)).0);
            if residual == 0 {
                continue;
            }
            match &spread {
                None => row.cells[no_end].add(proj, 0, residual as f64),
                Some(ws) if ws.is_empty() => row.cells[late].add(proj, 0, residual as f64),
                Some(ws) => {
                    let share = residual as f64 / ws.len() as f64;
                    for w in ws {
                        if let Some(i) = col_of(w.0 as i32) {
                            row.cells[i].add(proj, 0, share);
                        }
                    }
                }
            }
        }
    }
    rows.retain(|r| r.cells.iter().any(|c| !c.is_empty()));
    // Le due colonne speciali solo se qualcuno le usa.
    for i in [late, no_end] {
        if rows.iter().all(|r| r.cells[i].is_empty()) {
            cols.remove(i);
            for r in &mut rows {
                r.cells.remove(i);
            }
        }
    }
    let mut total = vec![DemandCell::default(); cols.len()];
    for r in &rows {
        for (t, c) in total.iter_mut().zip(&r.cells) {
            t.merge(c);
        }
    }
    DemandData { cols, rows, total }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date_utils::dates::{local_to_days, primo_giorno_settimana_corrente};
    use crate::single_effort_utils::single_effort::Effort;

    /// Lunedì della settimana che contiene il giorno.
    fn monday(y: i32, m: u32, d: u32) -> WeekId {
        let date = chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
        WeekId(local_to_days(&primo_giorno_settimana_corrente(&date)) as usize)
    }

    fn day(y: i32, m: u32, d: u32) -> i32 {
        local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap())
    }

    #[test]
    fn months_cover_the_period() {
        assert_eq!(months(day(2026, 11, 23), day(2027, 2, 1)), vec![(2026, 11), (2026, 12), (2027, 1), (2027, 2)]);
        assert_eq!(months(day(2026, 11, 2), day(2026, 11, 30)), vec![(2026, 11)]);
    }

    #[test]
    fn spread_starts_today_or_at_the_project_start() {
        let now = monday(2026, 11, 23);
        let end = Some(WeekId(now.0 + 3 * WEEK_STEP));
        assert_eq!(spread_weeks(None, end, now).unwrap().len(), 4);
        assert_eq!(spread_weeks(Some(WeekId(now.0 - 70)), end, now).unwrap().len(), 4, "inizio passato: da oggi");
        assert_eq!(spread_weeks(Some(WeekId(now.0 + 14)), end, now).unwrap().len(), 2, "inizio futuro");
        assert!(spread_weeks(None, None, now).is_none(), "senza fine");
        assert!(spread_weeks(None, Some(WeekId(now.0 - 7)), now).unwrap().is_empty(), "fine passata");
    }

    /// Assegnate nel mese del lunedì, residuo spalmato in parti uguali sulle
    /// settimane che restano (solo la parte nel periodo), residui senza fine o
    /// in ritardo nelle loro colonne; filtri sui progetti; solo i dev con
    /// domanda; totale per colonna.
    #[test]
    fn demand_splits_assigned_and_unassigned_by_month() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        app.devs.add("Test"); // senza domanda: niente riga
        let now = monday(2026, 11, 23);
        let wk = |i: usize| WeekId(now.0 + i * WEEK_STEP);
        // A: fine fra 3 settimane (23/11 → 14/12), Frontend pianificato 100h,
        // 20h già in griglia (8 a novembre, 12 a dicembre).
        let a = app.projects.add("A", Some("AAA"), None);
        app.projects.set_project_end_week(a, Some(wk(3)));
        app.projects.add_dev(a, fe);
        app.projects.add_dev_effort(a, fe, Effort(100));
        app.projects.add_effort(a, fe, wk(0), alice, Effort(8));
        app.projects.add_effort(a, fe, wk(2), alice, Effort(12));
        // B: senza fine, Backend 30h da assegnare.
        let b = app.projects.add("B", Some("BBB"), None);
        app.projects.add_dev(b, be);
        app.projects.add_dev_effort(b, be, Effort(30));
        // C: finito la settimana scorsa, Backend 10h mancanti.
        let c = app.projects.add("C", Some("CCC"), None);
        app.projects.set_project_end_week(c, Some(WeekId(now.0 - WEEK_STEP)));
        app.projects.add_dev(c, be);
        app.projects.add_dev_effort(c, be, Effort(10));
        // D: chiuso, filtrato via.
        let d = app.projects.add("D", Some("DDD"), None);
        app.projects.add_dev(d, fe);
        app.projects.add_effort(d, fe, wk(0), alice, Effort(40));
        app.projects.set_closed(d, true);

        let open: BTreeSet<ProjectStatus> = [ProjectStatus::Active, ProjectStatus::OnHold].into_iter().collect();
        let none = BTreeSet::new();
        let data = build(&app, now.0 as i32, day(2027, 1, 31), now, &none, &open, &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(
            data.cols,
            vec![
                DemandCol::Month { year: 2026, month: 11 },
                DemandCol::Month { year: 2026, month: 12 },
                DemandCol::Month { year: 2027, month: 1 },
                DemandCol::NoEnd,
                DemandCol::Late,
            ]
        );
        assert_eq!(data.rows.iter().map(|r| r.dev).collect::<Vec<_>>(), vec![fe, be], "Test senza domanda");
        let fe_row = &data.rows[0];
        // 80h da assegnare su 4 settimane = 20h l'una: due lunedì a novembre
        // (23/11, 30/11), due a dicembre (7/12, 14/12).
        assert_eq!((fe_row.cells[0].assigned, fe_row.cells[0].unassigned), (8, 40.0));
        assert_eq!((fe_row.cells[1].assigned, fe_row.cells[1].unassigned), (12, 40.0));
        assert!(fe_row.cells[2].is_empty());
        assert_eq!(fe_row.sum().total(), 100.0, "assegnate + da assegnare = pianificato");
        let be_row = &data.rows[1];
        assert_eq!((be_row.cells[3].unassigned, be_row.cells[4].unassigned), (30.0, 10.0));
        assert_eq!(be_row.cells[3].projects, vec![ProjDemand { proj: b, assigned: 0, unassigned: 30.0 }]);
        assert_eq!(data.total[1].total(), 52.0);
        let heavy = data.total[0].projects_by_weight();
        assert_eq!(heavy[0].proj, a);
        // Senza filtro di stato entra anche D (chiuso): 40h assegnate a novembre.
        let all = build(&app, now.0 as i32, day(2027, 1, 31), now, &none, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(all.rows[0].cells[0].assigned, 48);
        // Periodo di un mese: dicembre fuori, le colonne speciali restano.
        let nov = build(&app, now.0 as i32, day(2026, 11, 30), now, &none, &open, &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(nov.cols.len(), 3);
        assert_eq!(nov.rows[0].sum().total(), 48.0);
        // Filtro dev: solo il Backend (le colonne speciali restano, il totale segue).
        let only_be: BTreeSet<DevId> = [be].into_iter().collect();
        let be_only = build(&app, now.0 as i32, day(2027, 1, 31), now, &none, &open, &BTreeSet::new(), &only_be);
        assert_eq!(be_only.rows.iter().map(|r| r.dev).collect::<Vec<_>>(), vec![be]);
        assert_eq!(be_only.total.iter().map(|c| c.total()).sum::<f64>(), 40.0);
    }
}
