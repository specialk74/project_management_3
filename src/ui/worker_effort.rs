//! Finestra «Effort per worker» (Vista ▸ Effort per worker…): per ogni worker
//! (quelli con la spunta «Elenchi»), una tabella con a sinistra le righe
//! progetto × dev in cui è assegnato, in alto le settimane del periodo Da/A
//! su tre righe (anno, mese, giorno del lunedì) e in mezzo le ore. Solo
//! lettura; i progetti sono quelli del corpo centrale (modalità Vista + filtri
//! Progetti/categorie/anni). Dati in `crate::worker_effort`, condivisi con
//! l'export PDF.

use super::*;
use crate::worker_effort::{WeWorker, WeekMark, day_label, month_of, spans, year_of};

/// Larghezza della colonna progetto × dev.
const WE_LABEL_W: f32 = 160.0;
/// Larghezza di una colonna settimana: basta per «02» e per tre cifre di ore.
const WE_CELL_W: f32 = 24.0;
/// Altezza di una riga dell'intestazione (anno, mese, giorno) e di una riga ore.
const WE_HEAD_H: f32 = 14.0;
const WE_ROW_H: f32 = 16.0;
/// Font dell'intestazione e delle righe.
const WE_HEAD_FONT: f32 = 10.0;
const WE_CELL_FONT: f32 = 11.0;

/// Colore di una settimana di ferie / malattia (come i triangoli del footer).
fn we_status_fill(s: WeekStatus) -> Color32 {
    match s {
        WeekStatus::Ferie => g(Color32::from_rgb(0x33, 0x99, 0xFF)),
        WeekStatus::Malattia => g(Color32::RED),
    }
}

/// Settimana senza ore (totale 0): celle della colonna gialle.
fn we_empty_fill() -> Color32 {
    g(Color32::from_rgb(0xFF, 0xE0, 0x5C))
}

/// Totale oltre le ore max della settimana: cella del totale rossa.
fn we_over_fill() -> Color32 {
    g(Color32::from_rgb(0xD9, 0x26, 0x26))
}

/// Totale sotto l'80% delle ore max della settimana: cella del totale arancione.
fn we_under_fill() -> Color32 {
    g(Color32::from_rgb(0xFF, 0x99, 0x33))
}

/// Mesi abbreviati della riga mese (la GUI è in italiano).
const MONTHS_IT: [&str; 12] = [
    "Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic",
];

/// Progetti della finestra (e del suo PDF): quelli del corpo centrale.
pub(crate) fn worker_effort_projects(app: &App, state: &UiState) -> Vec<ProjectId> {
    body_projects(app, state.project_view, &ProjectFilter::from_state(state))
}

/// Tabelle della finestra (e dei suoi export): i worker con «Elenchi» e, se
/// la combo «Worker» ne ha scelti, solo quelli.
pub(crate) fn worker_effort_data(app: &App, state: &UiState, weeks: &[i32]) -> Vec<WeWorker> {
    let projects = worker_effort_projects(app, state);
    let mut data = crate::worker_effort::build(app, &projects, weeks);
    data.retain(|w| ticked_in_all(&state.worker_effort_workers, &w.id));
    data
}

/// Larghezza della combo «Worker».
const WE_WORKERS_COMBO_W: f32 = 160.0;

/// Combo «Worker» (worker con «Elenchi»): come le altre combo, «tutti» =
/// tutti spuntati, si tolgono uno a uno, l'ultimo rimasto non si toglie.
fn workers_combo(ui: &mut egui::Ui, app: &App, set: &mut std::collections::BTreeSet<WorkerId>) {
    // Solo i worker di «Elenchi»: uno scelto e poi tolto da lì non conta più.
    let items: Vec<TickItem<WorkerId>> =
        matrix_candidates(app).into_iter().map(|(w, name)| TickItem::plain(w, name)).collect();
    TickCombo {
        salt: "worker_effort_workers",
        title: "Worker",
        all_word: "tutti",
        width: WE_WORKERS_COMBO_W,
        height: Some(400.0),
        hover: "Togli la spunta ai worker da nascondere (anche negli export).",
    }
    .show(ui, &items, set);
}

/// Periodo di default: dalla settimana corrente a un anno da oggi (stesso
/// giorno dell'anno prossimo).
fn default_range() -> (i32, i32) {
    let today = today();
    let to = today
        .checked_add_months(chrono::Months::new(12))
        .map_or(today_days() + 365, |d| local_to_days(&d));
    (current_monday_days(), to)
}

/// Altezza del grafico sopra la tabella e margini interni (sopra e sotto).
const WE_CHART_H: f32 = 110.0;
const WE_CHART_PAD: f32 = 8.0;

