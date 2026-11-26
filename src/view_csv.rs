//! Export CSV delle viste (pulsante «Esporta CSV…» in Matrice, Torte,
//! Saturazione, Effort per worker e Domanda e capacità): gli stessi dati mostrati dalla finestra,
//! filtri compresi, in formato «lungo» (una riga per valore, le colonne
//! descrittive ripetute) adatto alle tabelle pivot. Il testo CSV (BOM, `;`,
//! CRLF, virgolette) è quello di `effort_csv::csv_text`; intestazioni in
//! inglese come gli altri export. Nessuna colonna ripete un valore «per
//! worker» (capacità, totali) su più righe: in una pivot verrebbe sommato più
//! volte.

use crate::app::App;
use crate::effort_csv::{category_name, csv_text, priority_name, status_en, week_fields};
use crate::worker_effort::WeWorker;
use crate::worker_matrix::{MatrixData, worker_hours};
use crate::worker_pies::{WorkerPie, pct_of};
use crate::workers_utils::worker::WeekStatus;

/// Colonne del CSV della matrice.
pub const MATRIX_HEADER: [&str; 9] = [
    "Tripletta", "Project", "Category", "Priority", "Status", "Worker", "Dev", "Hours",
    "Worker share %",
];

/// Matrice progetti × worker: una riga per progetto × worker × dev della cella
/// (anche a 0 ore: nella matrice la cella c'è), nell'ordine delle righe della
/// finestra (raggruppamento e ordinamento compresi), poi dei worker. «Worker
/// share %» = ore del dev sul totale del worker nel periodo, come nella cella.
pub fn matrix_rows(app: &App, m: &MatrixData) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for r in &m.rows {
        let category = category_name(app, r.category);
        let priority = priority_name(app, r.priority);
        for (w, (_, worker)) in m.workers.iter().enumerate() {
            let total = m.dev_hours.get(w).map_or(0, |h| worker_hours(h));
            let (Some(devs), Some(hours)) = (r.cells.get(w), r.hours.get(w)) else {
                continue;
            };
            for (dev, &h) in devs.iter().zip(hours) {
                out.push(vec![
                    r.tripletta.clone(),
                    r.name.clone(),
                    category.clone(),
                    priority.clone(),
                    status_en(r.status).to_string(),
                    worker.clone(),
                    dev.clone(),
                    h.to_string(),
                    pct_of(h, total).to_string(),
                ]);
            }
        }
    }
    out
}

/// CSV della matrice e numero di righe di dati.
pub fn matrix_csv(app: &App, m: &MatrixData) -> (String, usize) {
    let rows = matrix_rows(app, m);
    (csv_text(&MATRIX_HEADER, &rows), rows.len())
}

/// Colonne del CSV delle torte.
pub const PIES_HEADER: [&str; 10] = [
    "Worker", "Dev", "Category", "Tripletta", "Project", "Priority", "Status", "Hours", "Done",
    "Planned",
];

/// Torte dev/categorie: una riga per worker × dev × progetto (la fetta più
/// fine dell'anello esterno), con categoria, priorità e stato del progetto; ore totali,
/// fatte (settimane prima della corrente) e pianificate. Sommando per Dev o
/// per Category si ritrovano le due torte.
pub fn pies_rows(app: &App, pies: &[WorkerPie]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for pie in pies {
        for d in &pie.devs {
            let dev = app.devs.get(d.dev).map(|x| x.get_name()).unwrap_or_default();
            for p in &d.projects {
                out.push(vec![
                    pie.name.clone(),
                    dev.clone(),
                    category_name(app, p.category),
                    app.projects.get_tripletta(p.project),
                    app.projects.get_info(p.project),
                    priority_name(app, app.projects.get_priority(p.project)),
                    status_en(app.projects.status(p.project)).to_string(),
                    p.hours.total.to_string(),
                    p.hours.done.to_string(),
                    p.hours.planned().to_string(),
                ]);
            }
        }
    }
    out
}

/// CSV delle torte e numero di righe di dati.
pub fn pies_csv(app: &App, pies: &[WorkerPie]) -> (String, usize) {
    let rows = pies_rows(app, pies);
    (csv_text(&PIES_HEADER, &rows), rows.len())
}

/// Colonne del CSV della capacità delle torte.
pub const PIES_CAPACITY_HEADER: [&str; 8] = [
    "Worker", "Hours", "Done", "Planned", "Capacity", "Used %", "Future capacity", "Free hours",
];

