//! Domanda e capacità per dev e per mese (MIGLIORAMENTI #60, fasi 1-2): quanto lavoro c'è
//! per ogni dev (ruolo), mese per mese, diviso in **assegnato** (ore già in
//! griglia) e **da assegnare** (il residuo del dev, pianificato − ore in
//! griglia, spalmato in parti uguali sulle settimane che restano al
//! progetto). Fase 2: la **capacità** per dev = ore massime effettive dei
//! worker che hanno dichiarato una quota su quel dev (`Worker.dev_shares`) ×
//! la quota, e lo scarto capacità − domanda. Dati della finestra Vista ▸
//! Domanda e capacità…

use std::collections::BTreeSet;

use chrono::Datelike;

use crate::app::App;
use crate::categories::CategoryId;
use crate::date_utils::dates::days_to_local;
use crate::dev_utils::dev::DevId;
use crate::priorities::PriorityId;
use crate::project_utils::project::{ProjectId, ProjectStatus};
use crate::single_dev_utils::single_dev::{WEEK_STEP, WeekId};
use crate::workers_utils::worker::{WORKER_ID_ZERO, WorkerId};

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

/// Una cella: ore assegnate e da assegnare, con i progetti che le compongono,
/// e la capacità (0 nelle colonne «Senza fine» / «In ritardo») con i worker
/// che la danno.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemandCell {
    pub assigned: usize,
    pub unassigned: f64,
    pub projects: Vec<ProjDemand>,
    pub capacity: f64,
    pub cap_workers: Vec<(WorkerId, f64)>,
}

impl DemandCell {
    /// Domanda: assegnate + da assegnare.
    pub fn total(&self) -> f64 {
        self.assigned as f64 + self.unassigned
    }

    /// Niente domanda.
    pub fn no_demand(&self) -> bool {
        self.assigned == 0 && self.unassigned <= 0.0
    }

    /// Niente domanda né capacità.
    pub fn is_empty(&self) -> bool {
        self.no_demand() && self.capacity <= 0.0
    }

    /// Capacità − domanda: positivo = avanza, negativo = manca.
    pub fn gap(&self) -> f64 {
        self.capacity - self.total()
    }

    fn add_capacity(&mut self, worker: WorkerId, hours: f64) {
        self.capacity += hours;
        match self.cap_workers.iter_mut().find(|(w, _)| *w == worker) {
            Some((_, h)) => *h += hours,
            None => self.cap_workers.push((worker, hours)),
        }
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
        for &(w, h) in &other.cap_workers {
            self.add_capacity(w, h);
        }
    }

    /// Worker dal contributo più grande al più piccolo (per il tooltip).
    pub fn cap_workers_by_weight(&self) -> Vec<(WorkerId, f64)> {
        let mut v = self.cap_workers.clone();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    }

    /// Progetti dal più pesante al più leggero (per il tooltip).
    pub fn projects_by_weight(&self) -> Vec<&ProjDemand> {
        let mut v: Vec<&ProjDemand> = self.projects.iter().collect();
        v.sort_by(|a, b| (b.assigned as f64 + b.unassigned).total_cmp(&(a.assigned as f64 + a.unassigned)));
        v
    }
}

/// Una riga: un dev, una cella per colonna. `declared` = almeno un worker
/// (che conta per la capacità) ha una quota su questo dev: senza, la
/// capacità non è nota e lo scarto non ha senso.
#[derive(Clone, Debug, PartialEq)]
pub struct DemandRow {
    pub dev: DevId,
    pub cells: Vec<DemandCell>,
    pub declared: bool,
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

impl DemandData {
    /// Somma di tutta la tabella (domanda di tutte le colonne, capacità dei mesi).
    pub fn grand(&self) -> DemandCell {
        let mut c = DemandCell::default();
        for t in &self.total {
            c.merge(t);
        }
        c
    }