/// Colore della linea delle ore (la linea del max è rossa).
fn we_hours_color() -> Color32 {
    g(Color32::from_rgb(0x5B, 0x8D, 0xEF))
}

/// Grafico sopra la tabella di un worker, con le colonne della tabella: in x
/// le settimane (un punto al centro di ogni colonna), in y la percentuale
/// **normalizzata al max della settimana** (limite della settimana, max del
/// worker o 40h): linea blu = totale della settimana ÷ max, linea rossa = il
/// max, quindi sempre 100%. Nelle settimane di
/// ferie, malattia o con max 0 entrambe le linee si interrompono; un tratto di
/// una sola settimana è un punto. Nella colonna delle etichette la scala e la
/// legenda; passando col mouse, data, ore e max della settimana.
fn draw_worker_chart(ui: &mut egui::Ui, w: &WeWorker, weeks: &[i32]) {
    let n = weeks.len();
    let size = egui::vec2(WE_LABEL_W + n as f32 * WE_CELL_W, WE_CHART_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect);
    let x0 = rect.left() + WE_LABEL_W;
    let col_mid = |i: usize| x0 + (i as f32 + 0.5) * WE_CELL_W;
    let (y_bot, y_top) = (rect.bottom() - WE_CHART_PAD, rect.top() + WE_CHART_PAD);
    let top = w.chart_top(0..n) as f32;
    let y_of = |h: f32| y_bot - (y_bot - y_top) * h / top;
    let font = egui::FontId::proportional(WE_HEAD_FONT);
    let thin = Stroke::new(1.0, strip_border());
    // Griglia: 0, 50%, 100% e il fondo scala se è oltre, con le percentuali
    // nella colonna delle etichette.
    let mut grid = vec![0.0, 50.0, 100.0];
    if top > 100.0 {
        grid.push(top);
    }
    for h in grid {
        let y = y_of(h);
        p.hline(x0..=rect.right(), y, thin);
        p.text(
            egui::pos2(x0 - 4.0, y),
            Align2::RIGHT_CENTER,
            format!("{}%", h as u32),
            font.clone(),
            text_dim(),
        );
    }
    p.vline(x0, y_top..=y_bot, thin);
    // Legenda in alto a sinistra.
    let legend = [("Ore", we_hours_color()), ("Max", g(error_red()))];
    for (k, (label, color)) in legend.iter().enumerate() {
        let y = rect.top() + 12.0 + k as f32 * 14.0;
        let lx = rect.left() + 4.0;
        p.line_segment([egui::pos2(lx, y), egui::pos2(lx + 14.0, y)], Stroke::new(2.0, *color));
        p.text(egui::pos2(lx + 18.0, y), Align2::LEFT_CENTER, *label, font.clone(), text());
    }
    // Le due linee, un tratto per ogni gruppo di settimane senza interruzioni.
    let hours = |i: usize| w.chart_pct(i);
    let max = |_: usize| 100.0_f32;
    for (a, b) in w.chart_runs(0..n) {
        for (value, color) in [(&max as &dyn Fn(usize) -> f32, g(error_red())), (&hours, we_hours_color())] {
            let pts: Vec<egui::Pos2> = (a..=b).map(|i| egui::pos2(col_mid(i), y_of(value(i)))).collect();
            if pts.len() == 1 {
                p.circle_filled(pts[0], 2.5, color);
            } else {
                p.add(egui::Shape::line(pts, Stroke::new(2.0, color)));
            }
        }
    }
    // Tooltip della settimana sotto il puntatore.
    if let Some(pos) = resp.hover_pos()
        && pos.x >= x0
    {
        let i = ((pos.x - x0) / WE_CELL_W) as usize;
        if let Some(&wk) = weeks.get(i) {
            let detail = match w.status.get(i).copied().flatten() {
                Some(WeekStatus::Ferie) => "ferie".to_string(),
                Some(WeekStatus::Malattia) => "malattia".to_string(),
                None if w.max.get(i).copied().unwrap_or(0) == 0 => "max 0h".to_string(),
                None => format!(
                    "{}h su {}h ({}%)",
                    w.week_total(i),
                    w.max[i],
                    w.chart_pct(i).round() as u32
                ),
            };
            resp.on_hover_text_at_pointer(format!("{}: {detail}", week_label(wk)));
        }
    }
}

