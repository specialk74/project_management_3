//! Matrice progetti × worker (Vista ▸ Matrice progetti/worker…): prima una
//! dialog per scegliere i worker (la scelta resta per la sessione), poi la
//! finestra con l'intervallo di date e la matrice. I dati vengono da
//! `crate::worker_matrix`, condivisi con l'export PDF.

use super::*;
use crate::worker_matrix::{MatrixData, category_color, closed_colors, hatch_segments};

/// Giorno odierno (locale), nello stesso formato di `WeekId` (giorni).
fn today_days() -> i32 {
    local_to_days(&chrono::Local::now().date_naive())
}

fn days_to_jiff(days: i32) -> jiff::civil::Date {
    let d = days_to_local(days);
    jiff::civil::Date::new(d.year() as i16, d.month() as i8, d.day() as i8)
        .unwrap_or_else(|_| today_jiff())
}

fn jiff_to_days(d: jiff::civil::Date) -> Option<i32> {
    chrono::NaiveDate::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32)
        .map(|n| local_to_days(&n))
}

/// Intervallo `[1 gennaio dell'anno più vecchio, 31 dicembre del più recente]`
/// (giorni) degli anni scelti nella combo; `None` se nessun anno è scelto.
fn years_range(years: &std::collections::BTreeSet<i32>) -> Option<(i32, i32)> {
    let (first, last) = (years.first()?, years.last()?);
    let from = chrono::NaiveDate::from_ymd_opt(*first, 1, 1)?;
    let to = chrono::NaiveDate::from_ymd_opt(*last, 12, 31)?;
    Some((local_to_days(&from), local_to_days(&to)))
}

/// Combo degli anni (anni di inizio/fine dei progetti, come il filtro Anni):
/// scegliendone uno o più, Da/A diventano 1/1 del più vecchio → 31/12 del più
/// recente. Ritorna `true` se la selezione è cambiata.
fn years_combo(ui: &mut egui::Ui, app: &App, years: &mut std::collections::BTreeSet<i32>) -> bool {
    let mut changed = false;
    let text = if years.is_empty() {
        "Anni: —".to_string()
    } else {
        let list: Vec<String> = years.iter().map(i32::to_string).collect();
        format!("Anni: {}", list.join(", "))
    };
    egui::ComboBox::from_id_salt("matrix_years")
        .selected_text(text)
        .width(MX_YEARS_W)
        .show_ui(ui, |ui| {
            let all = available_years(app);
            if all.is_empty() {
                ui.label("Nessun progetto con date di inizio/fine.");
            }
            for y in all {
                let mut on = years.contains(&y);
                if ui.checkbox(&mut on, y.to_string()).changed() {
                    if on {
                        years.insert(y);
                    } else {
                        years.remove(&y);
                    }
                    changed = true;
                }
            }
            if !years.is_empty() {
                ui.separator();
                if ui.button("Deseleziona tutti").clicked() {
                    years.clear();
                    changed = true;
                }
            }
        })
        .response
        .on_hover_text(
            "Imposta Da/A dal 1° gennaio dell'anno più vecchio scelto al 31 dicembre \
             del più recente.",
        );
    changed
}

/// Estremo dell'intervallo: «Oggi» oppure una data scelta col calendario.
/// `value`: `None` = oggi. Ritorna `true` se l'utente l'ha cambiato.
fn range_end_picker(ui: &mut egui::Ui, label: &str, id: &str, value: &mut Option<i32>) -> bool {
    let before = *value;
    ui.label(egui::RichText::new(label).strong());
    if ui.radio(value.is_none(), "Oggi").clicked() {
        *value = None;
    }
    if ui.radio(value.is_some(), "Data:").clicked() && value.is_none() {
        *value = Some(today_days());
    }
    let mut date = days_to_jiff(value.unwrap_or_else(today_days));
    ui.add_enabled_ui(value.is_some(), |ui| {
        let r = ui.add(
            egui_extras::DatePickerButton::new(&mut date)
                .id_salt(id)
                .format("%Y-%m-%d"),
        );
        if r.changed() {
            if let Some(d) = jiff_to_days(date) {
                *value = Some(d);
            }
        }
    });
    *value != before
}

