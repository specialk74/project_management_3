//! Torte dev/categorie per worker (Vista ▸ Torte dev/categorie per worker…):
//! come la matrice, prima la dialog dei worker (selezione propria, solo
//! sessione), poi la finestra con l'intervallo di date e, per ogni worker, due
//! torte con la tabella dei valori accanto. Dati e geometria in
//! `crate::worker_pies`, condivisi con l'export PDF/SVG.

use super::*;
use crate::worker_matrix::category_color;
use crate::worker_pies::{Hours, Seg, WorkerPie, label_pos, pct_label};

/// Diametro dell'area di una torta (px).
const PIE_D: f32 = 340.0;
/// Raggio esterno delle torte.
const PIE_R: f32 = 162.0;
/// Raggio dell'anello interno (dev) della prima torta; l'anello esterno
/// (categorie) parte da qui + `PIE_GAP`.
const PIE_R_IN: f32 = 100.0;
const PIE_GAP: f32 = 4.0;
/// Spazio fra il primo blocco (torta con la sua tabella) e il secondo.
const PIE_SPACING: f32 = 72.0;
/// Larghezza massima della colonna «Categoria» delle tabelle (quadratino
/// compreso): i nomi più lunghi sono troncati con «…», intero nel tooltip.
const CAT_W: f32 = 110.0;

pub(crate) fn pies_select_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.pies_select_open {
        return;
    }
    match worker_pick_window(
        ctx,
        app,
        "Torte dev/categorie — scegli i worker",
        "Mostra torte",
        &mut state.pies_workers,
        &mut state.pies_worker_search,
    ) {
        WorkerPick::Open => {}
        WorkerPick::Show => {
            state.show_pies = true;
            state.pies_select_open = false;
        }
        WorkerPick::Closed => state.pies_select_open = false,
    }
}

fn category_rgb(cat: Option<CategoryId>) -> Color32 {
    rgb_color(category_color(cat))
}

/// Riempie la fetta fra i raggi `r0` e `r1` con una mesh di triangoli (la
/// fetta non è convessa, quindi non può essere un poligono pieno di egui).
#[allow(clippy::too_many_arguments)]
fn paint_sector(p: &egui::Painter, c: egui::Pos2, r0: f32, r1: f32, from: f32, to: f32, color: Color32) {
    use crate::worker_pies::arc;
    let outer = arc(c.x, c.y, r1, from, to, false);
    let inner = arc(c.x, c.y, r0.max(0.0), from, to, false);
    let mut mesh = egui::Mesh::default();
    for (o, i) in outer.iter().zip(&inner) {
        mesh.colored_vertex(egui::pos2(o.0, o.1), color);
        mesh.colored_vertex(egui::pos2(i.0, i.1), color);
    }
    for k in 0..outer.len().saturating_sub(1) as u32 {
        let b = 2 * k;
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b + 1, b + 3, b + 2);
    }
    p.add(egui::Shape::mesh(mesh));
}

/// Un anello: fette colorate, bordo chiaro fra le fette, percentuale (sul
/// totale del worker) dentro le fette in cui ci sta, velo su quella puntata.
#[allow(clippy::too_many_arguments)]
fn paint_ring<K>(
    ui: &egui::Ui,
    c: egui::Pos2,
    r0: f32,
    r1: f32,
    segs: &[Seg<K>],
    total: u32,
    color: impl Fn(&K) -> Color32,
    hovered: Option<usize>,
) {
    let p = ui.painter();
    for (i, s) in segs.iter().enumerate() {
        paint_sector(p, c, r0, r1, s.from, s.to, color(&s.key));
        if hovered == Some(i) {
            paint_sector(p, c, r0, r1, s.from, s.to, Color32::from_white_alpha(90));
        }
    }
    if segs.len() > 1 {
        for s in segs {
            let a = crate::worker_pies::arc_point(c.x, c.y, r0, s.from, false);
            let b = crate::worker_pies::arc_point(c.x, c.y, r1, s.from, false);
            p.line_segment(
                [egui::pos2(a.0, a.1), egui::pos2(b.0, b.1)],
                Stroke::new(1.5_f32, Color32::WHITE),
            );
        }
    }
    let font = egui::FontId::proportional(13.0);
    for s in segs {
        let label = pct_label(s.hours, total);
        let fill = color(&s.key);
        let galley = p.layout_no_wrap(label, font.clone(), contrast_text(fill));
        let size = galley.size();
        if let Some((x, y)) = label_pos(c.x, c.y, r0, r1, s.from, s.to, size.x, size.y, false) {
            let at = egui::pos2(x, y) - size / 2.0;
            p.galley(at, galley, contrast_text(fill));
        }
    }
}

