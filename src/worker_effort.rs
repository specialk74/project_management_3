//! Effort per worker (Vista ▸ Effort per worker…): dati condivisi dalla
//! finestra (`ui::worker_effort`) e dall'export PDF
//! (`pdf_export::build_worker_effort_pdf`).
//!
//! Per ogni worker con la spunta «Elenchi», le righe progetto × dev in cui è
//! **assegnato** (anche a 0 ore) in almeno una settimana del periodo, con le
//! ore settimana per settimana, e lo stato di ogni settimana (ferie /
//! malattia, registrato per settimana dal footer). I progetti li sceglie il
//! chiamante (quelli del corpo centrale).

use std::collections::HashMap;

use chrono::Datelike;

use crate::app::App;
use crate::date_utils::dates::{days_to_local, local_to_days, primo_giorno_settimana_corrente};
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::{WEEK_STEP, WeekId};
use crate::workers_utils::worker::{WORKER_ID_ZERO, WeekStatus};

/// Una riga della tabella di un worker: un dev di un progetto.
pub struct WeRow {
    /// Etichetta del progetto (tripletta, o descrizione).
    pub project: String,
    pub dev: String,
    /// Ore per settimana del periodo (parallelo alle settimane): `None` = non
    /// assegnato, `Some(0)` = assegnato a 0 ore.
    pub hours: Vec<Option<usize>>,
}

/// La tabella di un worker; `rows` vuoto = nessun effort nel periodo.
pub struct WeWorker {
    pub name: String,
    pub rows: Vec<WeRow>,
    /// Ferie / malattia per settimana del periodo (parallelo alle settimane):
    /// colora l'intera colonna della settimana.
    pub status: Vec<Option<WeekStatus>>,
    /// Ore max effettive per settimana del periodo (limite della settimana o
    /// ore max del worker, come il footer): oltre, il totale è rosso.
    pub max: Vec<u32>,
}

/// Come segnalare il totale di una settimana.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeekMark {
    /// Totale oltre le ore max della settimana: cella del totale rossa.
    Over,
    /// Nessuna ora (totale 0, anche se assegnato a 0) in una settimana senza
    /// ferie/malattia: celle della colonna gialle.
    Empty,
    /// Totale sotto l'80% delle ore max della settimana (anche in ferie /
    /// malattia, dove anche 0 lo è): cella del totale arancione.
    Under,
}

impl WeWorker {
    /// Totale delle ore della settimana `i` sulle righe della tabella.
    pub fn week_total(&self, i: usize) -> usize {
        self.rows.iter().filter_map(|r| r.hours.get(i).copied().flatten()).sum()
    }

    /// Segnalazione della settimana `i`: oltre il max prima di tutto; vuota
    /// solo se non è di ferie/malattia; poi sotto l'80% del max (con max 0
    /// mai).
    pub fn week_mark(&self, i: usize) -> Option<WeekMark> {
        let total = self.week_total(i);
        let max = self.max.get(i).copied().unwrap_or(u32::MAX) as usize;
        if total > max {
            Some(WeekMark::Over)
        } else if total == 0 && self.status.get(i).copied().flatten().is_none() {
            Some(WeekMark::Empty)
        } else if total * 5 < max * 4 {
            Some(WeekMark::Under)
        } else {
            None
        }
    }

    /// Il worker lavora su un solo dev in tutte le sue righe (anche in
    /// progetti diversi)?
    pub fn single_dev(&self) -> bool {
        self.rows.iter().all(|r| r.dev == self.rows[0].dev)
    }

    /// Il dev unico del worker, quando lo è (`single_dev`) e ha righe: va
    /// scritto accanto al nome, visto che le righe mostrano solo la tripletta.
    pub fn only_dev(&self) -> Option<&str> {
        (!self.rows.is_empty() && self.single_dev()).then(|| self.rows[0].dev.as_str())
    }

    /// Etichetta di una riga: «Tripletta – Dev» se il worker lavora su più
    /// dev, solo «Tripletta» se il dev è sempre lo stesso.
    pub fn row_label(&self, r: &WeRow) -> String {
        if self.single_dev() {
            r.project.clone()
        } else {
            format!("{} – {}", r.project, r.dev)
        }
    }
}

/// Lunedì delle settimane che si sovrappongono a `[from, to]` (giorni).
pub fn weeks(from: i32, to: i32) -> Vec<i32> {
    let monday = |d: i32| local_to_days(&primo_giorno_settimana_corrente(&days_to_local(d)));
    let last = monday(to);
    let mut out = Vec::new();
    let mut w = monday(from);
    while w <= last {
        out.push(w);
        w += WEEK_STEP as i32;
    }
    out
}

/// Raggruppa settimane consecutive con la stessa chiave: `(chiave, prima,
/// ultima)` come indici in `weeks`. Serve alle righe anno e mese
/// dell'intestazione (un'etichetta sopra tutte le settimane del gruppo).
pub fn spans<K: PartialEq>(weeks: &[i32], key: impl Fn(i32) -> K) -> Vec<(K, usize, usize)> {
    let mut out: Vec<(K, usize, usize)> = Vec::new();
    for (i, &w) in weeks.iter().enumerate() {
        let k = key(w);
        match out.last_mut() {
            Some(last) if last.0 == k => last.2 = i,
            _ => out.push((k, i, i)),
        }
    }
    out
}