/// Worker proponibili nella dialog di selezione: quelli con `show_in_find`
/// (come la colonna Workers dei filtri).
fn matrix_candidates(app: &App) -> Vec<(WorkerId, String)> {
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && app.workers.is_shown_in_find(*id))
        .collect()
}

/// Dialog di scelta dei worker da mostrare nella matrice. La selezione vive in
/// `UiState.matrix_workers` e resta valida finché l'app è aperta.
pub(crate) fn matrix_select_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.matrix_select_open {
        return;
    }
    let candidates = matrix_candidates(app);
    let q = state.matrix_worker_search.trim().to_lowercase();
    let listed: Vec<&(WorkerId, String)> = candidates
        .iter()
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .collect();

    let mut open = true;
    let mut show = false;
    let mut cancel = false;
    egui::Window::new("Matrice progetti × worker — scegli i worker")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(120.0, 80.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.matrix_worker_search)
                    .hint_text("Cerca worker…")
                    .desired_width(240.0),
            );
            let all = !listed.is_empty()
                && listed
                    .iter()
                    .all(|(id, _)| state.matrix_workers.contains(id));
            if let Some(on) = select_all_checkbox(ui, all) {
                for (id, _) in &listed {
                    if on {
                        state.matrix_workers.insert(*id);
                    } else {
                        state.matrix_workers.remove(id);
                    }
                }
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for (id, name) in &listed {
                        let mut on = state.matrix_workers.contains(id);
                        if ui.checkbox(&mut on, name).changed() {
                            if on {
                                state.matrix_workers.insert(*id);
                            } else {
                                state.matrix_workers.remove(id);
                            }
                        }
                    }
                });
            ui.separator();
            let n = candidates
                .iter()
                .filter(|(id, _)| state.matrix_workers.contains(id))
                .count();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(n > 0, egui::Button::new(format!("Mostra matrice ({n})")))
                    .clicked()
                {
                    show = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });
    if show {
        state.show_matrix = true;
    }
    if show || cancel || !open {
        state.matrix_select_open = false;
    }
}

// Geometria della matrice a schermo (px).
const MX_LABEL_W: f32 = 144.0;
const MX_COL_W: f32 = 84.0;
const MX_HEAD_H: f32 = 24.0;
const MX_GROUP_H: f32 = 20.0;
const MX_LINE_H: f32 = 15.0;
const MX_PAD: f32 = 4.0;
const MX_SUM_GAP: f32 = 4.0;
const MX_SUM_H: f32 = 20.0;
const MX_YEARS_W: f32 = 180.0;

fn mx_row_h(lines: usize) -> f32 {
    lines.max(1) as f32 * MX_LINE_H + 2.0 * MX_PAD
}

/// Tronca `s` (con ellissi) finché non entra in `max_w` px col font dato.
fn fit_text(ui: &egui::Ui, s: &str, font: &egui::FontId, max_w: f32) -> String {
    let w = |t: &str| {
        ui.painter()
            .layout_no_wrap(t.to_string(), font.clone(), Color32::BLACK)
            .size()
            .x
    };
    if w(s) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut n = chars.len();
    while n > 0 {
        let t: String = chars[..n].iter().collect::<String>() + "…";
        if w(&t) <= max_w {
            return t;
        }
        n -= 1;
    }
    "…".to_string()
}

/// `0xRRGGBB` → colore egui (passato per il filtro B/N).
fn rgb_color(c: u32) -> Color32 {
    g(Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8))
}

fn category_name(app: &App, cat: Option<CategoryId>) -> String {
    cat.and_then(|c| app.categories.get_name(c))
        .map(str::to_string)
        .unwrap_or_else(|| "Senza categoria".to_string())
}