/// Testo del tooltip di una fetta: `nome: 30h (25%)`.
fn seg_tip(name: &str, hours: u32, total: u32) -> String {
    format!("{name}: {hours}h ({})", pct_label(hours, total))
}

/// Prima torta: anello interno per dev, esterno per categoria dentro ogni dev.
/// `width` = larghezza del blocco: la torta sta al centro.
fn dev_category_pie(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, width: f32) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, PIE_D), Sense::hover());
    let c = rect.center();
    let inner = pie.dev_ring();
    let outer = pie.dev_category_ring();
    let pos = resp.hover_pos();
    let hit_in = pos.and_then(|q| crate::worker_pies::hit(c.x, c.y, 0.0, PIE_R_IN, &inner, q.x, q.y));
    let hit_out = pos.and_then(|q| {
        crate::worker_pies::hit(c.x, c.y, PIE_R_IN + PIE_GAP, PIE_R, &outer, q.x, q.y)
    });
    let dev_name = |d: DevId| app.devs.get(d).map(|d| d.get_name()).unwrap_or_default();
    paint_ring(ui, c, 0.0, PIE_R_IN, &inner, pie.total, |d| dev_color(app, *d), hit_in);
    paint_ring(
        ui,
        c,
        PIE_R_IN + PIE_GAP,
        PIE_R,
        &outer,
        pie.total,
        |(_, cat)| category_rgb(*cat),
        hit_out,
    );
    let tip = if let Some(i) = hit_in {
        Some(seg_tip(&dev_name(inner[i].key), inner[i].hours, pie.total))
    } else {
        hit_out.map(|i| {
            let (dev, cat) = outer[i].key;
            let name = format!("{} · {}", dev_name(dev), category_name(app, cat));
            seg_tip(&name, outer[i].hours, pie.total)
        })
    };
    if let Some(t) = tip {
        resp.on_hover_text_at_pointer(t);
    }
}

/// Seconda torta: una fetta per categoria.
/// `width` = larghezza del blocco: la torta sta al centro.
fn category_pie(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, width: f32) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, PIE_D), Sense::hover());
    let c = rect.center();
    let segs = pie.category_ring();
    let hit = resp
        .hover_pos()
        .and_then(|q| crate::worker_pies::hit(c.x, c.y, 0.0, PIE_R, &segs, q.x, q.y));
    paint_ring(ui, c, 0.0, PIE_R, &segs, pie.total, |cat| category_rgb(*cat), hit);
    if let Some(i) = hit {
        let s = &segs[i];
        resp.on_hover_text_at_pointer(seg_tip(&category_name(app, s.key), s.hours, pie.total));
    }
}

/// Quadratino del colore della fetta, davanti al nome nelle tabelle.
fn swatch(ui: &mut egui::Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
    ui.painter().rect_filled(r, 2.0, color);
}

/// Larghezza delle colonne numeriche delle tabelle: ci sta l'intestazione più
/// larga («Pianificate»).
const NUM_W: f32 = 84.0;

/// Cella «Categoria»: quadratino del colore e nome, al più `CAT_W` di
/// larghezza (nome troncato con «…» e intero nel tooltip se non ci sta).
fn category_cell(ui: &mut egui::Ui, app: &App, cat: Option<CategoryId>) {
    const SW: f32 = 12.0;
    const GAP: f32 = 6.0;
    let name = category_name(app, cat);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let shown = fit_text(ui, &name, &font, CAT_W - SW - GAP);
    let galley = ui
        .painter()
        .layout_no_wrap(shown.clone(), font, ui.visuals().text_color());
    let size = egui::vec2(SW + GAP + galley.size().x, ui.spacing().interact_size.y);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let sw = Rect::from_center_size(
        egui::pos2(rect.left() + SW / 2.0, rect.center().y),
        egui::vec2(SW, SW),
    );
    ui.painter().rect_filled(sw, 2.0, category_rgb(cat));
    let at = egui::pos2(rect.left() + SW + GAP, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, ui.visuals().text_color());
    if shown != name {
        resp.on_hover_text(name);
    }
}