/// Torte, file separato: **una riga per worker** (anche senza ore), con le
/// ore del periodo (filtri compresi), la capacità e l'occupazione %, e — solo
/// se il periodo arriva alla settimana corrente (`split`, come la riga «Da qui
/// a fine periodo» della finestra) — la capacità da qui a fine periodo e le
/// ore libere (negative = oltre la capacità); altrimenti vuote. «Used %» vuota
/// con capacità 0.
pub fn pies_capacity_rows(pies: &[WorkerPie], split: bool) -> Vec<Vec<String>> {
    pies.iter()
        .map(|p| {
            let opt = |v: String| if split { v } else { String::new() };
            vec![
                p.name.clone(),
                p.total.to_string(),
                p.done.to_string(),
                p.planned().to_string(),
                p.capacity.to_string(),
                if p.capacity > 0 { pct_of(p.total, p.capacity).to_string() } else { String::new() },
                opt(p.future_capacity.to_string()),
                opt(p.free_hours().to_string()),
            ]
        })
        .collect()
}

/// CSV della capacità delle torte e numero di righe di dati.
pub fn pies_capacity_csv(pies: &[WorkerPie], split: bool) -> (String, usize) {
    let rows = pies_capacity_rows(pies, split);
    (csv_text(&PIES_CAPACITY_HEADER, &rows), rows.len())
}

/// Ore con una cifra decimale e la **virgola** (come il `;`, per Excel in
/// italiano); intere senza decimali. Per le ore da assegnare, che sono
/// spalmate sulle settimane e quindi frazionarie.
pub fn hours_dec(h: f64) -> String {
    let r = (h * 10.0).round() / 10.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r:.1}").replace('.', ",") }
}

/// Colonne «Period», «Year», «Month» di una colonna di Domanda e capacità.
fn demand_period(c: crate::demand::DemandCol) -> [String; 3] {
    match c {
        crate::demand::DemandCol::Month { year, month } => {
            [format!("{year}-{month:02}"), year.to_string(), month.to_string()]
        }
        crate::demand::DemandCol::NoEnd => ["No end".into(), String::new(), String::new()],
        crate::demand::DemandCol::Late => ["Overdue".into(), String::new(), String::new()],
    }
}

/// Colonne del CSV della domanda per progetto.
pub const DEMAND_HEADER: [&str; 12] = [
    "Dev", "Period", "Year", "Month", "Tripletta", "Project", "Category", "Priority", "Status",
    "Assigned", "To assign", "Demand",
];

/// Domanda e capacità, file della domanda: una riga per dev × periodo (mese,
/// «No end», «Overdue») × progetto che compone la cella, nell'ordine della
/// tabella, con categoria, priorità e stato del progetto. Sommando per Dev e
/// Period si ritrovano le celle della finestra. La capacità sta nell'altro file
/// (non è di un progetto).
pub fn demand_rows(app: &App, data: &crate::demand::DemandData) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for row in &data.rows {
        let dev = app.devs.get(row.dev).map(|x| x.get_name()).unwrap_or_default();
        for (col, cell) in data.cols.iter().zip(&row.cells) {
            let [period, year, month] = demand_period(*col);
            for p in &cell.projects {
                out.push(vec![
                    dev.clone(),
                    period.clone(),
                    year.clone(),
                    month.clone(),
                    app.projects.get_tripletta(p.proj),
                    app.projects.get_info(p.proj),
                    category_name(app, app.projects.get_category(p.proj)),
                    priority_name(app, app.projects.get_priority(p.proj)),
                    status_en(app.projects.status(p.proj)).to_string(),
                    p.assigned.to_string(),
                    hours_dec(p.unassigned),
                    hours_dec(p.assigned as f64 + p.unassigned),
                ]);
            }
        }
    }
    out
}

/// CSV della domanda per progetto e numero di righe di dati.
pub fn demand_csv(app: &App, data: &crate::demand::DemandData) -> (String, usize) {
    let rows = demand_rows(app, data);
    (csv_text(&DEMAND_HEADER, &rows), rows.len())
}

/// Colonne del CSV della capacità per dev.
pub const DEMAND_CAPACITY_HEADER: [&str; 10] = [
    "Dev", "Period", "Year", "Month", "Assigned", "To assign", "Demand", "Capacity", "Gap",
    "Capacity declared",
];

