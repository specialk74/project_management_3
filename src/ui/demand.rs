//! Finestra «Domanda per dev» (Vista ▸ Domanda per dev…, MIGLIORAMENTI #60
//! fase 1): righe = dev, colonne = mesi del periodo (più «Senza fine» e «In
//! ritardo» se servono, e «Totale»); ogni cella = ore **assegnate** (già in
//! griglia) + ore **da assegnare** (residuo del dev spalmato sulle settimane
//! che restano al progetto). Solo lettura; dati in `crate::demand`. Dipinta a
//! coordinate come Effort per worker.

use super::*;
use crate::demand::{DemandCell, DemandCol, DemandData};

/// Colonna dei dev, colonne dei mesi, righe.
const DM_LABEL_W: f32 = 150.0;
const DM_CELL_W: f32 = 84.0;
const DM_HEAD_H: f32 = 22.0;
const DM_ROW_H: f32 = 36.0;
/// Barretta in fondo alla cella (assegnate + da assegnare).
const DM_BAR_H: f32 = 4.0;
/// Righe al massimo nel tooltip di una cella.
const DM_TIP_ROWS: usize = 15;

/// Mesi abbreviati dell'intestazione (la GUI è in italiano).
const DM_MONTHS: [&str; 12] = ["Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic"];

/// Colore delle ore assegnate e di quelle da assegnare.
fn dm_assigned_color() -> Color32 {
    g(Color32::from_rgb(0x5B, 0x8D, 0xEF))
}

fn dm_unassigned_color() -> Color32 {
    g(Color32::from_rgb(0xFF, 0x8C, 0x1A))
}

/// Etichetta di una colonna.
fn col_label(c: DemandCol) -> String {
    match c {
        DemandCol::Month { year, month } => format!("{} {:02}", DM_MONTHS[month as usize - 1], year % 100),
        DemandCol::NoEnd => "Senza fine".into(),
        DemandCol::Late => "In ritardo".into(),
    }
}

fn col_hint(c: DemandCol) -> Option<&'static str> {
    match c {
        DemandCol::Month { .. } => None,
        DemandCol::NoEnd => Some("Ore da assegnare dei progetti senza data di fine: non si sa quando cadono."),
        DemandCol::Late => Some("Ore da assegnare dei progetti con la fine già passata."),
    }
}

/// Ore arrotondate all'intero, con «h».
fn hours(h: f64) -> String {
    format!("{}h", h.round() as i64)
}

/// Dati della finestra con i suoi filtri.
pub(crate) fn demand_view_data(app: &App, state: &UiState, from: i32, to: i32) -> DemandData {
    crate::demand::build(
        app,
        from,
        to,
        current_week_id(),
        &state.demand_categories,
        &state.demand_statuses,
        &state.demand_priorities,
        &state.demand_devs,
    )
}

/// Una cella: totale in alto, sotto «assegnate + da assegnare» (la seconda
/// in arancione), in fondo una barretta lunga quanto il totale rispetto a
/// `max` (assenza di `max` = niente barretta). Tooltip con i progetti.
#[allow(clippy::too_many_arguments)]
fn draw_cell(ui: &egui::Ui, app: &App, rect: Rect, cell: &DemandCell, max: Option<f64>, strong: bool, id: egui::Id, bg: Color32) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, bg);
    p.rect_stroke(rect, 0.0, Stroke::new(0.5, strip_border()), egui::StrokeKind::Inside);
    if cell.is_empty() {
        p.text(rect.center(), Align2::CENTER_CENTER, "—", egui::FontId::proportional(11.0), text_faint());
        return;
    }
    let big = egui::FontId::proportional(if strong { 13.0 } else { 12.0 });
    p.text(egui::pos2(rect.center().x, rect.top() + 10.0), Align2::CENTER_CENTER, hours(cell.total()), big.clone(), text());
    if strong {
        p.text(egui::pos2(rect.center().x + HI_BOLD_DX, rect.top() + 10.0), Align2::CENTER_CENTER, hours(cell.total()), big, text());
    }
    // «assegnate + da assegnare», solo le parti che ci sono.
    let small = egui::FontId::proportional(10.0);
    let mut job = egui::text::LayoutJob::default();
    let fmt = |c: Color32| egui::TextFormat { font_id: small.clone(), color: c, ..Default::default() };
    if cell.assigned > 0 {
        job.append(&cell.assigned.to_string(), 0.0, fmt(dm_assigned_color()));
    }
    if cell.unassigned.round() > 0.0 {
        if cell.assigned > 0 {
            job.append(" + ", 0.0, fmt(text_dim()));
        }
        job.append(&format!("{}", cell.unassigned.round() as i64), 0.0, fmt(dm_unassigned_color()));
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let pos = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 18.0);
    p.galley(pos, galley, text());
    if let Some(max) = max.filter(|m| *m > 0.0) {
        let full = rect.width() - 8.0;
        let x0 = rect.left() + 4.0;
        let y = rect.bottom() - DM_BAR_H - 3.0;
        let wa = full * (cell.assigned as f64 / max) as f32;
        let wu = full * (cell.unassigned / max) as f32;
        p.rect_filled(Rect::from_min_size(egui::pos2(x0, y), egui::vec2(wa, DM_BAR_H)), 0.0, dm_assigned_color());
        p.rect_filled(Rect::from_min_size(egui::pos2(x0 + wa, y), egui::vec2(wu, DM_BAR_H)), 0.0, dm_unassigned_color());
    }
    ui.interact(rect, id, Sense::hover()).on_hover_ui(|ui| {
        let list = cell.projects_by_weight();
        for pd in list.iter().take(DM_TIP_ROWS) {
            let mut parts = Vec::new();
            if pd.assigned > 0 {
                parts.push(format!("{}h assegnate", pd.assigned));
            }
            if pd.unassigned.round() > 0.0 {
                parts.push(format!("{} da assegnare", hours(pd.unassigned)));
            }
            ui.label(format!("{}: {}", project_label(app, pd.proj), parts.join(" · ")));
        }
        if list.len() > DM_TIP_ROWS {
            ui.label(egui::RichText::new(format!("… e altri {}", list.len() - DM_TIP_ROWS)).color(text_dim()));
        }
    });
}