/// Cella numerica larga `NUM_W` col testo allineato a destra. Disegnato col
/// painter: un layout destra→sinistra dentro la griglia non allinea a destra
/// e nell'ultima colonna (senza limite) si allargava fino al bordo della
/// finestra, portando il valore fuori dalla tabella.
fn num(ui: &mut egui::Ui, s: String, strong: bool) {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = if strong {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(NUM_W, ui.spacing().interact_size.y),
        Sense::hover(),
    );
    ui.painter().text(rect.right_center(), Align2::RIGHT_CENTER, s, font, color);
}

/// Celle delle ore: fatte, pianificate e totale se `split`, altrimenti solo
/// il totale.
fn hour_cells(ui: &mut egui::Ui, h: Hours, split: bool, strong: bool) {
    if split {
        num(ui, format!("{}h", h.done), strong);
        num(ui, format!("{}h", h.planned()), strong);
    }
    num(ui, format!("{}h", h.total), strong);
}

/// Intestazioni delle colonne delle ore (vedi `hour_cells`).
fn hour_heads(ui: &mut egui::Ui, split: bool) {
    let heads: &[&str] = if split { &["Fatte", "Pianificate", "Totale"] } else { &["Ore"] };
    for h in heads {
        num(ui, h.to_string(), true);
    }
}

/// Ore totali e fatte del worker.
fn pie_hours(pie: &WorkerPie) -> Hours {
    Hours {
        total: pie.total,
        done: pie.done,
    }
}

/// Tabella della prima torta: per ogni dev una riga (ore, % sul totale), poi
/// le sue categorie (ore, % sul totale, % del dev); in fondo il totale. Con
/// `split` le ore sono divise in fatte, pianificate e totale.
/// Ritorna la larghezza della tabella.
fn dev_category_table(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, split: bool) -> f32 {
    egui::Grid::new(("pies_dev_table", pie.worker))
        .striped(true)
        .num_columns(if split { 7 } else { 5 })
        .spacing(egui::vec2(12.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Dev").strong());
            ui.label(egui::RichText::new("Categoria").strong());
            hour_heads(ui, split);
            for h in ["% totale", "% del dev"] {
                num(ui, h.to_string(), true);
            }
            ui.end_row();
            for d in &pie.devs {
                ui.horizontal(|ui| {
                    swatch(ui, dev_color(app, d.dev));
                    let name = app.devs.get(d.dev).map(|x| x.get_name()).unwrap_or_default();
                    ui.label(egui::RichText::new(name).strong());
                });
                ui.label("");
                hour_cells(ui, d.hours, split, true);
                num(ui, pct_label(d.hours.total, pie.total), true);
                ui.label("");
                ui.end_row();
                for &(cat, h) in &d.categories {
                    ui.label("");
                    category_cell(ui, app, cat);
                    hour_cells(ui, h, split, false);
                    num(ui, pct_label(h.total, pie.total), false);
                    num(ui, pct_label(h.total, d.hours.total), false);
                    ui.end_row();
                }
            }
            ui.label(egui::RichText::new("Totale").strong());
            ui.label("");
            hour_cells(ui, pie_hours(pie), split, true);
            num(ui, "100%".to_string(), true);
            ui.label("");
            ui.end_row();
        })
        .response
        .rect
        .width()
}