    /// Almeno un dev mostrato ha la capacità dichiarata.
    pub fn any_declared(&self) -> bool {
        self.rows.iter().any(|r| r.declared)
    }
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

/// I lunedì delle settimane del periodo: quelle con il lunedì in `[from, to]`.
pub fn period_mondays(from: i32, to: i32) -> Vec<WeekId> {
    let mut first = crate::date_utils::dates::local_to_days(&crate::date_utils::dates::primo_giorno_settimana_corrente(
        &days_to_local(from),
    ));
    if first < from {
        first += WEEK_STEP as i32;
    }
    (first..=to).step_by(WEEK_STEP).map(|d| WeekId(d as usize)).collect()
}

/// Worker che contano per la capacità: né archiviati (tolti da footer ed
/// elenchi) né ghost.
pub fn capacity_workers(app: &App) -> Vec<WorkerId> {
    app.workers
        .list()
        .into_iter()
        .map(|(id, _)| id)
        .filter(|&id| id != WORKER_ID_ZERO && !app.workers.is_archived(id) && !app.workers.is_ghost(id))
        .collect()
}

/// Quote proposte per `worker` dalle sue assegnazioni degli ultimi 6 mesi
/// (26 settimane prima di `now`, esclusa): ore per dev su tutti i progetti in
/// percentuale, intere, che sommano a 100 (resti più grandi). Vuoto senza ore.
pub fn propose_shares(app: &App, worker: WorkerId, now: WeekId) -> Vec<(DevId, u32)> {
    const WEEKS: usize = 26;
    let since = WeekId(now.0.saturating_sub(WEEKS * WEEK_STEP));
    let mut hours: Vec<(DevId, usize)> = Vec::new();
    for (proj, _) in app.projects.list() {
        for dev in app.projects.list_devs(proj) {
            let Some(sd) = app.projects.get_single_dev(proj, dev) else { continue };
            for wk in sd.get_weeks().into_iter().filter(|w| *w >= since && *w < now) {
                let h = sd.get_all(wk).and_then(|s| s.worker_id.get(&worker)).map_or(0, |e| e.get_effort().0);
                if h > 0 {
                    match hours.iter_mut().find(|(d, _)| *d == dev) {
                        Some((_, x)) => *x += h,
                        None => hours.push((dev, h)),
                    }
                }
            }
        }
    }
    let total: usize = hours.iter().map(|(_, h)| h).sum();
    if total == 0 {
        return Vec::new();
    }
    // Parte intera, poi i punti mancanti ai resti più grandi.
    let mut parts: Vec<(DevId, u32, usize)> =
        hours.iter().map(|&(d, h)| (d, (h * 100 / total) as u32, (h * 100) % total)).collect();
    let missing = 100 - parts.iter().map(|(_, p, _)| p).sum::<u32>();
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&a, &b| parts[b].2.cmp(&parts[a].2).then(parts[a].0.cmp(&parts[b].0)));
    for &i in order.iter().take(missing as usize) {
        parts[i].1 += 1;
    }
    let mut out: Vec<(DevId, u32)> = parts.into_iter().filter(|(_, p, _)| *p > 0).map(|(d, p, _)| (d, p)).collect();
    out.sort_by_key(|(d, _)| *d);
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
    let cap_workers = capacity_workers(app);
    let mut rows: Vec<DemandRow> = devs
        .iter()
        .map(|&dev| DemandRow {
            dev,
            cells: vec![DemandCell::default(); cols.len()],
            declared: cap_workers.iter().any(|&w| app.workers.dev_share(w, dev) > 0),
        })
        .collect();
    // Capacità: per ogni settimana del periodo, le ore massime effettive di
    // ogni worker × la sua quota sul dev (regola di Saturazione: limiti di
    // settimana, ferie solo se il limite è 0).
    for w in period_mondays(from, to) {
        let Some(i) = col_of(w.0 as i32) else { continue };
        for &wid in &cap_workers {
            let max = app.workers.get_effective_max_hours(wid, w.0) as f64;
            for &(dev, pct) in app.workers.dev_shares(wid) {
                if let Some(row) = rows.iter_mut().find(|r| r.dev == dev) {
                    row.cells[i].add_capacity(wid, max * pct as f64 / 100.0);
                }
            }
        }
    }
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

