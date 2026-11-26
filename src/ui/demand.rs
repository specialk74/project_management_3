//! Finestra «Domanda e capacità» (Vista ▸ Domanda e capacità…, MIGLIORAMENTI
//! #60 fasi 1-2): righe = dev, colonne = mesi del periodo (più «Senza fine» e «In
//! ritardo» se servono, e «Totale»); ogni cella = ore **assegnate** (già in
//! griglia) + ore **da assegnare** (residuo del dev spalmato sulle settimane
//! che restano al progetto). Solo lettura; dati in `crate::demand`. Dipinta a
//! coordinate come Effort per worker.

use super::*;
use crate::demand::{DemandCell, DemandCol, DemandData};

/// Colonna dei dev, colonne dei mesi, righe.
const DM_LABEL_W: f32 = 150.0;
const DM_CELL_W: f32 = 100.0;
const DM_HEAD_H: f32 = 22.0;
const DM_ROW_H: f32 = 52.0;
/// Barretta in fondo alla cella (assegnate + da assegnare).
const DM_BAR_H: f32 = 4.0;
/// Righe al massimo nel tooltip di una cella.
const DM_TIP_ROWS: usize = 15;

/// Mesi abbreviati dell'intestazione (la GUI è in italiano).
const DM_MONTHS: [&str; 12] = ["Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic"];

/// Colore delle ore assegnate e di quelle da assegnare: più chiari nel tema
/// scuro, più scuri in quello chiaro, per leggersi sui fondi delle righe.
fn dm_assigned_color() -> Color32 {
    g(if is_dark() { Color32::from_rgb(0x8A, 0xB4, 0xFF) } else { Color32::from_rgb(0x2E, 0x66, 0xD0) })
}

fn dm_unassigned_color() -> Color32 {
    g(if is_dark() { Color32::from_rgb(0xFF, 0xA0, 0x40) } else { Color32::from_rgb(0xC0, 0x5A, 0x00) })
}

/// Fondo delle righe della tabella, alternato: vicino al fondo della finestra
/// (non quello del footer, grigio medio nel tema scuro, su cui il testo
/// grigio e i colori sparivano).
fn dm_row_bg(odd: bool) -> Color32 {
    match (is_dark(), odd) {
        (true, false) => Color32::from_rgb(0x1E, 0x1E, 0x1E),
        (true, true) => Color32::from_rgb(0x2A, 0x2A, 0x2A),
        (false, false) => Color32::WHITE,
        (false, true) => Color32::from_rgb(0xE9, 0xE9, 0xE9),
    }
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

/// Come disegnare una cella.
#[derive(Clone, Copy)]
struct CellStyle {
    /// Scala della barretta (niente barretta con `None`).
    max: Option<f64>,
    /// Totale in grassetto (colonna e riga «Totale»).
    strong: bool,
    /// Riga capacità / scarto (dev con capacità dichiarata, colonne mese e
    /// «Totale»).
    show_cap: bool,
    bg: Color32,
    /// Tooltip al passaggio del mouse (no per la cella di esempio).
    tooltip: bool,
}

/// Riga 3 della cella: capacità in grigio, scarto in verde (avanza) o rosso
/// (manca).
fn cap_job(cell: &DemandCell, font: &egui::FontId) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let fmt = |c: Color32| egui::TextFormat { font_id: font.clone(), color: c, ..Default::default() };
    job.append(&format!("cap {} · ", hours(cell.capacity)), 0.0, fmt(text_dim()));
    let gap = cell.gap().round() as i64;
    let (text, color) = if gap < 0 {
        (format!("−{}h", -gap), g(error_red()))
    } else {
        (format!("+{gap}h"), g(ok_green()))
    };
    job.append(&text, 0.0, fmt(color));
    job
}