/// Tinta di sfondo di una categoria (piena per l'intestazione del gruppo,
/// schiarita per le righe dei progetti).
fn category_fill(cat: Option<CategoryId>, light: bool) -> Color32 {
    let c = category_color(cat);
    let (r, gr, b) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
    let mix = |v: u8| {
        if light {
            (255.0 - (255.0 - v as f32) * 0.45) as u8
        } else {
            v
        }
    };
    g(Color32::from_rgb(mix(r), mix(gr), mix(b)))
}

/// Disegna la matrice (righe progetto raggruppate per categoria) e, sotto, il
/// riepilogo dei progetti per categoria di ogni worker.
fn draw_matrix(ui: &mut egui::Ui, app: &App, m: &MatrixData) {
    let n = m.workers.len();
    let width = MX_LABEL_W + n as f32 * MX_COL_W;
    let mut groups = 0;
    let mut prev: Option<Option<CategoryId>> = None;
    let mut body_h = 0.0;
    for r in &m.rows {
        if prev != Some(r.category) {
            groups += 1;
            prev = Some(r.category);
        }
        body_h += mx_row_h(r.cells.iter().map(Vec::len).max().unwrap_or(0));
    }
    let sum_h = MX_SUM_GAP + (m.categories.len() + 1) as f32 * MX_SUM_H;
    let height = MX_HEAD_H + groups as f32 * MX_GROUP_H + body_h + sum_h;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let p = ui.painter().clone();
    let x0 = rect.left();
    let col_x = |i: usize| x0 + MX_LABEL_W + i as f32 * MX_COL_W;
    let border = Stroke::new(1.0_f32, strip_border());
    let bold = egui::FontId::proportional(13.0);
    let font = egui::FontId::proportional(12.5);

    let header = |y: f32, label: &str| {
        let r = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, MX_HEAD_H));
        p.rect_filled(r, 0.0, strip_bg(false));
        p.text(
            egui::pos2(x0 + MX_PAD, y + MX_HEAD_H / 2.0),
            Align2::LEFT_CENTER,
            label,
            bold.clone(),
            text(),
        );
        for (i, (_, name)) in m.workers.iter().enumerate() {
            p.text(
                egui::pos2(col_x(i) + MX_COL_W / 2.0, y + MX_HEAD_H / 2.0),
                Align2::CENTER_CENTER,
                fit_text(ui, name, &bold, MX_COL_W - 2.0 * MX_PAD),
                bold.clone(),
                text(),
            );
        }
        p.hline(x0..=x0 + width, y + MX_HEAD_H, border);
    };
    let vlines = |y0: f32, y1: f32| {
        for x in [x0, x0 + MX_LABEL_W].into_iter().chain((1..=n).map(col_x)) {
            p.vline(x, y0..=y1, border);
        }
    };

    let mut y = rect.top();
    header(y, "Progetto");
    vlines(y, y + MX_HEAD_H);
    y += MX_HEAD_H;

    let mut prev: Option<Option<CategoryId>> = None;
    for r in &m.rows {
        if prev != Some(r.category) {
            prev = Some(r.category);
            let gr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, MX_GROUP_H));
            p.rect_filled(gr, 0.0, category_fill(r.category, false));
            p.text(
                egui::pos2(x0 + MX_PAD, y + MX_GROUP_H / 2.0),
                Align2::LEFT_CENTER,
                category_name(app, r.category),
                bold.clone(),
                Color32::BLACK,
            );
            p.hline(x0..=x0 + width, y + MX_GROUP_H, border);
            y += MX_GROUP_H;
        }
        let h = mx_row_h(r.cells.iter().map(Vec::len).max().unwrap_or(0));
        let rr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, h));
        // Progetto chiuso: tinta della categoria spenta verso il grigio, a strisce.
        let fg = if r.closed {
            let (fill, hatch) = closed_colors(r.category);
            p.rect_filled(rr, 0.0, rgb_color(fill));
            let hatch = Stroke::new(1.0_f32, rgb_color(hatch));
            for (ax, ay, bx, by) in hatch_segments(rr.left(), rr.right(), rr.top(), rr.bottom(), 8.0)
            {
                p.line_segment([egui::pos2(ax, ay), egui::pos2(bx, by)], hatch);
            }
            Color32::from_gray(0x50)
        } else {
            p.rect_filled(rr, 0.0, category_fill(r.category, true));
            Color32::BLACK
        };

        // Etichetta: solo la tripletta (il nome è nel tooltip), più «chiuso».
        let ty = y + MX_PAD + MX_LINE_H / 2.0;
        let trip_w = p
            .text(
                egui::pos2(x0 + MX_PAD, ty),
                Align2::LEFT_CENTER,
                fit_text(ui, &r.tripletta, &bold, MX_LABEL_W - 2.0 * MX_PAD - if r.closed { 50.0 } else { 0.0 }),
                bold.clone(),
                fg,
            )
            .width();
        if r.closed {
            p.text(
                egui::pos2(x0 + MX_PAD + trip_w + 6.0, ty),
                Align2::LEFT_CENTER,
                "chiuso",
                font.clone(),
                fg,
            );
        }
        let label_rect = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(MX_LABEL_W, h));
        let tip = if r.closed {
            format!("{} (chiuso)\n{}", r.tripletta, r.name)
        } else {
            format!("{}\n{}", r.tripletta, r.name)
        };
        ui.interact(
            label_rect,
            ui.id().with(("mx_row", r.proj.0)),
            Sense::hover(),
        )
        .on_hover_text(tip);

        for (i, devs) in r.cells.iter().enumerate() {
            for (l, dev) in devs.iter().enumerate() {
                p.text(
                    egui::pos2(col_x(i) + MX_COL_W / 2.0, ty + l as f32 * MX_LINE_H),
                    Align2::CENTER_CENTER,
                    fit_text(ui, dev, &font, MX_COL_W - 2.0 * MX_PAD),
                    font.clone(),
                    fg,
                );
            }
        }
        p.hline(x0..=x0 + width, y + h, border);
        vlines(y, y + h);
        y += h;
    }

    // Riepilogo, fuori dalla matrice: progetti per categoria di ogni worker.
    y += MX_SUM_GAP;
    for (ci, cat) in m.categories.iter().enumerate() {
        let lr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(MX_LABEL_W, MX_SUM_H));
        p.rect_filled(lr, 0.0, category_fill(*cat, false));
        p.text(
            egui::pos2(x0 + MX_PAD, y + MX_SUM_H / 2.0),
            Align2::LEFT_CENTER,
            fit_text(
                ui,
                &category_name(app, *cat),
                &bold,
                MX_LABEL_W - 2.0 * MX_PAD,
            ),
            bold.clone(),
            Color32::BLACK,
        );
        for (wi, cnt) in m.summary[ci].iter().enumerate() {
            if *cnt > 0 {
                p.text(
                    egui::pos2(col_x(wi) + MX_COL_W / 2.0, y + MX_SUM_H / 2.0),
                    Align2::CENTER_CENTER,
                    cnt.to_string(),
                    bold.clone(),
                    text(),
                );
            }
        }
        p.hline(x0..=x0 + width, y + MX_SUM_H, border);
        vlines(y, y + MX_SUM_H);
        y += MX_SUM_H;
    }
    // Totale: progetti (tutte le categorie) in cui è coinvolto il worker.
    let tr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, MX_SUM_H));
    p.rect_filled(tr, 0.0, strip_bg(false));
    p.text(
        egui::pos2(x0 + MX_PAD, y + MX_SUM_H / 2.0),
        Align2::LEFT_CENTER,
        "Totale progetti",
        bold.clone(),
        text(),
    );
    for (wi, cnt) in m.totals.iter().enumerate() {
        p.text(
            egui::pos2(col_x(wi) + MX_COL_W / 2.0, y + MX_SUM_H / 2.0),
            Align2::CENTER_CENTER,
            cnt.to_string(),
            bold.clone(),
            text(),
        );
    }
    p.hline(x0..=x0 + width, y + MX_SUM_H, border);
    vlines(y, y + MX_SUM_H);
}