    /// Capacità per dev: ore massime effettive di ogni settimana del periodo
    /// × la quota del worker, nel mese del lunedì; i limiti di settimana
    /// contano, ghost e archiviati no; scarto = capacità − domanda; un dev con
    /// sola capacità ha la sua riga, uno senza quote è «non dichiarato».
    #[test]
    fn capacity_comes_from_the_worker_shares() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let ghost = app.workers.add("Ghost");
        let gone = app.workers.add("Gone");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let now = monday(2026, 11, 23);
        let wk = |i: usize| WeekId(now.0 + i * WEEK_STEP);
        app.workers.set_dev_shares(alice, &[(fe, 50), (be, 50)]);
        app.workers.set_dev_shares(bob, &[(fe, 100)]);
        app.workers.set_dev_shares(ghost, &[(fe, 100)]);
        app.workers.set_ghost(ghost, true);
        app.workers.set_dev_shares(gone, &[(fe, 100)]);
        app.workers.set_hidden_in_footer(gone, true);
        app.workers.set_shown_in_find(gone, false);
        app.workers.set_week_override(bob, wk(1).0, 0); // ferie
        assert_eq!(capacity_workers(&app), vec![alice, bob]);
        // Domanda: 30h di Frontend assegnate a novembre.
        let p = app.projects.add("A", Some("AAA"), None);
        app.projects.add_dev(p, fe);
        app.projects.add_effort(p, fe, wk(0), alice, Effort(30));
        let none = BTreeSet::new();
        // Novembre: lunedì 23 e 30 → Alice 2 × 20h + Bob 40h + 0h = 80h di Frontend.
        let data = build(&app, now.0 as i32, day(2026, 11, 30), now, &none, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(period_mondays(now.0 as i32, day(2026, 11, 30)), vec![wk(0), wk(1)]);
        let fe_row = data.rows.iter().find(|r| r.dev == fe).unwrap();
        assert!(fe_row.declared);
        assert_eq!(fe_row.cells[0].capacity, 80.0);
        assert_eq!(fe_row.cells[0].gap(), 50.0);
        assert_eq!(fe_row.cells[0].cap_workers_by_weight(), vec![(alice, 40.0), (bob, 40.0)]);
        let be_row = data.rows.iter().find(|r| r.dev == be).expect("solo capacità: riga presente");
        assert_eq!((be_row.cells[0].capacity, be_row.cells[0].total()), (40.0, 0.0));
        assert_eq!(data.grand().capacity, 120.0);
        // Senza quote: «non dichiarato».
        app.workers.set_dev_shares(alice, &[]);
        app.workers.set_dev_shares(bob, &[]);
        let data = build(&app, now.0 as i32, day(2026, 11, 30), now, &none, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(data.rows.len(), 1);
        assert!(!data.rows[0].declared && !data.any_declared());
    }

    /// Proposta delle quote: ore per dev nelle 26 settimane prima di oggi, in
    /// percentuali intere che sommano a 100; vuota senza ore.
    #[test]
    fn proposed_shares_follow_the_last_six_months() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let te = app.devs.add("Test");
        let now = monday(2026, 11, 23);
        let ago = |n: usize| WeekId(now.0 - n * WEEK_STEP);
        let p = app.projects.add("A", Some("AAA"), None);
        for d in [fe, be, te] {
            app.projects.add_dev(p, d);
        }
        assert!(propose_shares(&app, alice, now).is_empty());
        app.projects.add_effort(p, fe, ago(1), alice, Effort(20));
        app.projects.add_effort(p, be, ago(10), alice, Effort(10));
        app.projects.add_effort(p, te, ago(2), alice, Effort(10));
        app.projects.add_effort(p, te, ago(40), alice, Effort(100)); // troppo vecchia
        app.projects.add_effort(p, te, now, alice, Effort(100)); // settimana corrente: no
        assert_eq!(propose_shares(&app, alice, now), vec![(fe, 50), (be, 25), (te, 25)]);
        // La cella di Frontend passa a 10h: tre parti uguali, 34 + 33 + 33.
        app.projects.add_effort(p, fe, ago(1), alice, Effort(10));
        let s = propose_shares(&app, alice, now);
        assert_eq!(s.iter().map(|(_, p)| p).sum::<u32>(), 100);
        assert_eq!(s, vec![(fe, 34), (be, 33), (te, 33)]);
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