/// Una cella: domanda totale in alto, sotto «assegnate + da assegnare» (la
/// seconda in arancione), poi capacità e scarto (se `show_cap`), in fondo la
/// barretta della domanda rispetto a `max` con il segno nero della capacità.
/// Fondo rossastro se la domanda supera la capacità. Tooltip con i progetti
/// e i worker della capacità.
fn draw_cell(ui: &egui::Ui, app: &App, rect: Rect, cell: &DemandCell, st: CellStyle, id: egui::Id) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, st.bg);
    let short = st.show_cap && cell.gap().round() < 0.0;
    if short {
        p.rect_filled(rect, 0.0, g(error_red()).gamma_multiply(0.18));
    }
    p.rect_stroke(rect, 0.0, Stroke::new(0.5, strip_border()), egui::StrokeKind::Inside);
    if cell.is_empty() {
        p.text(rect.center(), Align2::CENTER_CENTER, "—", egui::FontId::proportional(11.0), text_faint());
        return;
    }
    let cx = rect.center().x;
    let big = egui::FontId::proportional(if st.strong { 13.0 } else { 12.0 });
    let total = if cell.no_demand() { "0h".to_string() } else { hours(cell.total()) };
    p.text(egui::pos2(cx, rect.top() + 10.0), Align2::CENTER_CENTER, &total, big.clone(), text());
    if st.strong {
        p.text(egui::pos2(cx + HI_BOLD_DX, rect.top() + 10.0), Align2::CENTER_CENTER, &total, big, text());
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
    if !job.text.is_empty() {
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        p.galley(egui::pos2(cx - galley.size().x / 2.0, rect.top() + 18.0), galley, text());
    }
    if st.show_cap {
        let galley = ui.fonts_mut(|f| f.layout_job(cap_job(cell, &small)));
        p.galley(egui::pos2(cx - galley.size().x / 2.0, rect.top() + 31.0), galley, text());
    }
    if let Some(max) = st.max.filter(|m| *m > 0.0) {
        let full = rect.width() - 8.0;
        let x0 = rect.left() + 4.0;
        let y = rect.bottom() - DM_BAR_H - 3.0;
        let wa = full * (cell.assigned as f64 / max) as f32;
        let wu = full * (cell.unassigned / max) as f32;
        p.rect_filled(Rect::from_min_size(egui::pos2(x0, y), egui::vec2(wa, DM_BAR_H)), 0.0, dm_assigned_color());
        p.rect_filled(Rect::from_min_size(egui::pos2(x0 + wa, y), egui::vec2(wu, DM_BAR_H)), 0.0, dm_unassigned_color());
        if st.show_cap {
            let xc = x0 + full * (cell.capacity / max) as f32;
            p.vline(xc, (y - 3.0)..=(y + DM_BAR_H + 3.0), Stroke::new(2.0, text()));
        }
    }
    if !st.tooltip {
        return;
    }
    ui.interact(rect, id, Sense::hover()).on_hover_ui(|ui| {
        if st.show_cap {
            let gap = cell.gap().round() as i64;
            let verdict = if gap < 0 { format!("mancano {}h", -gap) } else { format!("avanzano {gap}h") };
            ui.label(egui::RichText::new(format!(
                "Domanda {} · capacità {}: {verdict}",
                hours(cell.total()),
                hours(cell.capacity)
            ))
            .strong());
            let workers: Vec<String> = cell
                .cap_workers_by_weight()
                .iter()
                .map(|(w, h)| format!("{} {}", app.workers.get_name_by_id(*w), hours(*h)))
                .collect();
            if !workers.is_empty() {
                ui.label(egui::RichText::new(format!("Capacità da: {}", workers.join(", "))).color(text_dim()));
            }
            ui.separator();
        }
        let list = cell.projects_by_weight();
        if list.is_empty() {
            ui.label(egui::RichText::new("Nessun progetto.").color(text_dim()));
        }
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
        .chain(std::iter::once((
            "Totale".to_string(),
            Some("Somma delle colonne della riga: la capacità è quella dei mesi, la domanda comprende anche «Senza fine» e «In ritardo»."),
        )))
        .collect();
    for (i, (label, hint)) in labels.iter().enumerate() {
        let r = Rect::from_min_size(egui::pos2(x_of(i), rect.top()), egui::vec2(DM_CELL_W, DM_HEAD_H));
        p.text(r.center(), Align2::CENTER_CENTER, label, head_font.clone(), text());
        if let Some(h) = hint {
            ui.interact(r, ui.scope_id().with(("dm_head", i)), Sense::hover()).on_hover_text(*h);
        }
    }
    // Scala della barretta: la cella di dev più grande, domanda o capacità.
    let max = data
        .rows
        .iter()
        .flat_map(|r| r.cells.iter().map(|c| c.total().max(c.capacity)))
        .fold(0.0, f64::max);
    let is_month = |ci: usize| matches!(data.cols.get(ci), Some(DemandCol::Month { .. }) | None);
    let body_font = egui::FontId::proportional(12.0);
    for (ri, row) in data.rows.iter().enumerate() {
        let y = y_of(ri);
        let bg = dm_row_bg(ri % 2 == 1);
        let label = Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(DM_LABEL_W, DM_ROW_H));
        p.rect_filled(label.shrink(2.0), 3.0, dev_color(app, row.dev));
        let name = fit_text(ui, &dev_name(app, row.dev), &body_font, DM_LABEL_W - 14.0);
        p.text(egui::pos2(label.left() + 8.0, label.center().y - if row.declared { 0.0 } else { 6.0 }), Align2::LEFT_CENTER, name, body_font.clone(), dev_text_color(app, row.dev));
        if !row.declared {
            p.text(
                egui::pos2(label.left() + 8.0, label.center().y + 8.0),
                Align2::LEFT_CENTER,
                "capacità non dichiarata",
                egui::FontId::proportional(9.5),
                dev_text_color(app, row.dev),
            );
        }
        for (ci, cell) in row.cells.iter().enumerate() {
            let r = Rect::from_min_size(egui::pos2(x_of(ci), y), egui::vec2(DM_CELL_W, DM_ROW_H));
            let st = CellStyle { max: Some(max), strong: false, show_cap: row.declared && is_month(ci), bg, tooltip: true };
            draw_cell(ui, app, r, cell, st, ui.scope_id().with(("dm", ri, ci)));
        }
        let r = Rect::from_min_size(egui::pos2(x_of(ncol - 1), y), egui::vec2(DM_CELL_W, DM_ROW_H));
        let st = CellStyle { max: None, strong: true, show_cap: row.declared, bg, tooltip: true };
        draw_cell(ui, app, r, &row.sum(), st, ui.scope_id().with(("dm_sum", ri)));
    }
    // Riga «Totale», separata da una linea spessa.
    let y = y_of(data.rows.len()) + 2.0;
    p.hline(rect.left()..=rect.right(), y - 1.0, Stroke::new(2.0, text()));
    p.text(egui::pos2(rect.left() + 8.0, y + DM_ROW_H / 2.0), Align2::LEFT_CENTER, "Totale", body_font, text());
    let declared = data.any_declared();
    let tot_bg = strip_bg(false);
    for (ci, cell) in data.total.iter().enumerate() {
        let r = Rect::from_min_size(egui::pos2(x_of(ci), y), egui::vec2(DM_CELL_W, DM_ROW_H));
        let st = CellStyle { max: None, strong: true, show_cap: declared && is_month(ci), bg: tot_bg, tooltip: true };
        draw_cell(ui, app, r, cell, st, ui.scope_id().with(("dm_tot", ci)));
    }
    let r = Rect::from_min_size(egui::pos2(x_of(ncol - 1), y), egui::vec2(DM_CELL_W, DM_ROW_H));
    let st = CellStyle { max: None, strong: true, show_cap: declared, bg: tot_bg, tooltip: true };
    draw_cell(ui, app, r, &data.grand(), st, ui.scope_id().with("dm_grand"));
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
        // Il segno nero della capacità, come sulla barretta.
        let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 14.0), Sense::hover());
        ui.painter().vline(r.center().x, r.y_range(), Stroke::new(2.0, text()));
        ui.label(egui::RichText::new("Capacità").strong()).on_hover_text(
            "Ore massime effettive (limiti di settimana compresi) dei worker che hanno una quota su \
             quel dev, per la quota. Ghost e worker archiviati non contano.",
        );
        ui.add_space(12.0);
        ui.label(egui::RichText::new("+avanza").color(g(ok_green())).strong());
        ui.label(egui::RichText::new("−manca").color(g(error_red())).strong());
    });
}