/// La tabella, dipinta a coordinate: intestazione, una riga per dev, riga
/// «Totale»; ultima colonna «Totale».
fn draw_demand_table(ui: &mut egui::Ui, app: &App, data: &DemandData) {
    let ncol = data.cols.len() + 1;
    let nrow = data.rows.len() + 1;
    let size = egui::vec2(DM_LABEL_W + ncol as f32 * DM_CELL_W, DM_HEAD_H + nrow as f32 * DM_ROW_H + 2.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    let x_of = |i: usize| rect.left() + DM_LABEL_W + i as f32 * DM_CELL_W;
    let y_of = |r: usize| rect.top() + DM_HEAD_H + r as f32 * DM_ROW_H;
    let head_font = egui::FontId::proportional(11.0);
    // Intestazione.
    let head = Rect::from_min_size(rect.min, egui::vec2(size.x, DM_HEAD_H));
    p.rect_filled(head, 0.0, strip_bg(false));
    p.text(egui::pos2(rect.left() + 6.0, head.center().y), Align2::LEFT_CENTER, "Dev", head_font.clone(), text_dim());
    let labels: Vec<(String, Option<&str>)> = data
        .cols
        .iter()
        .map(|c| (col_label(*c), col_hint(*c)))
        .chain(std::iter::once(("Totale".to_string(), Some("Somma delle colonne della riga."))))
        .collect();
    for (i, (label, hint)) in labels.iter().enumerate() {
        let r = Rect::from_min_size(egui::pos2(x_of(i), rect.top()), egui::vec2(DM_CELL_W, DM_HEAD_H));
        p.text(r.center(), Align2::CENTER_CENTER, label, head_font.clone(), text());
        if let Some(h) = hint {
            ui.interact(r, ui.scope_id().with(("dm_head", i)), Sense::hover()).on_hover_text(*h);
        }
    }
    // Scala della barretta: la cella di dev più grande (mesi e colonne speciali).
    let max = data.rows.iter().flat_map(|r| r.cells.iter().map(|c| c.total())).fold(0.0, f64::max);
    let body_font = egui::FontId::proportional(12.0);
    for (ri, row) in data.rows.iter().enumerate() {
        let y = y_of(ri);
        let bg = if ri % 2 == 0 { row_even() } else { row_alt() };
        let label = Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(DM_LABEL_W, DM_ROW_H));
        p.rect_filled(label.shrink(2.0), 3.0, dev_color(app, row.dev));
        let name = fit_text(ui, &dev_name(app, row.dev), &body_font, DM_LABEL_W - 14.0);
        p.text(egui::pos2(label.left() + 8.0, label.center().y), Align2::LEFT_CENTER, name, body_font.clone(), dev_text_color(app, row.dev));
        for (ci, cell) in row.cells.iter().enumerate() {
            let r = Rect::from_min_size(egui::pos2(x_of(ci), y), egui::vec2(DM_CELL_W, DM_ROW_H));
            draw_cell(ui, app, r, cell, Some(max), false, ui.scope_id().with(("dm", ri, ci)), bg);
        }
        let r = Rect::from_min_size(egui::pos2(x_of(ncol - 1), y), egui::vec2(DM_CELL_W, DM_ROW_H));
        draw_cell(ui, app, r, &row.sum(), None, true, ui.scope_id().with(("dm_sum", ri)), bg);
    }
    // Riga «Totale», separata da una linea spessa.
    let y = y_of(data.rows.len()) + 2.0;
    p.hline(rect.left()..=rect.right(), y - 1.0, Stroke::new(2.0, text()));
    p.text(egui::pos2(rect.left() + 8.0, y + DM_ROW_H / 2.0), Align2::LEFT_CENTER, "Totale", body_font, text());
    let mut grand = DemandCell::default();
    for (ci, cell) in data.total.iter().enumerate() {
        let r = Rect::from_min_size(egui::pos2(x_of(ci), y), egui::vec2(DM_CELL_W, DM_ROW_H));
        draw_cell(ui, app, r, cell, None, true, ui.scope_id().with(("dm_tot", ci)), strip_bg(false));
        grand.assigned += cell.assigned;
        grand.unassigned += cell.unassigned;
        for pd in &cell.projects {
            match grand.projects.iter_mut().find(|x| x.proj == pd.proj) {
                Some(x) => {
                    x.assigned += pd.assigned;
                    x.unassigned += pd.unassigned;
                }
                None => grand.projects.push(pd.clone()),
            }
        }
    }
    let r = Rect::from_min_size(egui::pos2(x_of(ncol - 1), y), egui::vec2(DM_CELL_W, DM_ROW_H));
    draw_cell(ui, app, r, &grand, None, true, ui.scope_id().with("dm_grand"), strip_bg(false));
}