/// Tabella di un worker, disegnata a coordinate: tre righe di intestazione
/// (anno e mese raggruppati sulle loro settimane, poi il giorno del lunedì),
/// poi una riga per progetto × dev con le ore e in fondo la riga dei totali
/// (rossa oltre il max della settimana, arancione sotto l'80%; colonne senza
/// ore gialle).
fn draw_worker_table(ui: &mut egui::Ui, w: &WeWorker, weeks: &[i32], this_week: i32) {
    let n = weeks.len();
    let head_h = 3.0 * WE_HEAD_H;
    let cell_font = egui::FontId::proportional(WE_CELL_FONT);
    // Etichette impaginate alla larghezza della colonna: un nome che non ci sta
    // va a capo e la riga si alza quanto serve.
    let labels: Vec<std::sync::Arc<egui::Galley>> = w
        .rows
        .iter()
        .map(|r| {
            ui.painter()
                .layout(w.row_label(r), cell_font.clone(), text(), WE_LABEL_W - 8.0)
        })
        .collect();
    let heights: Vec<f32> = labels
        .iter()
        .map(|g| WE_ROW_H.max(g.size().y + 4.0))
        .collect();
    let size = egui::vec2(
        WE_LABEL_W + n as f32 * WE_CELL_W,
        head_h + heights.iter().sum::<f32>() + WE_ROW_H,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect);
    let x0 = rect.left() + WE_LABEL_W;
    let col_x = |i: usize| x0 + i as f32 * WE_CELL_W;
    let head_font = egui::FontId::proportional(WE_HEAD_FONT);
    let thin = Stroke::new(1.0, strip_border());

    p.rect_filled(
        Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + head_h)),
        0.0,
        strip_bg(false),
    );
    // Righe anno e mese: un'etichetta centrata (e ritagliata) sopra il gruppo.
    let span_row = |row: usize, items: Vec<(String, usize, usize)>, bold: bool| {
        let y0 = rect.top() + row as f32 * WE_HEAD_H;
        for (label, a, b) in items {
            let cell = Rect::from_min_max(
                egui::pos2(col_x(a), y0),
                egui::pos2(col_x(b + 1), y0 + WE_HEAD_H),
            );
            let color = if bold { text() } else { text_dim() };
            p.with_clip_rect(cell.intersect(rect)).text(
                cell.center(),
                Align2::CENTER_CENTER,
                label,
                head_font.clone(),
                color,
            );
            // Confine del gruppo, fino in fondo all'intestazione.
            p.vline(cell.left(), y0..=rect.top() + head_h, thin);
        }
    };
    span_row(
        0,
        spans(weeks, year_of)
            .into_iter()
            .map(|(y, a, b)| (y.to_string(), a, b))
            .collect(),
        true,
    );
    span_row(
        1,
        spans(weeks, month_of)
            .into_iter()
            .map(|((_, m), a, b)| (MONTHS_IT[(m - 1) as usize].to_string(), a, b))
            .collect(),
        false,
    );
    let y_day = rect.top() + 2.0 * WE_HEAD_H;
    for (i, &wk) in weeks.iter().enumerate() {
        let cell = Rect::from_min_max(
            egui::pos2(col_x(i), y_day),
            egui::pos2(col_x(i + 1), y_day + WE_HEAD_H),
        );
        let (font, color) = if wk == this_week {
            (egui::FontId::proportional(WE_HEAD_FONT + 1.0), text())
        } else {
            (head_font.clone(), text_dim())
        };
        p.text(cell.center(), Align2::CENTER_CENTER, day_label(wk), font, color);
        if wk == this_week {
            // grassetto simulato: l'app ha un solo peso di font
            p.text(
                cell.center() + egui::vec2(HI_BOLD_DX, 0.0),
                Align2::CENTER_CENTER,
                day_label(wk),
                egui::FontId::proportional(WE_HEAD_FONT + 1.0),
                color,
            );
        }
    }
    p.hline(rect.x_range(), rect.top() + head_h, thin);

    // Righe progetto × dev (altezza secondo le righe dell'etichetta).
    let mut y0 = rect.top() + head_h;
    for (ri, (r, label)) in w.rows.iter().zip(labels).enumerate() {
        let row = Rect::from_min_max(
            egui::pos2(rect.left(), y0),
            egui::pos2(rect.right(), y0 + heights[ri]),
        );
        y0 = row.bottom();
        p.rect_filled(row, 0.0, if ri % 2 == 0 { row_even() } else { row_alt() });
        let label_pos = egui::pos2(rect.left() + 4.0, row.center().y - label.size().y / 2.0);
        p.galley(label_pos, label, text());
        for (i, h) in r.hours.iter().enumerate() {
            // Ferie (blu) / malattia (rosso): tutta la cella, ore in bianco
            // sopra; settimana senza ore: gialla, testo nero.
            let cell = Rect::from_min_max(
                egui::pos2(col_x(i), row.top()),
                egui::pos2(col_x(i + 1), row.bottom()),
            );
            let status = w.status.get(i).copied().flatten();
            let color = if let Some(st) = status {
                p.rect_filled(cell, 0.0, we_status_fill(st));
                Color32::WHITE
            } else if w.week_mark(i) == Some(WeekMark::Empty) {
                p.rect_filled(cell, 0.0, we_empty_fill());
                Color32::BLACK
            } else {
                text()
            };
            if let Some(h) = h {
                p.text(
                    egui::pos2(col_x(i) + WE_CELL_W / 2.0, row.center().y),
                    Align2::CENTER_CENTER,
                    h.to_string(),
                    cell_font.clone(),
                    color,
                );
            }
        }
    }
    // Riga dei totali: oltre il max rossa e sotto l'80% arancione (anche in
    // ferie/malattia, è un avviso), senza ore gialla, altrimenti il colore
    // della settimana.
    let row = Rect::from_min_max(egui::pos2(rect.left(), y0), rect.max);
    p.rect_filled(row, 0.0, strip_bg(false));
    p.hline(rect.x_range(), y0, thin);
    p.text(
        egui::pos2(rect.left() + 4.0, row.center().y),
        Align2::LEFT_CENTER,
        "Totale",
        cell_font.clone(),
        text(),
    );
    for i in 0..n {
        let cell = Rect::from_min_max(
            egui::pos2(col_x(i), row.top()),
            egui::pos2(col_x(i + 1), row.bottom()),
        );
        let (fill, color) = match (w.week_mark(i), w.status.get(i).copied().flatten()) {
            (Some(WeekMark::Over), _) => (Some(we_over_fill()), Color32::WHITE),
            (Some(WeekMark::Under), _) => (Some(we_under_fill()), Color32::BLACK),
            (_, Some(st)) => (Some(we_status_fill(st)), Color32::WHITE),
            (Some(WeekMark::Empty), _) => (Some(we_empty_fill()), Color32::BLACK),
            (None, None) => (None, text()),
        };
        if let Some(f) = fill {
            p.rect_filled(cell, 0.0, f);
        }
        p.text(
            cell.center(),
            Align2::CENTER_CENTER,
            w.week_total(i).to_string(),
            cell_font.clone(),
            color,
        );
    }
    // Confini dei mesi anche sulle righe, per seguire le colonne.
    for (_, a, _) in spans(weeks, month_of) {
        p.vline(col_x(a), rect.top() + head_h..=rect.bottom(), thin);
    }
    p.vline(x0, rect.y_range(), thin);
    // Settimana corrente: bordo verde come nella griglia, dalla riga dei
    // giorni fino al totale, sopra a tutto (anche ai colori ferie/malattia).
    if let Some(i) = weeks.iter().position(|&wk| wk == this_week) {
        let col = Rect::from_x_y_ranges(col_x(i)..=col_x(i + 1), y_day..=rect.bottom());
        paint_this_week_border_w(ui, col, true, true, THIS_WEEK_BORDER_THIN_W);
    }
}