/// Finestra della matrice progetti × worker: in alto l'intervallo (a sinistra
/// l'inizio, a destra la fine, ognuno «Oggi» o una data), poi la matrice.
pub(crate) fn matrix_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_matrix {
        return;
    }
    let today = today_days();
    let from = state.matrix_from.unwrap_or(today);
    let to = state.matrix_to.unwrap_or(today);
    let valid = from <= to;
    let m = crate::worker_matrix::build(app, &state.matrix_workers, from, to);

    let mut open = true;
    egui::Window::new("Matrice progetti × worker")
        .collapsible(true)
        .resizable(true)
        .default_size(egui::vec2(900.0, 560.0))
        .default_pos(egui::pos2(80.0, 60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            // Da/A cambiati a mano: la combo anni non descrive più l'intervallo.
            let mut manual = false;
            let mut years_changed = false;
            ui.horizontal(|ui| {
                manual |= range_end_picker(ui, "Da:", "matrix_from", &mut state.matrix_from);
                // Fine intervallo ancorata a destra: un blocco di larghezza fissa
                // (disposto da sinistra a destra) in un layout destra→sinistra.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(280.0, ui.spacing().interact_size.y),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            manual |=
                                range_end_picker(ui, "A:", "matrix_to", &mut state.matrix_to)
                        },
                    );
                    // In mezzo: la combo degli anni, centrata nello spazio rimasto.
                    let gap = ((ui.available_width() - MX_YEARS_W) / 2.0).max(0.0);
                    ui.add_space(gap);
                    years_changed = years_combo(ui, app, &mut state.matrix_years);
                });
            });
            if years_changed {
                if let Some((f, t)) = years_range(&state.matrix_years) {
                    state.matrix_from = Some(f);
                    state.matrix_to = Some(t);
                }
            } else if manual {
                state.matrix_years.clear();
            }
            ui.horizontal(|ui| {
                if ui.button("Worker…").clicked() {
                    state.matrix_select_open = true;
                }
                let can_export = valid && !m.rows.is_empty();
                if ui
                    .add_enabled(can_export, egui::Button::new("Esporta PDF…"))
                    .clicked()
                {
                    actions.push(Action::ExportWorkerMatrix { from, to });
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{} worker · {} progetti",
                        m.workers.len(),
                        m.rows.len()
                    ))
                    .color(text_dim()),
                );
            });
            ui.separator();
            if !valid {
                ui.colored_label(
                    warn_amber(),
                    "La data iniziale è successiva a quella finale.",
                );
                return;
            }
            if m.workers.is_empty() {
                ui.label("Nessun worker selezionato: usa «Worker…».");
                return;
            }
            if m.rows.is_empty() {
                ui.label("Nessun progetto con i worker selezionati nell'intervallo.");
            }
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| draw_matrix(ui, app, &m));
        });
    if !open {
        state.show_matrix = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Anni scelti → Da = 1/1 del più vecchio, A = 31/12 del più recente (anche
    /// se in mezzo c'è un anno non scelto); nessun anno → nessun intervallo.
    #[test]
    fn years_range_spans_oldest_to_newest() {
        let d = |y, m, dd| local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        assert_eq!(years_range(&BTreeSet::new()), None);
        let one: BTreeSet<i32> = [2026].into();
        assert_eq!(years_range(&one), Some((d(2026, 1, 1), d(2026, 12, 31))));
        let gap: BTreeSet<i32> = [2027, 2024].into();
        assert_eq!(years_range(&gap), Some((d(2024, 1, 1), d(2027, 12, 31))));
    }
}
