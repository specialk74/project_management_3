//! Export CSV della matrice effort (File ▸ Esporta CSV…), per analisi esterne
//! e tabelle pivot (Excel, LibreOffice, Python…).
//!
//! Formato «lungo»: **una riga per cella** della griglia (progetto × dev ×
//! worker × settimana), con le colonne descrittive del progetto ripetute su
//! ogni riga, così una pivot può raggruppare per qualunque colonna.
//!
//! Scelte per l'apertura diretta in Excel (anche in italiano): separatore `;`
//! (quello di Excel con le impostazioni italiane), BOM UTF-8 in testa (altrimenti
//! Excel legge male gli accenti), righe `\r\n`, date ISO `yyyy-mm-dd`
//! (ordinabili e riconosciute ovunque). Le ore sono intere, quindi nessun
//! problema di virgola decimale. Intestazioni in inglese, come gli altri export.

use crate::app::App;
use crate::date_utils::dates::days_to_local;
use crate::project_utils::project::{ProjectId, ProjectStatus};
use crate::worker_matrix::week_overlaps;
use crate::workers_utils::worker::WORKER_ID_ZERO;
use chrono::Datelike;

/// Separatore di campo.
pub const SEP: char = ';';

/// Intestazione delle colonne, nell'ordine dei campi di ogni riga.
pub const HEADER: [&str; 12] = [
    "Tripletta", "Project", "Category", "Priority", "Status", "Dev", "Worker", "Week", "Year",
    "Month", "Hours", "Note",
];

/// Opzioni dell'export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CsvOptions {
    /// Solo le settimane che si sovrappongono a `[from, to]` (giorni); `None` =
    /// tutte.
    pub range: Option<(i32, i32)>,
    /// Anche le assegnazioni a 0 ore (di default no: per le somme non
    /// servono, e rendono il file più lungo).
    pub zero: bool,
}

