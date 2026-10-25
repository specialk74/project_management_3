//! Torte dev/categorie per worker (Vista ▸ Torte dev/categorie per worker…):
//! come la matrice, prima la dialog dei worker (selezione propria, solo
//! sessione), poi la finestra con l'intervallo di date e, per ogni worker, due
//! torte con la tabella dei valori accanto. Dati e geometria in
//! `crate::worker_pies`, condivisi con l'export PDF/SVG.

use super::*;
use crate::worker_matrix::category_color;
use crate::worker_pies::{Seg, WorkerPie, label_pos, pct_label};

/// Diametro dell'area di una torta (px).
const PIE_D: f32 = 340.0;
/// Raggio esterno delle torte.
const PIE_R: f32 = 162.0;
/// Raggio dell'anello interno (dev) della prima torta; l'anello esterno
/// (categorie) parte da qui + `PIE_GAP`.
const PIE_R_IN: f32 = 100.0;
const PIE_GAP: f32 = 4.0;
/// Spazio fra la prima torta (con la sua tabella) e la seconda.
const PIE_SPACING: f32 = 32.0;

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
fn dev_category_pie(ui: &mut egui::Ui, app: &App, pie: &WorkerPie) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(PIE_D, PIE_D), Sense::hover());
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
fn category_pie(ui: &mut egui::Ui, app: &App, pie: &WorkerPie) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(PIE_D, PIE_D), Sense::hover());
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
/// larga («% del dev»).
const NUM_W: f32 = 72.0;

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

/// Tabella della prima torta: per ogni dev una riga (ore, % sul totale), poi
/// le sue categorie (ore, % sul totale, % del dev); in fondo il totale.
fn dev_category_table(ui: &mut egui::Ui, app: &App, pie: &WorkerPie) {
    egui::Grid::new(("pies_dev_table", pie.worker))
        .striped(true)
        .num_columns(5)
        .spacing(egui::vec2(12.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Dev").strong());
            ui.label(egui::RichText::new("Categoria").strong());
            for h in ["Ore", "% totale", "% del dev"] {
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
                num(ui, format!("{}h", d.hours), true);
                num(ui, pct_label(d.hours, pie.total), true);
                ui.label("");
                ui.end_row();
                for &(cat, h) in &d.categories {
                    ui.label("");
                    ui.horizontal(|ui| {
                        swatch(ui, category_rgb(cat));
                        ui.label(category_name(app, cat));
                    });
                    num(ui, format!("{h}h"), false);
                    num(ui, pct_label(h, pie.total), false);
                    num(ui, pct_label(h, d.hours), false);
                    ui.end_row();
                }
            }
            ui.label(egui::RichText::new("Totale").strong());
            ui.label("");
            num(ui, format!("{}h", pie.total), true);
            num(ui, "100%".to_string(), true);
            ui.label("");
            ui.end_row();
        });
}

/// Tabella della seconda torta: ore e % sul totale per categoria.
fn category_table(ui: &mut egui::Ui, app: &App, pie: &WorkerPie) {
    egui::Grid::new(("pies_cat_table", pie.worker))
        .striped(true)
        .num_columns(3)
        .spacing(egui::vec2(12.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Categoria").strong());
            for h in ["Ore", "% totale"] {
                num(ui, h.to_string(), true);
            }
            ui.end_row();
            for &(cat, h) in &pie.categories {
                ui.horizontal(|ui| {
                    swatch(ui, category_rgb(cat));
                    ui.label(category_name(app, cat));
                });
                num(ui, format!("{h}h"), false);
                num(ui, pct_label(h, pie.total), false);
                ui.end_row();
            }
            ui.label(egui::RichText::new("Totale").strong());
            num(ui, format!("{}h", pie.total), true);
            num(ui, "100%".to_string(), true);
            ui.end_row();
        });
}

/// Le due torte di un worker, ciascuna con la sua tabella a destra.
fn draw_worker_pies(ui: &mut egui::Ui, app: &App, pie: &WorkerPie) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&pie.name).strong().size(16.0));
        ui.label(egui::RichText::new(format!("{}h nel periodo", pie.total)).color(text_dim()));
    });
    if pie.total == 0 {
        ui.label(egui::RichText::new("Nessuna ora nel periodo.").color(text_dim()));
        return;
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Dev e categorie").strong());
            ui.horizontal_top(|ui| {
                dev_category_pie(ui, app, pie);
                dev_category_table(ui, app, pie);
            });
        });
        ui.add_space(PIE_SPACING);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Categorie").strong());
            ui.horizontal_top(|ui| {
                category_pie(ui, app, pie);
                category_table(ui, app, pie);
            });
        });
    });
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
    let pies =
        crate::worker_pies::build(app, &state.pies_workers, from, to, &state.pies_categories);
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
                        draw_worker_pies(ui, app, pie);
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
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default());
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 900.0)),
            ..Default::default()
        };
        let mut out = None;
        // La griglia misura le colonne al primo frame e le usa dal secondo.
        for _ in 0..2 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| {
                dev_category_table(ui, &app, &pies[0]);
                category_table(ui, &app, &pies[0]);
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

    /// Per ogni worker: due torte (fette disegnate come mesh) con percentuali
    /// dentro e tabelle con ore e percentuali; un worker senza ore ha solo
    /// l'avviso.
    #[test]
    fn worker_pies_draw_slices_labels_and_tables() {
        let (app, mario, luigi) = sample();
        let sel: HashSet<_> = [mario, luigi].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default());
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1400.0, 900.0)),
            ..Default::default()
        };
        let out = ctx.run_ui_test(input, |ui| {
            for p in &pies {
                draw_worker_pies(ui, &app, p);
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