/// Legenda dei due colori della barretta (e dei numeri sotto il totale).
fn demand_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (color, text, hint) in [
            (dm_assigned_color(), "Assegnate", "Ore già in griglia per quel dev, su tutti i progetti."),
            (
                dm_unassigned_color(),
                "Da assegnare",
                "Pianificato del dev meno le ore già in griglia, spalmato in parti uguali da oggi (o \
                 dall'inizio del progetto) alla fine del progetto.",
            ),
        ] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 10.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, color);
            ui.label(egui::RichText::new(text).strong()).on_hover_text(hint);
            ui.add_space(12.0);
        }
        ui.label(
            egui::RichText::new("Ogni cella: totale, sotto «assegnate + da assegnare», in fondo la barretta.")
                .color(text_dim()),
        );
    });
}

pub(crate) fn demand_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    if !state.show_demand {
        return;
    }
    let open = os_window(ctx, "demand_window", "Domanda per dev", egui::vec2(1100.0, 560.0), |ui| {
        let (mut from, mut to) = state.demand_range.unwrap_or_else(default_range);
        let mut manual = false;
        let mut changed = false;
        ui.horizontal(|ui| {
            let picker = |ui: &mut egui::Ui, label: &str, salt: &str, day: &mut i32| -> bool {
                ui.label(egui::RichText::new(label).strong());
                let mut date = days_to_jiff(*day);
                let r = ui.add(egui_extras::DatePickerButton::new(&mut date).id_salt(salt).format("%Y-%m-%d"));
                if r.changed()
                    && let Some(d) = jiff_to_days(date)
                {
                    *day = d;
                    return true;
                }
                false
            };
            manual |= picker(ui, "Da:", "demand_from", &mut from);
            ui.separator();
            let years_changed = years_combo(ui, app, "demand_years", &mut state.demand_years);
            ui.separator();
            manual |= picker(ui, "A:", "demand_to", &mut to);
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
                if let Some(range) = years_range(&state.demand_years) {
                    (from, to) = range;
                    changed = true;
                }
            } else if manual {
                state.demand_years.clear();
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            categories_combo(ui, app, "demand_categories", &mut state.demand_categories);
            ui.separator();
            statuses_combo(ui, "demand_statuses", &mut state.demand_statuses);
            ui.separator();
            priorities_combo(ui, app, "demand_priorities", &mut state.demand_priorities);
            ui.separator();
            devs_combo(ui, app, "demand_devs", &mut state.demand_devs);
        });
        if changed {
            state.demand_range = Some((from, to));
        }
        let data = (from <= to).then(|| demand_view_data(app, state, from, to));
        let any = data.as_ref().is_some_and(|d| !d.rows.is_empty());
        if ui
            .add_enabled(any, egui::Button::new("Esporta PDF…"))
            .on_hover_text("La tabella in un PDF (in inglese), con la legenda e i filtri.")
            .clicked()
        {
            actions.push(Action::ExportDemand { from, to });
        }
        ui.label(
            egui::RichText::new(
                "Ore per dev e per mese, su tutti i progetti dei filtri (non solo quelli della vista). \
                 In blu le ore già assegnate in griglia (nel mese del lunedì della settimana); in \
                 arancione quelle da assegnare: pianificato del dev meno le ore in griglia, spalmato \
                 in parti uguali da oggi (o dall'inizio del progetto) alla sua fine. «Senza fine» e \
                 «In ritardo» raccolgono il da assegnare dei progetti senza data di fine o con la \
                 fine passata. Passando su una cella, i progetti che la compongono.",
            )
            .color(text_dim()),
        );
        demand_legend(ui);
        ui.separator();
        let Some(data) = data else {
            ui.colored_label(warn_amber(), "La data iniziale è successiva a quella finale.");
            return;
        };
        if data.rows.is_empty() {
            ui.label("Nessuna domanda nel periodo.");
            return;
        }
        egui::ScrollArea::both().scroll_source(drag_scroll()).auto_shrink([false, false]).show(ui, |ui| {
            draw_demand_table(ui, app, &data);
        });
    });
    if !open {
        state.show_demand = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La finestra mostra una riga per dev con domanda (nome nel colore del
    /// dev), le colonne dei mesi e quelle speciali che servono, in ogni cella
    /// il totale e «assegnate + da assegnare», e la riga / colonna Totale.
    #[test]
    fn demand_window_shows_assigned_and_unassigned_per_dev_and_month() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let now = current_week_id();
        let a = app.projects.add("A", Some("AAA"), None);
        app.projects.set_project_end_week(a, Some(now));
        app.projects.add_dev(a, fe);
        app.projects.add_dev_effort(a, fe, Effort(30));
        app.projects.add_effort(a, fe, now, alice, Effort(10));
        let b = app.projects.add("B", Some("BBB"), None);
        app.projects.add_dev(b, be);
        app.projects.add_dev_effort(b, be, Effort(25));
        let (from, to) = (now.0 as i32, now.0 as i32 + 6);
        let mut state = UiState { show_demand: true, demand_range: Some((from, to)), ..Default::default() };
        let ctx = egui::Context::default();
        let input = egui::RawInput { screen_rect: Some(screen(1200.0, 800.0)), ..Default::default() };
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui_test(input.clone(), |ui| demand_window(ui.ctx(), &app, &mut state, &mut Vec::new()));
            texts = drawn_texts(&out);
        }
        let month = col_label(DemandCol::Month {
            year: crate::demand::month_of_day(from).0,
            month: crate::demand::month_of_day(from).1,
        });
        // Frontend: 10h assegnate + 20h da assegnare nella settimana corrente
        // (fine del progetto); Backend: 25h senza fine.
        for t in [
            "Frontend", "Backend", month.as_str(), "Senza fine", "Totale", "30h", "10 + 20", "25h", "55h",
        ] {
            assert!(texts.iter().any(|x| x == t), "{t}: {texts:?}");
        }
        assert!(!texts.iter().any(|x| x == "In ritardo"), "colonna non usata: {texts:?}");
        for t in ["Assegnate", "Da assegnare", "Esporta PDF…", "Dev"] {
            assert!(texts.iter().any(|x| x == t), "{t}: {texts:?}");
        }
        assert!(state.show_demand);
        // Filtro dev: solo il Backend.
        state.demand_devs = [be].into_iter().collect();
        for _ in 0..2 {
            let out = ctx.run_ui_test(input.clone(), |ui| demand_window(ui.ctx(), &app, &mut state, &mut Vec::new()));
            texts = drawn_texts(&out);
        }
        assert!(texts.iter().any(|x| x == "Backend") && !texts.iter().any(|x| x == "Frontend"), "{texts:?}");
        // «Esporta PDF…» manda l'export del periodo.
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| demand_window(ui.ctx(), &app, &mut state, &mut actions));
        let pos = text_pos(&out, "Esporta PDF…").expect("pulsante");
        for ev in click_events(pos, egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| demand_window(ui.ctx(), &app, &mut state, &mut actions));
        }
        assert!(matches!(actions.as_slice(), [Action::ExportDemand { from: f, to: t }] if *f == from && *t == to));
    }
}