/// Spiegazioni della cella di esempio, una per elemento, con l'altezza
/// (dall'alto della cella) dell'elemento a cui si riferiscono.
const DM_KEY_NOTES: [(f32, &str); 4] = [
    (10.0, "160h — domanda del mese: tutte le ore di lavoro per quel dev"),
    (24.0, "120 + 40 — in blu le ore già assegnate in griglia, in arancione quelle ancora da assegnare"),
    (37.0, "cap 140h · −20h — capacità (worker con una quota sul dev) e scarto: + verde avanzano ore, − rosso mancano"),
    (47.0, "barretta — la domanda (blu + arancione); il segno nero è la capacità: se la barretta lo supera, mancano ore"),
];

/// Nota sotto la cella di esempio, sul fondo.
const DM_KEY_BG_NOTE: &str = "Fondo rossastro — in quel mese la domanda supera la capacità. Un dev con \
     «capacità non dichiarata» non ha né la riga «cap» né il fondo rosso.";

/// «Come si legge una cella»: una cella di esempio (domanda 160h = 120
/// assegnate + 40 da assegnare, capacità 140h: mancano 20h, fondo rossastro)
/// disegnata come quelle della tabella, con accanto la spiegazione di ogni
/// elemento collegata da una linea. Solo nella finestra, non nel PDF.
fn demand_cell_key(ui: &mut egui::Ui, app: &App) {
    let sample = DemandCell { assigned: 120, unassigned: 40.0, capacity: 140.0, ..Default::default() };
    let font = egui::FontId::proportional(12.0);
    let text_x = DM_CELL_W + 40.0;
    let notes_w = DM_KEY_NOTES
        .iter()
        .map(|(_, t)| ui.fonts_mut(|f| f.layout_no_wrap(t.to_string(), font.clone(), text()).size().x))
        .fold(0.0, f32::max);
    let size = egui::vec2(text_x + notes_w + 8.0, DM_ROW_H + 22.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let cell = Rect::from_min_size(rect.min, egui::vec2(DM_CELL_W, DM_ROW_H));
    let st = CellStyle { max: Some(200.0), strong: false, show_cap: true, bg: dm_row_bg(false), tooltip: false };
    draw_cell(ui, app, cell, &sample, st, ui.scope_id().with("dm_key"));
    let p = ui.painter();
    let link = Stroke::new(1.0, text_dim());
    for (dy, note) in DM_KEY_NOTES {
        let y = cell.top() + dy;
        p.hline((cell.right() + 4.0)..=(rect.left() + text_x - 6.0), y, link);
        p.circle_filled(egui::pos2(cell.right() + 4.0, y), 2.0, text_dim());
        p.text(egui::pos2(rect.left() + text_x, y), Align2::LEFT_CENTER, note, font.clone(), text());
    }
    p.text(
        egui::pos2(cell.left(), cell.bottom() + 11.0),
        Align2::LEFT_CENTER,
        DM_KEY_BG_NOTE,
        font,
        text(),
    );
}

pub(crate) fn demand_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    if !state.show_demand {
        return;
    }
    let open = os_window(ctx, "demand_window", "Domanda e capacità", egui::vec2(1200.0, 620.0), |ui| {
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
        ui.horizontal(|ui| {
            if ui
                .add_enabled(any, egui::Button::new("Esporta PDF…"))
                .on_hover_text("La tabella in un PDF (in inglese), con la legenda e i filtri.")
                .clicked()
            {
                actions.push(Action::ExportDemand { from, to });
            }
            if ui
                .add_enabled(any, egui::Button::new("Esporta CSV…"))
                .on_hover_text(
                    "Due file: la domanda con una riga per dev, mese e progetto (…_demand.csv) e la \
                     capacità con una riga per dev e mese (…_capacity.csv).",
                )
                .clicked()
            {
                actions.push(Action::ExportDemandCsv { from, to });
            }
            if ui
                .button("Quote dei worker…")
                .on_hover_text("Apre Aggiungi ▸ Worker, dev e categorie… sulla scheda Worker: la colonna «Dev» dice quali dev sa fare ognuno e con che quota.")
                .clicked()
            {
                state.registry_tab = RegistryTab::Workers;
                state.show_registry = true;
            }
        });
        ui.label(
            egui::RichText::new(
                "Domanda: ore per dev e per mese, su tutti i progetti dei filtri (non solo quelli della \
                 vista). In blu le ore già assegnate in griglia (nel mese del lunedì della settimana); \
                 in arancione quelle da assegnare: pianificato del dev meno le ore in griglia, \
                 spalmato in parti uguali da oggi (o dall'inizio del progetto) alla sua fine. «Senza \
                 fine» e «In ritardo» raccolgono il da assegnare dei progetti senza data di fine o con \
                 la fine passata. Capacità: dalle quote per dev dei worker (pulsante «Quote dei \
                 worker…»); non dipende dai filtri dei progetti. Passando su una cella, i progetti e i \
                 worker che la compongono.",
            )
            .color(text_dim()),
        );
        demand_legend(ui);
        egui::CollapsingHeader::new("Come si legge una cella")
            .id_salt("demand_cell_key")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("demand_cell_key_scroll")
                    .scroll_source(drag_scroll())
                    .show(ui, |ui| demand_cell_key(ui, app));
            });
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

    /// Rapporto di contrasto WCAG fra due colori (1 … 21).
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let lin = |v: u8| {
                let c = v as f32 / 255.0;
                if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
        };
        let (la, lb) = (lum(a), lum(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Su tutti e due i fondi delle righe, in tutti e due i temi, si leggono
    /// il testo, il testo grigio, il blu, l'arancione e lo scarto.
    #[test]
    fn row_backgrounds_keep_every_text_readable() {
        for dark in [true, false] {
            set_dark_theme(dark);
            for odd in [false, true] {
                let bg = dm_row_bg(odd);
                for (what, c, min) in [
                    ("testo", text(), 7.0),
                    ("grigio", text_dim(), 4.0),
                    ("blu", dm_assigned_color(), 3.5),
                    ("arancione", dm_unassigned_color(), 3.5),
                    ("rosso", g(error_red()), 3.0),
                    ("verde", g(ok_green()), 3.0),
                ] {
                    let k = contrast(c, bg);
                    assert!(k >= min, "{what} su {bg:?} (scuro {dark}): {k:.1} < {min}");
                }
            }
        }
        set_dark_theme(false);
    }

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
        // Cella di esempio commentata (solo finestra): la cella e una spiegazione
        // per elemento, più la nota sul fondo.
        for t in ["Come si legge una cella", "160h", "120 + 40", "cap 140h · −20h", DM_KEY_BG_NOTE] {
            assert!(texts.iter().any(|x| x == t), "{t}: {texts:?}");
        }
        for (_, note) in DM_KEY_NOTES {
            assert!(texts.iter().any(|x| x == note), "{note}: {texts:?}");
        }
        for t in ["Assegnate", "Da assegnare", "Capacità", "Esporta PDF…", "Quote dei worker…", "Dev", "capacità non dichiarata"] {
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
        // Un dev nascosto ma ancora usato resta nella combo Dev (segnato), così
        // si può togliere; nascosto e mai usato no.
        state.demand_devs.clear();
        app.devs.set_hidden(be, true);
        let unused = app.devs.add("Mai usato");
        app.devs.set_hidden(unused, true);
        let pos = {
            let out = ctx.run_ui_test(input.clone(), |ui| devs_combo(ui, &app, "t_devs", &mut state.demand_devs));
            text_pos(&out, "Dev: tutti").expect("combo")
        };
        let mut texts = Vec::new();
        for ev in click_events(pos, egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| devs_combo(ui, &app, "t_devs", &mut state.demand_devs));
        }
        for _ in 0..2 {
            let out = ctx.run_ui_test(input.clone(), |ui| devs_combo(ui, &app, "t_devs", &mut state.demand_devs));
            texts = drawn_texts(&out);
        }
        assert!(texts.iter().any(|x| x == "Backend (nascosto)"), "{texts:?}");
        assert!(!texts.iter().any(|x| x.starts_with("Mai usato")), "{texts:?}");
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
        // «Esporta CSV…» manda l'export dei due file.
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| demand_window(ui.ctx(), &app, &mut state, &mut actions));
        let pos = text_pos(&out, "Esporta CSV…").expect("pulsante CSV");
        for ev in click_events(pos, egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| demand_window(ui.ctx(), &app, &mut state, &mut actions));
        }
        assert!(matches!(actions.as_slice(), [Action::ExportDemandCsv { from: f, to: t }] if *f == from && *t == to));
        // Quote dichiarate: «cap … · scarto» e il pulsante apre il registro dei worker.
        app.workers.set_dev_shares(alice, &[(be, 50)]);
        for _ in 0..2 {
            let out = ctx.run_ui_test(input.clone(), |ui| demand_window(ui.ctx(), &app, &mut state, &mut Vec::new()));
            texts = drawn_texts(&out);
        }
        assert!(texts.iter().any(|x| x == "cap 20h · −5h"), "{texts:?}");
        let out = ctx.run_ui_test(input.clone(), |ui| demand_window(ui.ctx(), &app, &mut state, &mut Vec::new()));
        let pos = text_pos(&out, "Quote dei worker…").unwrap();
        for ev in click_events(pos, egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| demand_window(ui.ctx(), &app, &mut state, &mut Vec::new()));
        }
        assert!(state.show_registry && state.registry_tab == RegistryTab::Workers);
    }
}
