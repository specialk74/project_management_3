//! Torte dev/categorie per worker (Vista ▸ Torte dev/categorie per worker…):
//! come la matrice, prima la dialog dei worker (selezione propria, solo
//! sessione), poi la finestra con l'intervallo di date e, per ogni worker, due
//! torte con la tabella dei valori sotto: la prima a tre anelli (dev,
//! categorie, progetti), la seconda per categoria. I progetti (tripletta)
//! compaiono anche nei tooltip delle fette e nelle tabelle. Dati e geometria in
//! `crate::worker_pies`, condivisi con l'export PDF/SVG.

use super::*;
use crate::worker_matrix::category_color;
use crate::worker_pies::{Hours, ProjHours, Seg, WorkerPie, label_pos, pct_label};

/// Diametro dell'area di una torta (px).
const PIE_D: f32 = 340.0;
/// Raggio esterno delle torte.
const PIE_R: f32 = 162.0;
/// Prima torta: raggio dello spicchio dei dev; l'anello delle categorie va da
/// qui + `PIE_GAP` a `PIE_R_CAT`, quello dei progetti da `PIE_R_CAT` +
/// `PIE_GAP` a `PIE_R`.
const PIE_R_IN: f32 = 78.0;
const PIE_R_CAT: f32 = 120.0;
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

/// Larghezza della combo dei dev (come quelle di anni e categorie).
const DEVS_COMBO_W: f32 = 150.0;