/// Anno del lunedì della settimana.
pub fn year_of(week: i32) -> i32 {
    days_to_local(week).year()
}

/// (anno, mese 1–12) del lunedì della settimana: una settimana a cavallo di due
/// mesi sta nel mese del suo lunedì, come l'etichetta della griglia.
pub fn month_of(week: i32) -> (i32, u32) {
    let d = days_to_local(week);
    (d.year(), d.month())
}

/// Giorno del mese del lunedì, a due cifre («02»).
pub fn day_label(week: i32) -> String {
    format!("{:02}", days_to_local(week).day())
}

/// Un `WeWorker` per ogni worker con `show_in_find`, con le righe progetto ×
/// dev (nell'ordine di `projects`, poi dei dev del progetto) in cui è
/// assegnato — anche a 0 ore — in almeno una di `weeks`.
pub fn build(app: &App, projects: &[ProjectId], weeks: &[i32]) -> Vec<WeWorker> {
    let dev_names: HashMap<_, String> = app.devs.list().into_iter().collect();
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && app.workers.is_shown_in_find(*id))
        .map(|(wid, name)| {
            let mut rows = Vec::new();
            for &proj in projects {
                let project = app
                    .projects
                    .get(proj)
                    .map(|p| p.label(proj))
                    .unwrap_or_default();
                for dev in app.projects.list_devs(proj) {
                    let Some(sd) = app.projects.get_single_dev(proj, dev) else {
                        continue;
                    };
                    let hours: Vec<Option<usize>> = weeks
                        .iter()
                        .map(|&w| {
                            sd.get_all(WeekId(w as usize))
                                .and_then(|s| s.worker_id.get(&wid))
                                .map(|e| e.get_effort().0)
                        })
                        .collect();
                    if hours.iter().any(Option::is_some) {
                        rows.push(WeRow {
                            project: project.clone(),
                            dev: dev_names.get(&dev).cloned().unwrap_or_default(),
                            hours,
                        });
                    }
                }
            }
            let status = weeks
                .iter()
                .map(|&w| app.workers.get_week_status(wid, w as usize))
                .collect();
            let max = weeks
                .iter()
                .map(|&w| app.workers.get_effective_max_hours(wid, w as usize))
                .collect();
            WeWorker {
                name,
                rows,
                status,
                max,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date_utils::dates::current_monday_days;
    use crate::single_effort_utils::single_effort::Effort;

    #[test]
    fn weeks_cover_every_week_overlapping_the_period() {
        let base = current_monday_days();
        // Da un mercoledì al lunedì di due settimane dopo: tre settimane.
        assert_eq!(weeks(base + 2, base + 14), vec![base, base + 7, base + 14]);
        assert_eq!(weeks(base + 3, base + 4), vec![base]);
        assert!(weeks(base + 7, base).is_empty());
    }

    #[test]
    fn spans_group_consecutive_weeks_by_year_and_month() {
        // Lunedì 2026-11-23, 30, 2026-12-07, …, 2027-01-04.
        let d = |y, m, dd| local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        let ws: Vec<i32> = (0..7).map(|i| d(2026, 11, 23) + 7 * i).collect();
        assert_eq!(spans(&ws, year_of), vec![(2026, 0, 5), (2027, 6, 6)]);
        assert_eq!(
            spans(&ws, month_of),
            vec![((2026, 11), 0, 1), ((2026, 12), 2, 5), ((2027, 1), 6, 6)]
        );
        assert_eq!(day_label(ws[2]), "07");
    }

    #[test]
    fn rows_are_project_dev_pairs_with_hours_and_zero_assignments() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        app.workers.add("Bob");
        let hidden = app.workers.add("Nascosto");
        app.workers.set_shown_in_find(hidden, false);
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let q = app.projects.add("Altro", Some("XYZ"), None);
        let base = current_monday_days();
        let wk = |i: i32| WeekId((base + 7 * i) as usize);
        app.projects.add_effort(p, fe, wk(0), alice, Effort(8));
        app.projects.add_effort(p, be, wk(1), alice, Effort(0));
        app.projects.add_effort(q, fe, wk(5), alice, Effort(4)); // fuori periodo
        app.projects.add_effort(p, fe, wk(0), hidden, Effort(2));

        let ws = weeks(base, base + 7);
        let data = build(&app, &[p, q], &ws);
        let names: Vec<&str> = data.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, vec!["Alice", "Bob"], "solo i worker con «Elenchi»");

        let a = &data[0];
        let rows: Vec<(String, &[Option<usize>])> =
            a.rows.iter().map(|r| (a.row_label(r), r.hours.as_slice())).collect();
        assert_eq!(
            rows,
            vec![
                ("ABC – Frontend".to_string(), &[Some(8), None][..]),
                ("ABC – Backend".to_string(), &[None, Some(0)][..]),
            ],
            "una riga per progetto × dev, anche a 0 ore; XYZ fuori periodo"
        );
        assert!(data[1].rows.is_empty() && data[1].name == "Bob");
        assert_eq!(a.status, vec![None, None]);

        // Solo i progetti passati (quelli della vista).
        assert!(build(&app, &[q], &ws)[0].rows.is_empty());
    }

    #[test]
    fn dev_is_shown_only_when_the_worker_works_on_several_devs() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let q = app.projects.add("Altro", Some("XYZ"), None);
        let base = current_monday_days();
        let wk = WeekId(base as usize);
        // Alice: due dev ⇒ «Tripletta – Dev».
        app.projects.add_effort(p, fe, wk, alice, Effort(8));
        app.projects.add_effort(q, be, wk, alice, Effort(4));
        // Bob: sempre Frontend, anche in progetti diversi ⇒ solo la tripletta.
        app.projects.add_effort(p, fe, wk, bob, Effort(2));
        app.projects.add_effort(q, fe, wk, bob, Effort(0));
        let data = build(&app, &[p, q], &weeks(base, base));
        let labels = |w: &WeWorker| w.rows.iter().map(|r| w.row_label(r)).collect::<Vec<_>>();
        assert_eq!(labels(&data[0]), ["ABC – Frontend", "XYZ – Backend"]);
        assert_eq!(labels(&data[1]), ["ABC", "XYZ"]);
        // Il dev unico va accanto al nome; con più dev, o senza righe, niente.
        assert_eq!(data[1].only_dev(), Some("Frontend"));
        assert_eq!(data[0].only_dev(), None);
        assert_eq!(build(&app, &[], &weeks(base, base))[0].only_dev(), None);
    }

    #[test]
    fn status_follows_the_vacation_and_sick_weeks() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let base = current_monday_days();
        app.workers.set_week_status(alice, base as usize, Some(WeekStatus::Ferie));
        app.workers.set_week_status(alice, (base + 14) as usize, Some(WeekStatus::Malattia));
        let data = build(&app, &[], &weeks(base, base + 14));
        assert_eq!(
            data[0].status,
            vec![Some(WeekStatus::Ferie), None, Some(WeekStatus::Malattia)]
        );
    }

    #[test]
    fn week_totals_are_red_over_the_week_max_and_yellow_when_empty() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        app.workers.set_max_hours(alice, 10);
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let hidden = app.projects.add("Fuori vista", Some("XYZ"), None);
        let base = current_monday_days();
        let wk = |i: i32| (base + 7 * i) as usize;
        // 0: 6 + 4 = 10, al limite; 1: 6 + 5 = 11, oltre; 2: assegnato a 0;
        // 3: niente; 4: niente ma ferie; 5: 4 con limite settimana a 3;
        // 6: 7 < 8 (80% di 10); 7: 8, proprio l'80%; 8: 0 in ferie a limite 0.
        // 9: 1 in ferie a limite 2 (sotto l'80%).
        app.projects.add_effort(p, fe, WeekId(wk(0)), alice, Effort(6));
        app.projects.add_effort(p, be, WeekId(wk(0)), alice, Effort(4));
        app.projects.add_effort(p, fe, WeekId(wk(1)), alice, Effort(6));
        app.projects.add_effort(p, be, WeekId(wk(1)), alice, Effort(5));
        app.projects.add_effort(p, fe, WeekId(wk(2)), alice, Effort(0));
        app.workers.set_week_status(alice, wk(4), Some(WeekStatus::Ferie));
        app.projects.add_effort(p, fe, WeekId(wk(5)), alice, Effort(4));
        app.workers.set_week_override(alice, wk(5), 3);
        app.projects.add_effort(p, fe, WeekId(wk(6)), alice, Effort(7));
        app.projects.add_effort(p, fe, WeekId(wk(7)), alice, Effort(8));
        app.workers.set_week_status(alice, wk(8), Some(WeekStatus::Ferie));
        app.workers.set_week_override(alice, wk(8), 0);
        app.projects.add_effort(p, fe, WeekId(wk(9)), alice, Effort(1));
        app.workers.set_week_status(alice, wk(9), Some(WeekStatus::Malattia));
        app.workers.set_week_override(alice, wk(9), 2);
        // Un progetto fuori dalla vista non entra nel totale.
        app.projects.add_effort(hidden, fe, WeekId(wk(3)), alice, Effort(40));

        let data = build(&app, &[p], &weeks(base, base + 63));
        let a = &data[0];
        let totals: Vec<usize> = (0..10).map(|i| a.week_total(i)).collect();
        assert_eq!(totals, [10, 11, 0, 0, 0, 4, 7, 8, 0, 1]);
        let marks: Vec<Option<WeekMark>> = (0..10).map(|i| a.week_mark(i)).collect();
        use WeekMark::*;
        assert_eq!(
            marks,
            [
                None,
                Some(Over),
                Some(Empty),
                Some(Empty),
                Some(Under), // ferie: si controlla il max, anche con 0
                Some(Over),
                Some(Under),
                None,
                None, // ferie a limite 0: niente arancione
                Some(Under),
            ]
        );
    }
}