/// Tabella della seconda torta: ore e % sul totale per categoria.
/// Ritorna la larghezza della tabella.
fn category_table(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, split: bool) -> f32 {
    egui::Grid::new(("pies_cat_table", pie.worker))
        .striped(true)
        .num_columns(if split { 5 } else { 3 })
        .spacing(egui::vec2(12.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Categoria").strong());
            hour_heads(ui, split);
            num(ui, "% totale".to_string(), true);
            ui.end_row();
            for &(cat, h) in &pie.categories {
                category_cell(ui, app, cat);
                hour_cells(ui, h, split, false);
                num(ui, pct_label(h.total, pie.total), false);
                ui.end_row();
            }
            ui.label(egui::RichText::new("Totale").strong());
            hour_cells(ui, pie_hours(pie), split, true);
            num(ui, "100%".to_string(), true);
            ui.end_row();
        })
        .response
        .rect
        .width()
}

/// Un blocco: titolo, torta e tabella sotto, centrate l'una sull'altra. Il
/// blocco è largo quanto la più larga fra torta e tabella; la tabella si
/// disegna dopo la torta, quindi si usa la sua larghezza misurata nel frame
/// precedente (memoria temporanea di egui, chiave `key`) e, se cambia, si
/// ridisegna.
fn pie_block(
    ui: &mut egui::Ui,
    key: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    draw_pie: impl FnOnce(&mut egui::Ui, f32),
    draw_table: impl FnOnce(&mut egui::Ui) -> f32,
) {
    let id = ui.make_persistent_id(key);
    let table_w = ui.data(|d| d.get_temp::<f32>(id));
    let width = table_w.unwrap_or(PIE_D).max(PIE_D);
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).strong());
        draw_pie(ui, width);
        // Tabella più stretta della torta: centrata sotto la torta.
        let pad = ((width - table_w.unwrap_or(width)) / 2.0).max(0.0);
        let measured = ui
            .horizontal_top(|ui| {
                ui.add_space(pad);
                draw_table(ui)
            })
            .inner;
        if table_w.is_none_or(|w| (w - measured).abs() > 0.5) {
            ui.data_mut(|d| d.insert_temp(id, measured));
            ui.ctx().request_repaint();
        }
    });
}

/// Larghezza sotto la quale i due blocchi (torta con sotto la tabella) vanno
/// uno sotto l'altro invece che affiancati: ogni blocco è largo quanto la sua
/// tabella, e con le colonne fatte/pianificate le tabelle sono più larghe.
fn side_by_side_min_w(split: bool) -> f32 {
    if split { 1300.0 } else { 1000.0 }
}

/// Le due torte di un worker, ciascuna con la sua tabella sotto (come nel
/// PDF); i due blocchi affiancati se c'è spazio, altrimenti uno sotto l'altro.
fn draw_worker_pies(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, split: bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&pie.name).strong().size(16.0));
        let summary = if split {
            format!(
                "{}h nel periodo: {}h fatte, {}h pianificate",
                pie.total,
                pie.done,
                pie.total - pie.done
            )
        } else {
            format!("{}h nel periodo", pie.total)
        };
        ui.label(egui::RichText::new(summary).color(text_dim()));
        // Capacità: occupazione oltre il 100% in rosso.
        let over = pie.total > pie.capacity;
        let cap = format!(
            "· disponibili {}h, occupazione {}",
            pie.capacity,
            pct_label(pie.total, pie.capacity)
        );
        let cap = egui::RichText::new(cap);
        ui.label(if over { cap.color(error_red()).strong() } else { cap.color(text_dim()) })
            .on_hover_text(
                "Ore disponibili: somma, sulle settimane del periodo, del massimo ore \
                 effettivo del worker (limite della settimana se c'è, altrimenti il suo \
                 massimo), come nella Saturazione. Ferie e malattia contano solo se \
                 hanno abbassato il limite della settimana.",
            );
    });
    if pie.total == 0 {
        ui.label(egui::RichText::new("Nessuna ora nel periodo.").color(text_dim()));
        return;
    }
    let first = |ui: &mut egui::Ui| {
        pie_block(
            ui,
            ("pies_dev_block", pie.worker),
            "Dev e categorie",
            |ui, w| dev_category_pie(ui, app, pie, w),
            |ui| dev_category_table(ui, app, pie, split),
        );
    };
    let second = |ui: &mut egui::Ui| {
        pie_block(
            ui,
            ("pies_cat_block", pie.worker),
            "Categorie",
            |ui, w| category_pie(ui, app, pie, w),
            |ui| category_table(ui, app, pie, split),
        );
    };
    if ui.available_width() >= side_by_side_min_w(split) {
        ui.horizontal_top(|ui| {
            first(ui);
            ui.add_space(PIE_SPACING);
            second(ui);
        });
    } else {
        first(ui);
        ui.add_space(PIE_SPACING / 2.0);
        second(ui);
    }
}