/// Un campo CSV: tra virgolette (raddoppiate dentro) se contiene il
/// separatore, virgolette o a capo.
fn field(s: &str) -> String {
    if s.contains([SEP, '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn status_en(s: ProjectStatus) -> &'static str {
    match s {
        ProjectStatus::Active => "Active",
        ProjectStatus::OnHold => "On-Hold",
        ProjectStatus::Closed => "Closed",
    }
}

/// Righe (senza intestazione) dei progetti `projects`, nell'ordine dato; dentro
/// il progetto per dev (ordine del progetto), settimana, nome del worker.
pub fn effort_rows(app: &App, projects: &[ProjectId], opts: CsvOptions) -> Vec<[String; 12]> {
    let mut out = Vec::new();
    for &proj in projects {
        let trip = app.projects.get_tripletta(proj);
        let name = app.projects.get_info(proj);
        let category = category_name(app, app.projects.get_category(proj));
        let priority = priority_name(app, app.projects.get_priority(proj));
        let status = status_en(app.projects.status(proj)).to_string();
        for dev in app.projects.list_devs(proj) {
            let Some(sd) = app.projects.get_single_dev(proj, dev) else {
                continue;
            };
            let dev_name = app.devs.get(dev).map(|d| d.get_name()).unwrap_or_default();
            for wk in sd.get_weeks() {
                let day = wk.0 as i32;
                if opts.range.is_some_and(|(from, to)| !week_overlaps(day, from, to)) {
                    continue;
                }
                let Some(sew) = sd.get_all(wk) else { continue };
                let [week, year, month] = week_fields(day);
                let mut cells: Vec<(String, u32, String)> = sew
                    .worker_id
                    .iter()
                    .filter(|(w, _)| **w != WORKER_ID_ZERO)
                    .map(|(w, se)| {
                        (
                            app.workers.get_name_by_id(*w).to_string(),
                            se.get_effort().0 as u32,
                            se.get_note(),
                        )
                    })
                    .filter(|(_, h, _)| opts.zero || *h > 0)
                    .collect();
                cells.sort_by(|a, b| a.0.cmp(&b.0));
                for (worker, hours, note) in cells {
                    out.push([
                        trip.clone(),
                        name.clone(),
                        category.clone(),
                        priority.clone(),
                        status.clone(),
                        dev_name.clone(),
                        worker,
                        week.clone(),
                        year.clone(),
                        month.clone(),
                        hours.to_string(),
                        note,
                    ]);
                }
            }
        }
    }
    out
}

/// Il file CSV completo (BOM, intestazione, righe). Ritorna anche il numero di
/// righe di dati, per l'avviso «niente da esportare».
pub fn build_effort_csv(app: &App, projects: &[ProjectId], opts: CsvOptions) -> (String, usize) {
    let rows = effort_rows(app, projects, opts);
    (csv_text(&HEADER, &rows), rows.len())
}

/// Testo CSV per Excel (BOM UTF-8, `;`, righe CRLF, campi tra virgolette quando
/// serve) da un'intestazione e delle righe: condiviso da tutti gli export CSV.
pub fn csv_text<R: AsRef<[String]>>(header: &[&str], rows: &[R]) -> String {
    let sep = SEP.to_string();
    let mut out = String::from("\u{FEFF}");
    out.push_str(&header.iter().map(|h| field(h)).collect::<Vec<_>>().join(&sep));
    out.push_str("\r\n");
    for r in rows {
        let line: Vec<String> = r.as_ref().iter().map(|f| field(f)).collect();
        out.push_str(&line.join(&sep));
        out.push_str("\r\n");
    }
    out
}

/// Colonne data di una settimana (lunedì `days`): `yyyy-mm-dd`, anno, mese.
pub fn week_fields(days: i32) -> [String; 3] {
    let date = days_to_local(days);
    [
        date.format("%Y-%m-%d").to_string(),
        date.year().to_string(),
        date.month().to_string(),
    ]
}

/// Nome della categoria (vuoto = senza).
pub fn category_name(app: &App, c: Option<crate::categories::CategoryId>) -> String {
    c.and_then(|c| app.categories.get_name(c)).unwrap_or_default().to_string()
}

/// Nome della priorità (vuoto = senza).
pub fn priority_name(app: &App, p: Option<crate::priorities::PriorityId>) -> String {
    p.and_then(|p| app.priorities.get_name(p)).unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::single_effort::Effort;

    fn fixture() -> (App, ProjectId, ProjectId, i32) {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let anna = app.workers.add("Anna");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let cat = app.categories.add("Cliente; A");
        let must = app.priorities.list()[0].0;
        let p = app.projects.add("Portale \"nuovo\"", Some("ABC"), None);
        app.projects.set_category(p, Some(cat));
        app.projects.set_priority(p, Some(must));
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, be);
        // Lunedì 2026-01-05.
        let monday = crate::date_utils::dates::local_to_days(
            &chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        );
        let wk = |i: i32| WeekId((monday + 7 * i) as usize);
        app.projects.add_effort(p, fe, wk(0), mario, Effort(10));
        app.projects.add_effort(p, fe, wk(0), anna, Effort(6));
        app.projects.add_effort(p, be, wk(1), mario, Effort(0));
        app.projects.add_effort(p, fe, wk(4), mario, Effort(8));
        app.projects.set_note(p, fe, wk(0), mario, "riga 1\nriga 2");
        let q = app.projects.add("Altro", Some("XYZ"), None);
        app.projects.add_dev(q, be);
        app.projects.add_effort(q, be, wk(0), anna, Effort(3));
        (app, p, q, monday)
    }

    /// Una riga per cella, con le colonne del progetto ripetute; ordine per
    /// progetto, dev, settimana, worker; le 0 ore solo su richiesta.
    #[test]
    fn one_row_per_cell_in_order_and_zero_on_request() {
        let (app, p, q, _) = fixture();
        let rows = effort_rows(&app, &[p, q], CsvOptions::default());
        let short: Vec<_> = rows
            .iter()
            .map(|r| format!("{} {} {} {} {}", r[0], r[5], r[6], r[7], r[10]))
            .collect();
        assert_eq!(
            short,
            [
                "ABC Frontend Anna 2026-01-05 6",
                "ABC Frontend Mario 2026-01-05 10",
                "ABC Frontend Mario 2026-02-02 8",
                "XYZ Backend Anna 2026-01-05 3",
            ]
        );
        let r = &rows[1];
        assert_eq!(
            [&r[2], &r[3], &r[4], &r[8], &r[9]],
            ["Cliente; A", "Must", "Active", "2026", "1"]
        );
        assert_eq!(r[11], "riga 1\nriga 2");
        let all = effort_rows(&app, &[p], CsvOptions { zero: true, ..Default::default() });
        assert!(all.iter().any(|r| r[5] == "Backend" && r[10] == "0"), "assegnazione a 0 ore");
        // Solo i progetti scelti.
        assert_eq!(effort_rows(&app, &[q], CsvOptions::default()).len(), 1);
    }

    /// Il periodo tiene le settimane che lo toccano anche di un solo giorno.
    #[test]
    fn range_keeps_the_overlapping_weeks() {
        let (app, p, _, monday) = fixture();
        // Dalla domenica della prima settimana al lunedì della seconda.
        let opts = CsvOptions { range: Some((monday + 6, monday + 7)), zero: true };
        let weeks: Vec<_> = effort_rows(&app, &[p], opts).iter().map(|r| r[7].clone()).collect();
        assert_eq!(weeks, ["2026-01-05", "2026-01-05", "2026-01-12"]);
    }

    /// File per Excel: BOM, `;`, righe CRLF, campi con `;` / virgolette / a
    /// capo tra virgolette.
    #[test]
    fn csv_text_is_excel_friendly() {
        let (app, p, _, _) = fixture();
        let (csv, n) = build_effort_csv(&app, &[p], CsvOptions::default());
        assert_eq!(n, 3);
        assert!(csv.starts_with("\u{FEFF}Tripletta;Project;Category;Priority;Status;Dev;Worker;Week;"));
        assert!(csv.contains("ABC;\"Portale \"\"nuovo\"\"\";\"Cliente; A\";Must;Active;Frontend;Mario;2026-01-05;2026;1;10;\"riga 1\nriga 2\"\r\n"));
        assert_eq!(csv.matches("\r\n").count(), 1 + n);
        let (empty, n) = build_effort_csv(&app, &[], CsvOptions::default());
        assert_eq!(n, 0);
        assert_eq!(empty, format!("\u{FEFF}{}\r\n", HEADER.join(";")));
    }
}