/// Domanda e capacità, file della capacità: una riga per dev × periodo con la
/// domanda della cella, la capacità e lo scarto (capacità − domanda, negativo
/// = mancano ore). Capacità e scarto vuoti nelle colonne «No end» /
/// «Overdue» e per i dev senza capacità dichiarata.
pub fn demand_capacity_rows(app: &App, data: &crate::demand::DemandData) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for row in &data.rows {
        let dev = app.devs.get(row.dev).map(|x| x.get_name()).unwrap_or_default();
        for (col, cell) in data.cols.iter().zip(&row.cells) {
            let month = matches!(col, crate::demand::DemandCol::Month { .. });
            let cap = row.declared && month;
            let [period, year, m] = demand_period(*col);
            out.push(vec![
                dev.clone(),
                period,
                year,
                m,
                cell.assigned.to_string(),
                hours_dec(cell.unassigned),
                hours_dec(cell.total()),
                if cap { hours_dec(cell.capacity) } else { String::new() },
                if cap { hours_dec(cell.gap()) } else { String::new() },
                if row.declared { "Yes" } else { "No" }.to_string(),
            ]);
        }
    }
    out
}

/// CSV della capacità per dev e numero di righe di dati.
pub fn demand_capacity_csv(app: &App, data: &crate::demand::DemandData) -> (String, usize) {
    let rows = demand_capacity_rows(app, data);
    (csv_text(&DEMAND_CAPACITY_HEADER, &rows), rows.len())
}

/// Colonne del CSV dell'effort per worker.
pub const WORKER_EFFORT_HEADER: [&str; 8] =
    ["Worker", "Project", "Dev", "Week", "Year", "Month", "Hours", "Week status"];

fn week_status_en(s: Option<WeekStatus>) -> &'static str {
    match s {
        Some(WeekStatus::Ferie) => "Vacation",
        Some(WeekStatus::Malattia) => "Sick",
        None => "",
    }
}

/// Effort per worker: una riga per worker × riga della sua tabella ×
/// settimana in cui è assegnato (anche a 0 ore, come nella finestra), con
/// ferie / malattia della settimana.
pub fn worker_effort_rows(data: &[WeWorker], weeks: &[i32]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for w in data {
        for r in &w.rows {
            for (i, &week) in weeks.iter().enumerate() {
                let Some(Some(h)) = r.hours.get(i) else { continue };
                let [day, year, month] = week_fields(week);
                out.push(vec![
                    w.name.clone(),
                    r.project.clone(),
                    r.dev.clone(),
                    day,
                    year,
                    month,
                    h.to_string(),
                    week_status_en(w.status.get(i).copied().flatten()).to_string(),
                ]);
            }
        }
    }
    out
}

/// CSV dell'effort per worker e numero di righe di dati.
pub fn worker_effort_csv(data: &[WeWorker], weeks: &[i32]) -> (String, usize) {
    let rows = worker_effort_rows(data, weeks);
    (csv_text(&WORKER_EFFORT_HEADER, &rows), rows.len())
}

/// Colonne del CSV della saturazione.
pub const SATURATION_HEADER: [&str; 8] = [
    "Worker", "Period", "Year", "Month", "Allocated", "Capacity", "Saturation %", "Over",
];

/// Una cella della saturazione già calcolata dalla finestra.
pub struct SatCell<'a> {
    pub worker: &'a str,
    /// Primo lunedì della colonna (giorni).
    pub first_week: i32,
    pub monthly: bool,
    pub alloc: i32,
    pub cap: i32,
}

/// Saturazione: una riga per worker × colonna (settimana o mese, come la
/// vista). «Period» = lunedì `yyyy-mm-dd` o mese `yyyy-mm`; «Saturation %»
/// vuota con capacità 0; «Over» = ore oltre la capacità (0 se dentro).
pub fn saturation_rows(cells: &[SatCell]) -> Vec<Vec<String>> {
    cells
        .iter()
        .map(|c| {
            let [day, year, month] = week_fields(c.first_week);
            let period = if c.monthly {
                format!("{year}-{:0>2}", month)
            } else {
                day
            };
            let pct = if c.cap > 0 {
                ((c.alloc * 100 + c.cap / 2) / c.cap).to_string()
            } else {
                String::new()
            };
            vec![
                c.worker.to_string(),
                period,
                year,
                month,
                c.alloc.to_string(),
                c.cap.to_string(),
                pct,
                (c.alloc - c.cap).max(0).to_string(),
            ]
        })
        .collect()
}