pub(crate) fn pies_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_pies {
        return;
    }
    let today = today_days();
    let from = state.pies_from.unwrap_or(today);
    let to = state.pies_to.unwrap_or(today);
    let valid = from <= to;
    let now = current_monday_days();
    let pies = crate::worker_pies::build(
        app,
        &state.pies_workers,
        from,
        to,
        &state.pies_categories,
        now,
    );
    let split = crate::worker_pies::split_shown(to, now);
    let with_hours = pies.iter().filter(|p| p.total > 0).count();

    let mut open = true;
    egui::Window::new("Torte dev/categorie per worker")
        .collapsible(true)
        .resizable(true)
        .default_size(egui::vec2(1100.0, 620.0))
        .default_pos(egui::pos2(80.0, 60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            // Stessi controlli della matrice: Da / Anni / Categorie / A.
            let mut manual = false;
            let mut years_changed = false;
            ui.horizontal(|ui| {
                manual |= range_end_picker(ui, "Da:", "pies_from", &mut state.pies_from);
                ui.separator();
                years_changed = years_combo(ui, app, "pies_years", &mut state.pies_years);
                categories_combo(ui, app, "pies_categories", &mut state.pies_categories);
                ui.separator();
                manual |= range_end_picker(ui, "A:", "pies_to", &mut state.pies_to);
            });
            if years_changed {
                if let Some((f, t)) = years_range(&state.pies_years) {
                    state.pies_from = Some(f);
                    state.pies_to = Some(t);
                }
            } else if manual {
                state.pies_years.clear();
            }
            ui.horizontal(|ui| {
                if ui.button("Worker…").clicked() {
                    state.pies_select_open = true;
                }
                let can_export = valid && with_hours > 0;
                for (label, svg) in [("Esporta PDF…", false), ("Esporta SVG…", true)] {
                    if ui.add_enabled(can_export, egui::Button::new(label)).clicked() {
                        actions.push(Action::ExportWorkerPies {
                            from,
                            to,
                            years: state.pies_years.iter().copied().collect(),
                            svg,
                        });
                    }
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{} worker · {} con ore nel periodo",
                        pies.len(),
                        with_hours
                    ))
                    .color(text_dim()),
                );
            });
            ui.separator();
            if !valid {
                ui.colored_label(warn_amber(), "La data iniziale è successiva a quella finale.");
                return;
            }
            if pies.is_empty() {
                ui.label("Nessun worker selezionato: usa «Worker…».");
                return;
            }
            egui::ScrollArea::both()
                .scroll_source(drag_scroll())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, pie) in pies.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        draw_worker_pies(ui, app, pie, split);
                    }
                });
        });
    if !open {
        state.show_pies = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::single_effort::Effort;

    fn sample() -> (App, WorkerId, WorkerId) {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let luigi = app.workers.add("Luigi");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let ca = app.categories.add("Cliente A");
        let p = app.projects.add("x", Some("P1"), None);
        app.projects.set_category(p, Some(ca));
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, be);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(30));
        app.projects.add_effort(p, be, WeekId(20000), mario, Effort(10));
        app.projects.add_effort(p, fe, WeekId(20000), luigi, Effort(0));
        (app, mario, luigi)
    }

    /// Le colonne numeriche restano nella tabella: ogni valore finisce sotto
    /// la sua intestazione (allineati a destra), anche nell'ultima colonna,
    /// che prima si allargava fino al bordo della finestra.
    #[test]
    fn table_values_stay_under_their_headers() {
        let (app, mario, _) = sample();
        let sel: HashSet<_> = [mario].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), 20000);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 900.0)),
            ..Default::default()
        };
        let mut out = None;
        // La griglia misura le colonne al primo frame e le usa dal secondo.
        for _ in 0..2 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                dev_category_table(ui, &app, &pies[0], false);
                category_table(ui, &app, &pies[0], false);
            }));
        }
        let out = out.unwrap();
        let rights = |t: &str| -> Vec<f32> {
            out.shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::epaint::Shape::Text(s) if s.galley.text() == t => {
                        Some(s.pos.x + s.galley.size().x)
                    }
                    _ => None,
                })
                .collect()
        };
        let head = rights("% del dev")[0];
        // «% del dev» delle categorie: Frontend 30h → Cliente A 100%;
        // Backend 10h → Cliente A 100%.
        let pct_dev: Vec<f32> = rights("100%").into_iter().filter(|x| (x - head).abs() < 1.0).collect();
        assert_eq!(pct_dev.len(), 2, "valori sotto «% del dev» ({head}): {:?}", rights("100%"));
        assert!(rights("100%").iter().all(|x| *x < 600.0), "fuori tabella: {:?}", rights("100%"));
    }

    /// Periodo oltre la settimana corrente: intestazione con fatte e
    /// pianificate e, nelle tabelle, le colonne Fatte / Pianificate / Totale.
    #[test]
    fn planned_hours_split_shows_done_and_planned_columns() {
        let (mut app, mario, _) = sample();
        let fe = app.devs.list()[0].0;
        let p = app.projects.list()[0].0;
        app.projects.add_effort(p, fe, WeekId(20007), mario, Effort(20));
        let sel: HashSet<_> = [mario].into_iter().collect();
        // «Oggi» nella prima settimana: le 20h della seconda sono pianificate.
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20013, &Default::default(), 20000);
        assert!(crate::worker_pies::split_shown(20013, 20000));
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 900.0)),
            ..Default::default()
        };
        let out = ctx.run_ui_test(input, |ui| draw_worker_pies(ui, &app, &pies[0], true));
        let texts = drawn_texts(&out);
        // 20000..20013 non parte di lunedì: tocca tre settimane da 40h, 120h
        // disponibili, 60h → 50%.
        for t in [
            "60h nel periodo: 40h fatte, 20h pianificate",
            "· disponibili 120h, occupazione 50%",
            "Fatte",
            "Pianificate",
            "Totale",
            "20h",
            "50h",
        ] {
            assert!(texts.iter().any(|s| s == t), "manca «{t}»: {texts:?}");
        }
        // Frontend: 30h fatte + 20h pianificate = 50h.
        let without = ctx.run_ui_test(egui::RawInput::default(), |ui| {
            draw_worker_pies(ui, &app, &pies[0], false)
        });
        assert!(!drawn_texts(&without).iter().any(|s| s == "Pianificate"));
    }

    /// Colonna «Categoria» stretta: un nome lungo è troncato entro `CAT_W`;
    /// fra le due tabelle c'è almeno `PIE_SPACING`.
    #[test]
    fn category_column_is_capped_and_tables_are_spaced() {
        let (mut app, mario, _) = sample();
        let long = app.categories.add("Categoria dal nome molto molto lungo");
        let p = app.projects.add("x", Some("P2"), None);
        let fe = app.devs.list()[0].0;
        app.projects.set_category(p, Some(long));
        app.projects.add_dev(p, fe);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(5));
        let sel: HashSet<_> = [mario].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), 20000);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1600.0, 1200.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..3 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                draw_worker_pies(ui, &app, &pies[0], false)
            }));
        }
        let out = out.unwrap();
        let texts: Vec<(String, f32, f32)> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) => {
                    Some((t.galley.text().to_string(), t.pos.x, t.galley.size().x))
                }
                _ => None,
            })
            .collect();
        let cut: Vec<_> = texts.iter().filter(|(s, ..)| s.starts_with("Categoria dal")).collect();
        assert_eq!(cut.len(), 2, "nome lungo in tutte e due le tabelle: {texts:?}");
        assert!(cut.iter().all(|(s, _, w)| s.ends_with('…') && *w <= CAT_W));
        // Fine del primo blocco (ultima colonna della sua tabella) e inizio
        // del secondo (il suo titolo «Categorie»).
        let second = texts.iter().find(|(s, ..)| s == "Categorie").unwrap().1;
        let first_end = texts
            .iter()
            .filter(|(_, x, _)| *x < second)
            .map(|(_, x, w)| x + w)
            .fold(0.0, f32::max);
        assert!(second - first_end >= PIE_SPACING, "tabelle troppo vicine: {first_end} → {second}");
    }

    /// La torta è centrata sopra la sua tabella (dal secondo frame, quando
    /// la larghezza della tabella è nota).
    #[test]
    fn pie_is_centered_over_its_table() {
        let (app, mario, _) = sample();
        let sel: HashSet<_> = [mario].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), 20000);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1600.0, 1200.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..3 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                draw_worker_pies(ui, &app, &pies[0], false)
            }));
        }
        let out = out.unwrap();
        // Centro orizzontale della prima torta (le mesh a sinistra del
        // secondo blocco) e della prima tabella (da «Dev» a «% del dev»).
        let texts: Vec<(String, f32, f32)> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) => {
                    Some((t.galley.text().to_string(), t.pos.x, t.galley.size().x))
                }
                _ => None,
            })
            .collect();
        let find = |s: &str| texts.iter().find(|t| t.0 == s).unwrap().clone();
        let (_, dev_x, _) = find("Dev");
        let (_, pd_x, pd_w) = find("% del dev");
        let table_mid = (dev_x + pd_x + pd_w) / 2.0;
        let second_x = find("Categorie").1;
        let xs: Vec<f32> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Mesh(m) => Some(m.vertices.iter().map(|v| v.pos.x)),
                _ => None,
            })
            .flatten()
            .filter(|x| *x < second_x)
            .collect();
        let pie_mid = (xs.iter().copied().fold(f32::MAX, f32::min)
            + xs.iter().copied().fold(f32::MIN, f32::max))
            / 2.0;
        assert!((pie_mid - table_mid).abs() < 2.0, "torta {pie_mid} / tabella {table_mid}");
    }

    /// Ogni tabella sta **sotto** la sua torta (come nel PDF): l'intestazione
    /// «Categoria» delle tabelle è più in basso dell'area della torta.
    #[test]
    fn tables_are_drawn_below_the_pies() {
        let (app, mario, _) = sample();
        let sel: HashSet<_> = [mario].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), 20000);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 1200.0)),
            ..Default::default()
        };
        let out = ctx.run_ui_test(input, |ui| draw_worker_pies(ui, &app, &pies[0], false));
        let pie_bottom = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Mesh(m) => {
                    m.vertices.iter().map(|v| v.pos.y).reduce(f32::max)
                }
                _ => None,
            })
            .reduce(f32::max)
            .expect("torte disegnate");
        let heads: Vec<f32> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "Categoria" => Some(t.pos.y),
                _ => None,
            })
            .collect();
        assert_eq!(heads.len(), 2, "due tabelle");
        assert!(heads.iter().all(|y| *y > pie_bottom), "tabelle sotto le torte: {heads:?} / {pie_bottom}");
    }

    /// Per ogni worker: due torte (fette disegnate come mesh) con percentuali
    /// dentro e tabelle con ore e percentuali; un worker senza ore ha solo
    /// l'avviso.
    #[test]
    fn worker_pies_draw_slices_labels_and_tables() {
        let (app, mario, luigi) = sample();
        let sel: HashSet<_> = [mario, luigi].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), 20000);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 900.0)),
            ..Default::default()
        };
        let out = ctx.run_ui_test(input, |ui| {
            for p in &pies {
                draw_worker_pies(ui, &app, p, false);
            }
        });
        let texts = drawn_texts(&out);
        for t in ["Mario", "Frontend", "Backend", "Cliente A", "30h", "10h", "40h", "75%", "25%"] {
            assert!(texts.iter().any(|s| s == t), "manca «{t}»: {texts:?}");
        }
        assert!(texts.iter().any(|s| s == "Nessuna ora nel periodo."), "Luigi senza ore");
        let meshes = out
            .shapes
            .iter()
            .filter(|cs| matches!(cs.shape, egui::epaint::Shape::Mesh(_)))
            .count();
        // anello dev (2) + anello categorie (2: FE·A, BE·A) + torta categorie (1)
        assert_eq!(meshes, 5, "una mesh per fetta");
    }
}