pub(crate) fn worker_effort_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_worker_effort {
        return;
    }
    let open = os_window(ctx, "worker_effort_window", "Effort per worker", egui::vec2(900.0, 560.0), |ui| {
            let (mut from, mut to) = state.worker_effort_range.unwrap_or_else(default_range);
            let mut changed = false;
            let weeks = crate::worker_effort::weeks(from, to);
            let data = worker_effort_data(app, state, &weeks);
            let any_rows = data.iter().any(|w| !w.rows.is_empty());
            // Cambio a mano di Da/A (o «Da oggi a un anno»): svuota gli anni.
            let mut manual = false;
            let mut years_changed = false;
            ui.horizontal(|ui| {
                let picker = |ui: &mut egui::Ui, label: &str, salt: &str, day: &mut i32| -> bool {
                    ui.label(egui::RichText::new(label).strong());
                    let mut date = days_to_jiff(*day);
                    let r = ui.add(
                        egui_extras::DatePickerButton::new(&mut date)
                            .id_salt(salt)
                            .format("%Y-%m-%d"),
                    );
                    if r.changed()
                        && let Some(d) = jiff_to_days(date)
                    {
                        *day = d;
                        return true;
                    }
                    false
                };
                manual |= picker(ui, "Da:", "worker_effort_from", &mut from);
                ui.separator();
                // Come in matrice e torte: gli anni impostano Da/A (1/1 del più
                // vecchio → 31/12 del più recente).
                years_changed = years_combo(ui, app, "worker_effort_years", &mut state.worker_effort_years);
                ui.separator();
                manual |= picker(ui, "A:", "worker_effort_to", &mut to);
                ui.separator();
                workers_combo(ui, app, &mut state.worker_effort_workers);
                ui.separator();
                if ui
                    .button("Da oggi a un anno")
                    .on_hover_text("Rimette il periodo di partenza: da oggi a un anno da oggi.")
                    .clicked()
                {
                    (from, to) = default_range();
                    manual = true;
                }
                if years_changed {
                    if let Some(range) = years_range(&state.worker_effort_years) {
                        (from, to) = range;
                        changed = true;
                    }
                } else if manual {
                    state.worker_effort_years.clear();
                    changed = true;
                }
                if ui
                    .add_enabled(any_rows, egui::Button::new("Esporta PDF…"))
                    .on_hover_text("Una pagina (o più) per ogni worker con effort nel periodo.")
                    .clicked()
                {
                    actions.push(Action::ExportWorkerEffort { from, to });
                }
                if ui
                    .add_enabled(any_rows, egui::Button::new("Esporta CSV…"))
                    .on_hover_text("Una riga per worker, progetto, dev e settimana in cui è assegnato (anche a 0 ore), con ferie / malattia.")
                    .clicked()
                {
                    actions.push(Action::ExportWorkerEffortCsv { from, to });
                }
            });
            if changed {
                state.worker_effort_range = Some((from, to));
            }
            ui.label(
                egui::RichText::new(
                    "Progetti della vista corrente (Ctrl+1…4) e dei filtri Progetti, \
                     Categorie e Anni. Celle vuote: worker non assegnato. Blu: ferie, \
                     rosso: malattia. Totale rosso: oltre le ore max della settimana; \
                     arancione: sotto l'80% delle ore max; colonna gialla: nessuna ora \
                     nella settimana. Grafico: ore della settimana in % delle ore max (blu) e il \
                     max al 100% (rosso), interrotti nelle settimane di ferie, malattia o \
                     con max 0.",
                )
                .small()
                .color(text_dim()),
            );
            ui.separator();
            if from > to {
                ui.colored_label(warn_amber(), "La data iniziale è successiva a quella finale.");
                return;
            }
            if data.is_empty() {
                ui.label("Nessun worker da mostrare (spunta «Elenchi»).");
                return;
            }
            let this_week = current_monday_days();
            egui::ScrollArea::both()
                .scroll_source(drag_scroll())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, w) in data.iter().enumerate() {
                        if i > 0 {
                            worker_divider(ui);
                        } else {
                            ui.add_space(6.0);
                        }
                        // Un solo dev: righe con la sola tripletta, il dev va qui.
                        worker_heading(ui, |ui| {
                            ui.label(egui::RichText::new(&w.name).strong().size(15.0));
                            if let Some(dev) = w.only_dev() {
                                ui.label(
                                    egui::RichText::new(format!("— {dev}"))
                                        .size(15.0)
                                        .color(text_dim()),
                                );
                            }
                        });
                        if w.rows.is_empty() {
                            ui.label(
                                egui::RichText::new("Nessun effort nel periodo.").color(text_dim()),
                            );
                            continue;
                        }
                        draw_worker_chart(ui, w, &weeks);
                        draw_worker_table(ui, w, &weeks, this_week);
                    }
                });
        });
    if !open {
        state.show_worker_effort = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_shows_each_worker_with_year_month_day_rows_and_hours() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        app.workers.add("Bob");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        // Tre settimane a cavallo dell'anno: lun 21/12/2026, 28/12, 4/1/2027.
        let d = |y, m, dd| local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        let first = d(2026, 12, 21);
        app.projects.add_effort(p, fe, WeekId(first as usize), alice, Effort(12));
        app.projects.add_effort(p, fe, WeekId((first + 14) as usize), alice, Effort(0));
        let mut state = UiState {
            show_worker_effort: true,
            worker_effort_range: Some((first, first + 14)),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        let mut texts = Vec::new();
        for _ in 0..2 {
            let mut actions = Vec::new();
            let out = ctx.run_ui_test(input.clone(), |ui| {
                worker_effort_window(ui.ctx(), &app, &mut state, &mut actions);
            });
            texts = drawn_texts(&out);
        }
        let count = |t: &str| texts.iter().filter(|x| *x == t).count();
        for t in [
            // Un solo dev: l'etichetta è solo la tripletta, il dev accanto al nome.
            "Alice", "— Frontend", "ABC", "12", "0", "Bob", "Nessun effort nel periodo.", "2026",
            "2027", "Dic", "Gen", "21", "28", "04",
        ] {
            assert!(count(t) > 0, "{t}: {texts:?}");
        }
        // Anno e mese una volta per gruppo, non una per settimana.
        assert_eq!(count("2026"), 1, "{texts:?}");
        assert_eq!(count("Dic"), 1, "{texts:?}");
        assert!(
            !texts.iter().any(|x| *x == week_label(first)),
            "niente più date complete «yy-mm-dd» in intestazione"
        );
    }

    /// Settimana di ferie: tutta la colonna blu (anche le righe senza
    /// assegnazione), le ore in bianco; malattia: rossa.
    #[test]
    fn vacation_and_sick_weeks_fill_the_whole_column() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(12));
        app.projects.add_effort(p, be, WeekId((base + 7) as usize), alice, Effort(3));
        app.workers.set_week_status(alice, base as usize, Some(WeekStatus::Ferie));
        app.workers.set_week_status(alice, (base + 7) as usize, Some(WeekStatus::Malattia));
        // Limiti pari alle ore: niente arancione, il totale ha il colore della settimana.
        app.workers.set_week_override(alice, base as usize, 12);
        app.workers.set_week_override(alice, (base + 7) as usize, 3);
        let mut state = UiState {
            show_worker_effort: true,
            worker_effort_range: Some((base, base + 7)),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            let mut actions = Vec::new();
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                worker_effort_window(ui.ctx(), &app, &mut state, &mut actions);
            }));
        }
        let out = out.unwrap();
        // La finestra appare in dissolvenza: si confronta il colore senza
        // l'opacità (i componenti sono premoltiplicati).
        let rgb = |c: Color32| {
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            [r / 8, g / 8, b / 8]
        };
        let fills = |c: Color32| {
            out.shapes
                .iter()
                .filter(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.fill.a() > 0 && rgb(r.fill) == rgb(c)))
                .count()
        };
        // Due righe più la cella del totale.
        assert_eq!(fills(we_status_fill(WeekStatus::Ferie)), 3, "ferie: due righe + totale");
        assert_eq!(fills(we_status_fill(WeekStatus::Malattia)), 3, "malattia: due righe + totale");
        // Ferie senza ore resta blu: niente giallo.
        assert_eq!(fills(we_empty_fill()), 0);
        let white: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if rgb(t.fallback_color) == rgb(Color32::WHITE) => {
                    Some(t.galley.text().to_string())
                }
                _ => None,
            })
            .collect();
        // (col tema scuro anche il testo normale è chiaro: basta che le ore lo siano)
        for h in ["12", "3"] {
            assert!(white.iter().any(|t| t == h), "{h} in bianco: {white:?}");
        }
    }

    /// Riga «Totale» sotto le settimane: rossa (numero bianco) oltre le ore max,
    /// gialla con tutta la colonna dove non ci sono ore.
    #[test]
    fn week_totals_are_red_over_max_and_empty_weeks_are_yellow() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        app.workers.set_max_hours(alice, 10);
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(7));
        app.projects.add_effort(p, be, WeekId(base as usize), alice, Effort(6));
        app.projects.add_effort(p, fe, WeekId((base + 7) as usize), alice, Effort(0));
        // 5h su 10: sotto l'80%.
        app.projects.add_effort(p, fe, WeekId((base + 14) as usize), alice, Effort(5));
        let ws = [base, base + 7, base + 14];
        let data = crate::worker_effort::build(&app, &[p], &ws);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                draw_worker_table(ui, &data[0], &ws, base);
            }));
        }
        let out = out.unwrap();
        let texts = drawn_texts(&out);
        for t in ["Totale", "13"] {
            assert!(texts.iter().any(|x| x == t), "{t}: {texts:?}");
        }
        let fills = |c: Color32| {
            out.shapes
                .iter()
                .filter(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.fill == c))
                .count()
        };
        assert_eq!(fills(we_over_fill()), 1, "13h > 10h: la cella del totale");
        assert_eq!(fills(we_empty_fill()), 3, "settimana a 0: due righe + il totale");
        assert_eq!(fills(we_under_fill()), 1, "5h < 8h: solo la cella del totale");
        let white_13 = out.shapes.iter().any(|cs| {
            matches!(&cs.shape, egui::Shape::Text(t)
                if t.galley.text() == "13" && t.fallback_color == Color32::WHITE)
        });
        assert!(white_13, "totale oltre il max in bianco sul rosso");
    }

    /// Settimana corrente: bordo verde come nella griglia (4 lati: chiuso
    /// sulla riga dei giorni e sul totale), solo sulla sua colonna.
    #[test]
    fn current_week_has_the_green_border() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(7));
        let ws = [base - 7, base, base + 7];
        let data = crate::worker_effort::build(&app, &[p], &ws);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        let out = ctx.run_ui_test(input, |ui| draw_worker_table(ui, &data[0], &ws, base));
        let green: Vec<Rect> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.fill == this_week_border() => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(green.len(), 4, "{green:?}");
        let left = green.iter().map(|r| r.left()).fold(f32::MAX, f32::min);
        let right = green.iter().map(|r| r.right()).fold(f32::MIN, f32::max);
        assert!((right - left - WE_CELL_W).abs() < 0.5, "una colonna: {}", right - left);
        let none = ctx.run_ui_test(
            egui::RawInput { screen_rect: Some(screen(1200.0, 800.0)), ..Default::default() },
            |ui| draw_worker_table(ui, &data[0], &ws, base + 700),
        );
        assert!(
            !none.shapes.iter().any(|cs| matches!(&cs.shape,
                egui::Shape::Rect(r) if r.fill == this_week_border())),
            "settimana corrente fuori periodo: nessun bordo"
        );
    }

    #[test]
    fn default_period_goes_from_this_week_to_one_year_from_today() {
        let (from, to) = default_range();
        assert_eq!(from, current_monday_days());
        let next_year = today().with_year(today().year() + 1).map(|d| local_to_days(&d));
        // Il 29/2 non esiste l'anno dopo: `checked_add_months` va al 28/2.
        assert!(next_year.is_none_or(|d| d == to), "{to}");
        assert!((365..=366).contains(&(to - today_days())));
    }

    /// Un nome più largo della colonna va a capo (niente «…») e la riga si
    /// alza: la tabella è più alta di quella con nomi corti.
    #[test]
    fn long_labels_wrap_and_make_the_row_taller() {
        let table_h = |trip: &str| {
            let mut app = App::new();
            let alice = app.workers.add("Alice");
            let fe = app.devs.add("Frontend");
            let p = app.projects.add("Progetto", Some(trip), None);
            let base = current_monday_days();
            app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(8));
            let data = crate::worker_effort::build(&app, &[p], &[base]);
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(screen(1200.0, 800.0)),
                ..Default::default()
            };
            let mut h = 0.0;
            let mut texts = Vec::new();
            for _ in 0..2 {
                let out = ctx.run_ui_test(input.clone(), |ui| {
                    let top = ui.cursor().top();
                    draw_worker_table(ui, &data[0], &[base], base);
                    h = ui.cursor().top() - top;
                });
                texts = drawn_texts(&out);
            }
            (h, texts)
        };
        let (short_h, _) = table_h("ABC");
        let long = "Progetto con un nome molto lungo da andare a capo";
        let (long_h, texts) = table_h(long);
        assert!(long_h > short_h + 5.0, "{long_h} vs {short_h}");
        assert!(texts.iter().any(|t| t == long), "nome intero: {texts:?}");
        assert!(!texts.iter().any(|t| t.ends_with('…')), "{texts:?}");
    }

    /// Grafico sopra la tabella: legenda Ore / Max, linea rossa del max e
    /// blu delle ore, interrotte nella settimana di ferie (un punto prima, un
    /// tratto dopo).
    #[test]
    fn chart_sits_above_the_table_and_breaks_on_vacation() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        for i in [0, 2, 3] {
            app.projects.add_effort(p, fe, WeekId((base + 7 * i) as usize), alice, Effort(8));
        }
        app.workers.set_week_status(alice, (base + 7) as usize, Some(WeekStatus::Ferie));
        let weeks: Vec<i32> = (0..4).map(|i| base + 7 * i).collect();
        let data = crate::worker_effort::build(&app, &[p], &weeks);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        let out = ctx.run_ui_test(input, |ui| {
            draw_worker_chart(ui, &data[0], &weeks);
            draw_worker_table(ui, &data[0], &weeks, base);
        });
        let max_y = text_pos(&out, "Max").expect("legenda").y;
        let total_y = text_pos(&out, "Totale").expect("tabella").y;
        assert!(text_pos(&out, "Ore").is_some());
        assert!(max_y < total_y, "grafico sopra la tabella");
        let red = g(error_red());
        let (mut paths, mut points) = (0, 0);
        for cs in &out.shapes {
            match &cs.shape {
                egui::epaint::Shape::Path(ps) if ps.stroke.color == egui::epaint::ColorMode::Solid(red) => {
                    paths += 1
                }
                egui::epaint::Shape::Circle(c) if c.fill == red => points += 1,
                _ => {}
            }
        }
        assert_eq!((points, paths), (1, 1), "settimana 0 sola, poi 2-3 dopo le ferie");
    }

    /// Combo «Anni» come nelle torte: spuntare un anno imposta il periodo
    /// dal 1/1 al 31/12; «Da oggi a un anno» la svuota e rimette il periodo
    /// di partenza.
    #[test]
    fn years_combo_sets_the_period_and_manual_change_clears_it() {
        let mut app = App::new();
        let d = |y, m, dd| local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        app.projects.add("Progetto", Some("ABC"), Some(WeekId(d(2026, 3, 2) as usize)));
        let mut state = UiState {
            show_worker_effort: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 800.0)),
            ..Default::default()
        };
        let run = |state: &mut UiState, events: Vec<egui::Event>| {
            let mut i = input.clone();
            i.events = events;
            ctx.run_ui_test(i, |ui| worker_effort_window(ui.ctx(), &app, state, &mut Vec::new()))
        };
        let click = |state: &mut UiState, label: &str| {
            run(state, vec![]);
            let out = run(state, vec![]);
            let pos = text_pos(&out, label).unwrap_or_else(|| panic!("manca «{label}»: {:?}", drawn_texts(&out)));
            for ev in click_events(pos, egui::PointerButton::Primary) {
                run(state, vec![ev]);
            }
        };
        click(&mut state, "Anni: —");
        click(&mut state, "2026");
        assert_eq!(state.worker_effort_years, [2026].into());
        assert_eq!(state.worker_effort_range, Some((d(2026, 1, 1), d(2026, 12, 31))));
        click(&mut state, "Da oggi a un anno");
        assert!(state.worker_effort_years.is_empty(), "cambio a mano: anni svuotati");
        assert_eq!(state.worker_effort_range, Some(default_range()));
    }

    /// Worker ben separati: nome in una fascia colorata e, fra un worker e
    /// l'altro, una linea spessa (una sola per due worker), fra i due nomi.
    #[test]
    fn workers_are_separated_by_a_thick_line_and_a_heading_band() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(8));
        app.projects.add_effort(p, fe, WeekId(base as usize), bob, Effort(8));
        let mut state = UiState {
            show_worker_effort: true,
            worker_effort_range: Some((base, base + 7)),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 1200.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                worker_effort_window(ui.ctx(), &app, &mut state, &mut Vec::new())
            }));
        }
        let out = out.unwrap();
        let lines = worker_divider_rects(&out);
        assert_eq!(lines.len(), 1, "una linea fra due worker");
        let (a, b) = (text_pos(&out, "Alice").unwrap().y, text_pos(&out, "Bob").unwrap().y);
        assert!(a < lines[0].center().y && lines[0].center().y < b, "linea fra i due: {a} {lines:?} {b}");
        let bands = out
            .shapes
            .iter()
            .filter(|cs| matches!(&cs.shape, egui::epaint::Shape::Rect(r) if same_rgb(r.fill, strip_bg(false)) && r.rect.contains(egui::pos2(r.rect.center().x, a))))
            .count();
        assert!(bands >= 1, "nome di Alice su una fascia");
    }

    /// Combo «Worker»: con un worker scelto la finestra (e gli export, che
    /// usano gli stessi dati) mostrano solo lui; vuota = tutti.
    #[test]
    fn worker_filter_limits_the_tables_and_the_exports() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(8));
        app.projects.add_effort(p, fe, WeekId(base as usize), bob, Effort(8));
        let weeks = vec![base];
        let mut state = UiState {
            show_worker_effort: true,
            worker_effort_range: Some((base, base + 6)),
            project_view: ProjectViewMode::All,
            ..Default::default()
        };
        let names = |state: &UiState| -> Vec<String> {
            worker_effort_data(&app, state, &weeks).into_iter().map(|w| w.name).collect()
        };
        assert_eq!(names(&state), ["Alice", "Bob"]);
        state.worker_effort_workers = [bob].into();
        assert_eq!(names(&state), ["Bob"]);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 900.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                worker_effort_window(ui.ctx(), &app, &mut state, &mut Vec::new())
            }));
        }
        let texts = drawn_texts(&out.unwrap());
        assert!(texts.iter().any(|t| t == "Worker: Bob"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Alice"), "Alice filtrata: {texts:?}");
    }

    #[test]
    fn export_button_sends_the_period() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("Progetto", Some("ABC"), None);
        let base = current_monday_days();
        app.projects.add_effort(p, fe, WeekId(base as usize), alice, Effort(8));
        let mut state = UiState {
            show_worker_effort: true,
            worker_effort_range: Some((base, base + 7)),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        let mut button = None;
        let mut actions = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui_test(input.clone(), |ui| {
                worker_effort_window(ui.ctx(), &app, &mut state, &mut actions);
            });
            button = text_pos(&out, "Esporta PDF…");
        }
        for ev in click_events(button.expect("Esporta PDF…"), egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| {
                worker_effort_window(ui.ctx(), &app, &mut state, &mut actions);
            });
        }
        assert!(
            matches!(actions.as_slice(), [Action::ExportWorkerEffort { from, to }] if *from == base && *to == base + 7),
            "attesa una ExportWorkerEffort col periodo"
        );
    }
}