/// CSV della saturazione e numero di righe di dati.
pub fn saturation_csv(cells: &[SatCell]) -> (String, usize) {
    let rows = saturation_rows(cells);
    (csv_text(&SATURATION_HEADER, &rows), rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::single_effort::Effort;
    use std::collections::{BTreeSet, HashSet};

    /// Mario: 10h Frontend sulla prima settimana e 30h Backend sulla seconda
    /// di ABC (Must, categoria «Cliente»); Anna solo assegnata a 0 ore.
    fn fixture() -> (App, HashSet<crate::workers_utils::worker::WorkerId>, i32) {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let anna = app.workers.add("Anna");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let cat = app.categories.add("Cliente");
        let must = app.priorities.list()[0].0;
        let p = app.projects.add("Portale", Some("ABC"), None);
        app.projects.set_category(p, Some(cat));
        app.projects.set_priority(p, Some(must));
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, be);
        let monday = crate::date_utils::dates::local_to_days(
            &chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        );
        let wk = |i: i32| WeekId((monday + 7 * i) as usize);
        app.projects.add_effort(p, fe, wk(0), mario, Effort(10));
        app.projects.add_effort(p, be, wk(1), mario, Effort(30));
        app.projects.add_effort(p, fe, wk(1), anna, Effort(0));
        app.compute_sovra();
        (app, [mario, anna].into_iter().collect(), monday)
    }

    #[test]
    fn matrix_csv_has_a_row_per_worker_and_dev_with_share() {
        let (app, sel, monday) = fixture();
        let m = crate::worker_matrix::build_filtered(
            &app,
            &sel,
            monday,
            monday + 13,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        let mut rows: Vec<String> = matrix_rows(&app, &m).iter().map(|r| r.join("|")).collect();
        rows.sort();
        assert_eq!(
            rows,
            [
                "ABC|Portale|Cliente|Must|Active|Anna|Frontend|0|0",
                "ABC|Portale|Cliente|Must|Active|Mario|Backend|30|75",
                "ABC|Portale|Cliente|Must|Active|Mario|Frontend|10|25",
            ]
        );
        let (csv, n) = matrix_csv(&app, &m);
        assert_eq!(n, 3);
        assert!(csv.starts_with("\u{FEFF}Tripletta;Project;Category;Priority;Status;Worker;Dev;Hours;Worker share %\r\n"));
    }

    #[test]
    fn pies_csv_splits_done_and_planned_per_dev_and_project() {
        let (app, sel, monday) = fixture();
        // «Oggi» = seconda settimana: la prima è fatta, la seconda pianificata.
        let pies = crate::worker_pies::build_filtered(
            &app,
            &sel,
            monday,
            monday + 13,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            monday + 7,
        );
        let mut rows: Vec<String> = pies_rows(&app, &pies).iter().map(|r| r.join("|")).collect();
        rows.sort();
        assert_eq!(
            rows,
            [
                "Mario|Backend|Cliente|ABC|Portale|Must|Active|30|0|30",
                "Mario|Frontend|Cliente|ABC|Portale|Must|Active|10|10|0",
            ]
        );
    }

    #[test]
    fn pies_capacity_csv_has_a_row_per_worker_with_free_hours() {
        let (mut app, sel, monday) = fixture();
        for (id, _) in app.workers.list() {
            app.workers.set_max_hours(id, 40);
        }
        let pies = crate::worker_pies::build_filtered(
            &app,
            &sel,
            monday,
            monday + 13,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            monday + 7,
        );
        let rows: Vec<String> =
            pies_capacity_rows(&pies, true).iter().map(|r| r.join("|")).collect();
        // Mario: 40h di cui 10 fatte, 30 pianificate su 40 disponibili da qui.
        assert!(rows.contains(&"Mario|40|10|30|80|50|40|10".to_string()), "{rows:?}");
        assert!(rows.contains(&"Anna|0|0|0|80|0|40|40".to_string()), "{rows:?}");
        let past = pies_capacity_rows(&pies, false);
        assert!(past.iter().all(|r| r[6].is_empty() && r[7].is_empty()));
        let (csv, n) = pies_capacity_csv(&pies, true);
        assert_eq!(n, 2);
        assert!(csv.starts_with("\u{FEFF}Worker;Hours;Done;Planned;Capacity;Used %;"));
    }

    #[test]
    fn worker_effort_csv_has_assigned_weeks_and_week_status() {
        let (mut app, _, monday) = fixture();
        let anna = app.workers.list().into_iter().find(|(_, n)| n == "Anna").unwrap().0;
        app.workers.set_week_status(anna, (monday + 7) as usize, Some(WeekStatus::Malattia));
        let p = app.projects.list()[0].0;
        let weeks = crate::worker_effort::weeks(monday, monday + 13);
        let data = crate::worker_effort::build(&app, &[p], &weeks);
        let rows: Vec<String> =
            worker_effort_rows(&data, &weeks).iter().map(|r| r.join("|")).collect();
        assert_eq!(
            rows,
            [
                "Anna|ABC|Frontend|2026-01-12|2026|1|0|Sick",
                "Mario|ABC|Frontend|2026-01-05|2026|1|10|",
                "Mario|ABC|Backend|2026-01-12|2026|1|30|",
            ]
        );
    }

    #[test]
    fn saturation_rows_show_period_percent_and_over() {
        let cells = [
            SatCell { worker: "Mario", first_week: 20458, monthly: false, alloc: 50, cap: 40 },
            SatCell { worker: "Anna", first_week: 20458, monthly: true, alloc: 0, cap: 0 },
        ];
        let day = crate::effort_csv::week_fields(20458);
        let rows = saturation_rows(&cells);
        assert_eq!(rows[0][1], day[0]);
        assert_eq!(rows[0][4..], ["50", "40", "125", "10"]);
        assert_eq!(rows[1][1], format!("{}-{:0>2}", day[1], day[2]));
        assert_eq!(rows[1][4..], ["0", "0", "", "0"]);
    }

    /// Decimali con la virgola, interi senza.
    #[test]
    fn hours_use_a_decimal_comma() {
        assert_eq!(hours_dec(12.0), "12");
        assert_eq!(hours_dec(13.333), "13,3");
        assert_eq!(hours_dec(-20.04), "-20");
        assert_eq!(hours_dec(0.06), "0,1");
    }

    /// Domanda e capacità: file della domanda con una riga per dev × periodo ×
    /// progetto (sommando si ritrovano le celle), file della capacità con una
    /// riga per dev × periodo; capacità e scarto solo dove c'è capacità
    /// dichiarata e nei mesi.
    #[test]
    fn demand_csv_has_project_rows_and_capacity_rows() {
        let (mut app, _, monday) = fixture();
        let mario = app.workers.list().into_iter().find(|(_, n)| n == "Mario").unwrap().0;
        let be = app.devs.list().into_iter().find(|(_, n)| n == "Backend").unwrap().0;
        let fe = app.devs.list().into_iter().find(|(_, n)| n == "Frontend").unwrap().0;
        let p = app.projects.list()[0].0;
        // 20h di Frontend da assegnare, senza data di fine; Mario 50% Backend.
        app.projects.add_dev_effort(p, fe, Effort(30));
        app.workers.set_dev_shares(mario, &[(be, 50)]);
        let now = WeekId(monday as usize);
        let none = BTreeSet::new();
        let data = crate::demand::build(&app, monday, monday + 13, now, &none, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());
        let rows = demand_rows(&app, &data);
        let txt: Vec<String> = rows.iter().map(|r| r.join("|")).collect();
        assert_eq!(
            txt,
            [
                "Frontend|2026-01|2026|1|ABC|Portale|Cliente|Must|Active|10|0|10",
                "Frontend|No end|||ABC|Portale|Cliente|Must|Active|0|20|20",
                "Backend|2026-01|2026|1|ABC|Portale|Cliente|Must|Active|30|0|30",
            ]
        );
        let cap: Vec<String> = demand_capacity_rows(&app, &data).iter().map(|r| r.join("|")).collect();
        assert_eq!(
            cap,
            [
                "Frontend|2026-01|2026|1|10|0|10|||No",
                "Frontend|No end|||0|20|20|||No",
                // Backend: due lunedì × 40h × 50% = 40h di capacità, 30h di domanda.
                "Backend|2026-01|2026|1|30|0|30|40|10|Yes",
                "Backend|No end|||0|0|0|||Yes",
            ]
        );
        let (csv, n) = demand_csv(&app, &data);
        assert_eq!(n, 3);
        assert!(csv.starts_with("\u{FEFF}Dev;Period;Year;Month;Tripletta;Project;"));
        assert!(demand_capacity_csv(&app, &data).0.contains("Capacity;Gap;Capacity declared\r\n"));
    }
}