/// Combo dei dev con le spunte (nome sul colore del dev): con almeno una
/// scelta le torte contano solo le ore di quei dev (vuoto = tutti), come la
/// combo Categorie: con «tutti» le voci sono tutte spuntate e si tolgono una a
/// una (`toggle_in_all`); l'ultima rimasta non si toglie.
pub(crate) fn devs_combo(
    ui: &mut egui::Ui,
    app: &App,
    salt: &str,
    devs: &mut std::collections::BTreeSet<DevId>,
) {
    let all = app.devs.list();
    let text = if devs.is_empty() {
        "Dev: tutti".to_string()
    } else {
        let names: Vec<&str> = all
            .iter()
            .filter(|(d, _)| devs.contains(d))
            .map(|(_, n)| n.as_str())
            .collect();
        format!("Dev: {}", names.join(", "))
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(DEVS_COMBO_W)
        .show_ui(ui, |ui| {
            let ids: Vec<DevId> = all.iter().map(|(d, _)| *d).collect();
            // I dev nascosti non si elencano: restano compresi nel «tutti» e
            // contano come spuntati togliendo gli altri.
            let visible: Vec<DevId> = ids.iter().copied().filter(|d| !app.devs.is_hidden(*d)).collect();
            for (d, name) in &all {
                if app.devs.is_hidden(*d) {
                    continue;
                }
                let mut on = ticked_in_all(devs, d);
                let fill = dev_color(app, *d);
                let label = egui::RichText::new(name)
                    .background_color(fill)
                    .color(contrast_text(fill));
                let enabled = can_untick_visible(devs, &visible, d);
                if ui.add_enabled(enabled, egui::Checkbox::new(&mut on, label)).changed() {
                    toggle_in_all(devs, &ids, d, on);
                }
            }
            if !devs.is_empty() {
                ui.separator();
                if ui.button("Tutti").clicked() {
                    devs.clear();
                }
            }
        })
        .response
        .on_hover_text("Togli la spunta ai dev da escludere.");
}

/// Nome di una priorità per le combo e le tabelle (`None` = «Senza priorità»).
pub(crate) fn priority_name(app: &App, p: Option<PriorityId>) -> String {
    match p {
        Some(id) => app.priorities.get_name(id).unwrap_or("?").to_string(),
        None => "Senza priorità".to_string(),
    }
}

/// Larghezza della combo Priorità (matrice e torte).
pub(crate) const PRI_COMBO_W: f32 = 150.0;

/// Combo delle priorità (prima «Senza priorità», poi le priorità per id): con
/// almeno una scelta matrice e torte contano solo i progetti con quelle
/// priorità (vuoto = tutte), come la combo Categorie: con «tutte» le voci sono
/// tutte spuntate e si tolgono una a una (`toggle_in_all`); l'ultima non si
/// toglie. `salt` distingue le combo delle due finestre.
pub(crate) fn priorities_combo(
    ui: &mut egui::Ui,
    app: &App,
    salt: &str,
    pris: &mut std::collections::BTreeSet<Option<PriorityId>>,
) {
    let all: Vec<(Option<PriorityId>, String)> = std::iter::once((None, priority_name(app, None)))
        .chain(app.priorities.list().into_iter().map(|(id, n)| (Some(id), n)))
        .collect();
    let text = if pris.is_empty() {
        "Priorità: tutte".to_string()
    } else {
        let names: Vec<&str> = all
            .iter()
            .filter(|(p, _)| pris.contains(p))
            .map(|(_, n)| n.as_str())
            .collect();
        format!("Priorità: {}", names.join(", "))
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(PRI_COMBO_W)
        .show_ui(ui, |ui| {
            let ids: Vec<Option<PriorityId>> = all.iter().map(|(p, _)| *p).collect();
            // Le priorità nascoste non si elencano: restano comprese nel
            // «tutte» e contano come spuntate togliendo le altre.
            let hidden = |p: &Option<PriorityId>| p.is_some_and(|p| app.priorities.is_hidden(p));
            let visible: Vec<Option<PriorityId>> = ids.iter().copied().filter(|p| !hidden(p)).collect();
            for (p, name) in &all {
                if hidden(p) {
                    continue;
                }
                let mut on = ticked_in_all(pris, p);
                let enabled = can_untick_visible(pris, &visible, p);
                if ui.add_enabled(enabled, egui::Checkbox::new(&mut on, name)).changed() {
                    toggle_in_all(pris, &ids, p, on);
                }
            }
            if !pris.is_empty() {
                ui.separator();
                if ui.button("Tutte").clicked() {
                    pris.clear();
                }
            }
        })
        .response
        .on_hover_text("Togli la spunta alle priorità da escludere.");
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

/// Tooltip di una fetta seguito dai suoi progetti, uno per riga (tripletta,
/// ore e % sul totale del worker).
fn seg_tip_projects<'a>(
    app: &App,
    name: &str,
    hours: u32,
    total: u32,
    projects: impl Iterator<Item = &'a ProjHours>,
) -> String {
    let mut t = seg_tip(name, hours, total);
    for p in projects {
        t.push_str("\n  ");
        t.push_str(&seg_tip(&project_label(app, p.project), p.hours.total, total));
    }
    t
}

/// Colore del progetto nella torta del worker.
fn project_rgb(pie: &WorkerPie, proj: ProjectId) -> Color32 {
    rgb_color(pie.project_color(proj))
}

/// Prima torta: anello interno per dev, poi per categoria dentro ogni dev e,
/// all'esterno, per progetto dentro ogni dev·categoria.
/// `width` = larghezza del blocco: la torta sta al centro.
fn dev_category_pie(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, width: f32) {
    use crate::worker_pies::hit;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, PIE_D), Sense::hover());
    let c = rect.center();
    let inner = pie.dev_ring();
    let middle = pie.dev_category_ring();
    let outer = pie.dev_project_ring();
    let (r_cat, r_proj) = (PIE_R_IN + PIE_GAP, PIE_R_CAT + PIE_GAP);
    let pos = resp.hover_pos();
    let hit_in = pos.and_then(|q| hit(c.x, c.y, 0.0, PIE_R_IN, &inner, q.x, q.y));
    let hit_mid = pos.and_then(|q| hit(c.x, c.y, r_cat, PIE_R_CAT, &middle, q.x, q.y));
    let hit_out = pos.and_then(|q| hit(c.x, c.y, r_proj, PIE_R, &outer, q.x, q.y));
    let dev_name = |d: DevId| app.devs.get(d).map(|d| d.get_name()).unwrap_or_default();
    let dev_hours = |d: DevId| pie.devs.iter().find(|x| x.dev == d);
    paint_ring(ui, c, 0.0, PIE_R_IN, &inner, pie.total, |d| dev_color(app, *d), hit_in);
    paint_ring(ui, c, r_cat, PIE_R_CAT, &middle, pie.total, |(_, cat)| category_rgb(*cat), hit_mid);
    paint_ring(ui, c, r_proj, PIE_R, &outer, pie.total, |(_, p)| project_rgb(pie, *p), hit_out);
    let tip = if let Some(i) = hit_in {
        let s = &inner[i];
        let projects = dev_hours(s.key).into_iter().flat_map(|d| d.projects.iter());
        Some(seg_tip_projects(app, &dev_name(s.key), s.hours, pie.total, projects))
    } else if let Some(i) = hit_mid {
        let s = &middle[i];
        let (dev, cat) = s.key;
        let name = format!("{} · {}", dev_name(dev), category_name(app, cat));
        let projects = dev_hours(dev).into_iter().flat_map(|d| d.projects_in(cat));
        Some(seg_tip_projects(app, &name, s.hours, pie.total, projects))
    } else {
        hit_out.map(|i| {
            let (dev, proj) = outer[i].key;
            let cat = app.projects.get_category(proj);
            let name = format!(
                "{} · {} · {}",
                dev_name(dev),
                category_name(app, cat),
                project_label(app, proj)
            );
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
        let name = category_name(app, s.key);
        let projects = pie.projects_in(s.key);
        resp.on_hover_text_at_pointer(seg_tip_projects(app, &name, s.hours, pie.total, projects));
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
    swatch_cell(ui, category_name(app, cat), category_rgb(cat), 0.0);
}

/// Rientro delle righe dei progetti sotto la loro categoria.
const PROJ_INDENT: f32 = 14.0;

/// Cella di un progetto (riga sotto la sua categoria): rientrata, quadratino
/// della tinta del progetto e tripletta.
fn project_cell(ui: &mut egui::Ui, app: &App, pie: &WorkerPie, proj: ProjectId) {
    swatch_cell(ui, project_label(app, proj), project_rgb(pie, proj), PROJ_INDENT);
}

/// Quadratino del colore e nome dopo `indent` px, al più `CAT_W` di larghezza
/// in tutto (nome troncato con «…» e intero nel tooltip se non ci sta).
fn swatch_cell(ui: &mut egui::Ui, name: String, color: Color32, indent: f32) {
    const SW: f32 = 12.0;
    const GAP: f32 = 6.0;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let shown = fit_text(ui, &name, &font, CAT_W - indent - SW - GAP);
    let galley = ui
        .painter()
        .layout_no_wrap(shown.clone(), font, ui.visuals().text_color());
    let size = egui::vec2(indent + SW + GAP + galley.size().x, ui.spacing().interact_size.y);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let left = rect.left() + indent;
    let sw = Rect::from_center_size(egui::pos2(left + SW / 2.0, rect.center().y), egui::vec2(SW, SW));
    ui.painter().rect_filled(sw, 2.0, color);
    let at = egui::pos2(left + SW + GAP, rect.center().y - galley.size().y / 2.0);
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
/// le sue categorie (ore, % sul totale, % del dev), ognuna seguita dai suoi
/// progetti (rientrati, stesse colonne); in fondo il totale. Con
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
                    for p in d.projects_in(cat) {
                        ui.label("");
                        project_cell(ui, app, pie, p.project);
                        hour_cells(ui, p.hours, split, false);
                        num(ui, pct_label(p.hours.total, pie.total), false);
                        num(ui, pct_label(p.hours.total, d.hours.total), false);
                        ui.end_row();
                    }
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

/// Tabella della seconda torta: ore e % sul totale per categoria, ognuna
/// seguita dai suoi progetti (rientrati).
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
                for p in pie.projects_in(cat) {
                    project_cell(ui, app, pie, p.project);
                    hour_cells(ui, p.hours, split, false);
                    num(ui, pct_label(p.hours.total, pie.total), false);
                    ui.end_row();
                }
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

/// Riga «da qui a fine periodo»: ore pianificate (dei dev e delle categorie
/// selezionati), disponibili e libere (o, se negative, oltre la capacità, in
/// rosso).
fn future_text(pie: &WorkerPie) -> egui::RichText {
    let free = pie.free_hours();
    let head = format!(
        "Da qui a fine periodo: {}h pianificate su {}h disponibili, ",
        pie.planned(), pie.future_capacity
    );
    if free >= 0 {
        egui::RichText::new(format!("{head}{free}h libere")).color(text_dim())
    } else {
        egui::RichText::new(format!("{head}{}h oltre la capacità", -free))
            .color(error_red())
            .strong()
    }
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
                pie.planned()
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
    if split {
        ui.label(future_text(pie)).on_hover_text(
            "Dalla settimana corrente (compresa) a fine periodo: ore disponibili \
             (come il numero verde accanto al worker nel footer) meno ore già \
             pianificate. Le ore pianificate sono solo quelle dei dev e delle \
             categorie selezionati.",
        );
    }
    if pie.total == 0 {
        ui.label(egui::RichText::new("Nessuna ora nel periodo.").color(text_dim()));
        return;
    }
    let first = |ui: &mut egui::Ui| {
        pie_block(
            ui,
            ("pies_dev_block", pie.worker),
            "Dev, categorie e progetti",
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
    let pies = crate::worker_pies::build_filtered(
        app,
        &state.pies_workers,
        from,
        to,
        &state.pies_categories,
        &state.pies_devs,
        &state.pies_priorities,
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
            // Stessi controlli della matrice (Da / Anni / Categorie / A) più
            // la scelta dei dev e delle priorità.
            let mut manual = false;
            let mut years_changed = false;
            ui.horizontal(|ui| {
                manual |= range_end_picker(ui, "Da:", "pies_from", &mut state.pies_from);
                ui.separator();
                years_changed = years_combo(ui, app, "pies_years", &mut state.pies_years);
                categories_combo(ui, app, "pies_categories", &mut state.pies_categories);
                priorities_combo(ui, app, "pies_priorities", &mut state.pies_priorities);
                devs_combo(ui, app, "pies_devs", &mut state.pies_devs);
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
                if ui
                    .add_enabled(can_export, egui::Button::new("Esporta CSV…"))
                    .on_hover_text("Una riga per worker, dev e progetto: ore totali, fatte e pianificate, con categoria e priorità.")
                    .clicked()
                {
                    actions.push(Action::ExportWorkerPiesCsv { from, to });
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
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), &Default::default(), 20000);
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
        // «% del dev» delle categorie e dei loro progetti: Frontend 30h →
        // Cliente A 100% → P1 100%; Backend 10h → Cliente A 100% → P1 100%.
        let pct_dev: Vec<f32> = rights("100%").into_iter().filter(|x| (x - head).abs() < 1.0).collect();
        assert_eq!(pct_dev.len(), 4, "valori sotto «% del dev» ({head}): {:?}", rights("100%"));
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
        // «Oggi» nella seconda settimana: la prima è conclusa (fatta), le 20h
        // della seconda sono pianificate.
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20013, &Default::default(), &Default::default(), 20007);
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
            // Dalla settimana corrente: solo l'ultima del periodo (lunedì
            // 20011) è ≥ 20007, 40h; pianificate 20h.
            "Da qui a fine periodo: 20h pianificate su 40h disponibili, 20h libere",
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
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), &Default::default(), 20000);
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
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), &Default::default(), 20000);
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
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), &Default::default(), 20000);
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
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), &Default::default(), 20000);
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
        for t in ["Mario", "Frontend", "Backend", "Cliente A", "P1", "30h", "10h", "40h", "75%", "25%"] {
            assert!(texts.iter().any(|s| s == t), "manca «{t}»: {texts:?}");
        }
        assert!(texts.iter().any(|s| s == "Nessuna ora nel periodo."), "Luigi senza ore");
        let meshes = out
            .shapes
            .iter()
            .filter(|cs| matches!(cs.shape, egui::epaint::Shape::Mesh(_)))
            .count();
        // anello dev (2) + anello categorie (2: FE·A, BE·A) + anello progetti
        // (2: FE·P1, BE·P1) + torta categorie (1)
        assert_eq!(meshes, 7, "una mesh per fetta");
        // P1 nelle due tabelle: sotto FE·Cliente A, BE·Cliente A e Cliente A.
        assert_eq!(texts.iter().filter(|s| *s == "P1").count(), 3, "{texts:?}");
    }

    /// Tooltip: le fette dei dev e delle categorie elencano i loro progetti;
    /// quella di un progetto dice dev, categoria e tripletta.
    #[test]
    fn slice_tooltips_list_the_projects() {
        let (mut app, mario, _) = sample();
        let fe = app.devs.list()[0].0;
        let p2 = app.projects.add("x", Some("P2"), None);
        app.projects.add_dev(p2, fe);
        app.projects.add_effort(p2, fe, WeekId(20000), mario, Effort(10));
        let sel: HashSet<_> = [mario].into_iter().collect();
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), &Default::default(), 20000);
        let pie = &pies[0];
        let fe_hours = pie.devs.iter().find(|d| d.dev == fe).unwrap();
        let tip = seg_tip_projects(&app, "Frontend", 40, pie.total, fe_hours.projects.iter());
        assert_eq!(tip, "Frontend: 40h (80%)\n  P1: 30h (60%)\n  P2: 10h (20%)");
        let tip = seg_tip_projects(&app, "Senza categoria", 10, pie.total, pie.projects_in(None));
        assert_eq!(tip, "Senza categoria: 10h (20%)\n  P2: 10h (20%)");
        // Colori dei progetti diversi da quello della categoria.
        let ca = app.projects.get_category(pie.projects[0].project);
        assert_ne!(project_rgb(pie, pie.projects[0].project), category_rgb(ca));
    }
}
