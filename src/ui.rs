//! Interfaccia egui (immediate-mode) — sostituisce il layer Slint.
//! Tappa 1: griglia effort fedele, editing celle, scroll sincronizzato,
//! salva/apri, aggiunta worker/dev/progetto/categoria.

use std::collections::HashMap;

use chrono::Utc;
use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Vec2};

use crate::app::App;
use crate::date_utils::dates::{
    days_to_local, local_to_days, parse_date_str, primo_giorno_settimana_corrente,
};
use crate::dev_utils::dev::DevId;
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::WeekId;
use crate::single_effort_utils::sinlge_effort::Effort;
use crate::ui_style::*;
use crate::workers_utils::worker::WORKER_ID_ZERO;

// ── Stato di sola UI ────────────────────────────────────────────────────────

struct Editing {
    proj: ProjectId,
    dev: DevId,
    week: i32,
    row: usize,
    buf: String,
    just_opened: bool,
}

#[derive(Default)]
pub struct UiState {
    current_file: String,
    changed: bool,
    this_week: i32,
    scroll_x: f32,
    scroll_y: f32,
    editing: Option<Editing>,
    // input toolbar
    new_worker: String,
    new_dev: String,
    new_category: String,
    // buffer di editing per nomi progetto ed effort dev
    name_buffers: HashMap<usize, String>,
    effort_buffers: HashMap<(usize, usize), String>,
}

// ── Azioni differite (applicate dopo il rendering) ──────────────────────────

enum Action {
    Save,
    Open,
    NewProject,
    AddWorker(String),
    AddDev(String),
    AddCategory(String),
    SetProjectName { proj: ProjectId, name: String },
    SetDevEffort { proj: ProjectId, dev: DevId, effort: usize },
    AddRow { proj: ProjectId, dev: DevId },
    CommitCell { proj: ProjectId, dev: DevId, week: WeekId, rows: Vec<String>, notes: Vec<String> },
}

pub struct PjmApp {
    app: App,
    ui: UiState,
}

impl PjmApp {
    pub fn new(app: App, current_file: String, cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let today = Utc::now().date_naive();
        let this_week = local_to_days(&primo_giorno_settimana_corrente(&today));

        Self {
            app,
            ui: UiState {
                current_file,
                this_week,
                ..Default::default()
            },
        }
    }
}

impl eframe::App for PjmApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions: Vec<Action> = Vec::new();
        {
            let app = &self.app;
            let state = &mut self.ui;

            // Ctrl+S globale
            if ui.ctx().input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                actions.push(Action::Save);
            }

            egui::TopBottomPanel::top("toolbar")
                .show_inside(ui, |ui| toolbar(ui, app, state, &mut actions));

            egui::TopBottomPanel::top("header")
                .frame(egui::Frame::NONE.fill(BG_DARK))
                .show_inside(ui, |ui| header(ui, app, state));

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(BG_DARK))
                .show_inside(ui, |ui| body(ui, app, state, &mut actions));
        }

        for a in actions {
            self.apply(a);
        }
    }
}

impl PjmApp {
    fn mark_changed(&mut self) {
        self.app.recompute_week_range();
        self.app.compute_sovra();
        self.ui.changed = true;
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::Save => {
                self.app.save(&self.ui.current_file);
                self.ui.changed = false;
            }
            Action::Open => {
                if let Some(path_buf) =
                    rfd::FileDialog::new().add_filter("RON files", &["ron"]).pick_file()
                {
                    let path = path_buf.to_string_lossy().to_string();
                    match App::load(&path) {
                        Ok(loaded) => {
                            self.app = loaded;
                            self.ui.current_file = path;
                            self.ui.name_buffers.clear();
                            self.ui.effort_buffers.clear();
                            self.ui.editing = None;
                            self.app.compute_sovra();
                            self.ui.changed = false;
                        }
                        Err(e) => eprintln!("Errore apertura '{path}': {e}"),
                    }
                }
            }
            Action::NewProject => {
                let start = parse_date_str(
                    &primo_giorno_settimana_corrente(&Utc::now().date_naive())
                        .format("%y-%m-%d")
                        .to_string(),
                )
                .map(|d| WeekId(d as usize));
                self.app.projects.add("Nuovo Progetto", None, start);
                self.mark_changed();
            }
            Action::AddWorker(name) => {
                if !name.is_empty() {
                    self.app.workers.add(&name);
                    self.mark_changed();
                }
            }
            Action::AddDev(name) => {
                if !name.is_empty() {
                    self.app.devs.add(&name);
                    self.mark_changed();
                }
            }
            Action::AddCategory(name) => {
                if !name.is_empty() {
                    self.app.categories.add(&name);
                    self.mark_changed();
                }
            }
            Action::SetProjectName { proj, name } => {
                self.app.projects.set_project_info(proj, &name);
                self.ui.changed = true;
            }
            Action::SetDevEffort { proj, dev, effort } => {
                self.app.projects.add_dev_effort(proj, dev, Effort(effort));
                self.mark_changed();
            }
            Action::AddRow { proj, dev } => {
                if let Some(week) = self.app.projects.get_week_with_max_worker(proj, dev) {
                    self.app.projects.add_effort(proj, dev, week, WORKER_ID_ZERO, Effort(0));
                    self.mark_changed();
                }
            }
            Action::CommitCell { proj, dev, week, rows, notes } => {
                self.app.projects.reset_effort(proj, dev, week);
                for (text, note) in rows.iter().zip(notes.iter()) {
                    let parts: Vec<&str> = text.split('|').collect();
                    if parts.len() != 2 {
                        continue;
                    }
                    if let Some(wid) = self.app.workers.get_id_by_name(parts[0].trim()) {
                        let e = parts[1].trim().parse::<usize>().unwrap_or(0);
                        self.app.projects.add_effort(proj, dev, week, wid, Effort(e));
                        if !note.is_empty() {
                            self.app.projects.set_note(proj, dev, week, wid, note);
                        }
                    }
                }
                self.mark_changed();
            }
        }
    }
}

// ── Helper griglia ──────────────────────────────────────────────────────────

fn weeks_vec(app: &App) -> Vec<i32> {
    (app.start_week.0..=app.end_week.0).step_by(7).map(|w| w as i32).collect()
}

fn dev_max_rows(app: &App, proj: ProjectId, dev: DevId) -> usize {
    app.projects
        .get_single_dev(proj, dev)
        .map(|sd| sd.max_num_efforts())
        .unwrap_or(0)
        .max(1)
}

fn dev_block_height(max_rows: usize) -> f32 {
    DEV_BORDER + (max_rows as f32 + 1.0) * ROW_H + DEV_BORDER
}

/// Slot (testo "nome|effort", nota) per ogni riga della settimana, riempiti con vuoti.
fn gather_slots(
    app: &App,
    proj: ProjectId,
    dev: DevId,
    week: i32,
    max_rows: usize,
) -> Vec<(String, String)> {
    let mut slots: Vec<(String, String)> = app
        .projects
        .get_single_dev(proj, dev)
        .and_then(|sd| sd.get_all(WeekId(week as usize)))
        .map(|sew| {
            let mut v: Vec<(String, String)> = sew
                .worker_id
                .iter()
                .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                .map(|(wid, se)| {
                    let name = app.workers.get_name_by_id(*wid);
                    (format!("{}|{}", name, se.get_effort().0), se.get_note())
                })
                .collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        })
        .unwrap_or_default();
    while slots.len() < max_rows {
        slots.push((String::new(), String::new()));
    }
    slots
}

// ── Toolbar ─────────────────────────────────────────────────────────────────

fn toolbar(ui: &mut egui::Ui, _app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if ui.button("+ Progetto").clicked() {
            actions.push(Action::NewProject);
        }
        ui.separator();

        let we = ui.add(
            egui::TextEdit::singleline(&mut state.new_worker)
                .hint_text("Nome worker…")
                .desired_width(120.0),
        );
        if (we.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.button("+ Worker").clicked()
        {
            if !state.new_worker.is_empty() {
                actions.push(Action::AddWorker(std::mem::take(&mut state.new_worker)));
            }
        }

        let de = ui.add(
            egui::TextEdit::singleline(&mut state.new_dev)
                .hint_text("Nome dev…")
                .desired_width(120.0),
        );
        if (de.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.button("+ Dev").clicked()
        {
            if !state.new_dev.is_empty() {
                actions.push(Action::AddDev(std::mem::take(&mut state.new_dev)));
            }
        }

        let ce = ui.add(
            egui::TextEdit::singleline(&mut state.new_category)
                .hint_text("Nome categoria…")
                .desired_width(120.0),
        );
        if (ce.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.button("+ Categoria").clicked()
        {
            if !state.new_category.is_empty() {
                actions.push(Action::AddCategory(std::mem::take(&mut state.new_category)));
            }
        }

        ui.separator();
        if ui.button("Salva").clicked() {
            actions.push(Action::Save);
        }
        if ui.button("Apri").clicked() {
            actions.push(Action::Open);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let col = if state.changed { EFFORT_ORANGE } else { TEXT_DIM };
            let label = format!("{}{}", state.current_file, if state.changed { " (*)" } else { "" });
            ui.colored_label(col, label);
        });
    });
}

// ── Header (date settimane) ─────────────────────────────────────────────────

fn header(ui: &mut egui::Ui, app: &App, state: &UiState) {
    let weeks = weeks_vec(app);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.add_space(LEFT_W);
        egui::ScrollArea::horizontal()
            .id_salt("header_scroll")
            .scroll_offset(Vec2::new(state.scroll_x, 0.0))
            .enable_scrolling(false)
            .show(ui, |ui| {
                let content_w = weeks.len() as f32 * COL_W;
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(content_w, ROW_H), Sense::hover());
                let painter = ui.painter_at(rect);
                for (i, w) in weeks.iter().enumerate() {
                    let x = rect.left() + i as f32 * COL_W;
                    let cell = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, ROW_H));
                    if *w == state.this_week {
                        painter.rect_filled(cell, 0.0, THIS_WEEK.gamma_multiply(0.5));
                    }
                    let txt = primo_giorno_settimana_corrente(&days_to_local(*w))
                        .format("%y-%m-%d")
                        .to_string();
                    painter.text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        txt,
                        cell_font(),
                        TEXT_WHITE,
                    );
                }
            });
    });
}

// ── Corpo: colonna sinistra + griglia con scroll sincronizzato ──────────────

fn body(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let mut new_offset = Vec2::new(state.scroll_x, state.scroll_y);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let sy = state.scroll_y;
        egui::ScrollArea::vertical()
            .id_salt("left_scroll")
            .vertical_scroll_offset(sy)
            .enable_scrolling(false)
            .max_width(LEFT_W)
            .auto_shrink([false, false])
            .show(ui, |ui| left_column(ui, app, state, actions));

        let out = egui::ScrollArea::both()
            .id_salt("grid_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| grid(ui, app, state, actions));
        new_offset = out.state.offset;
    });
    state.scroll_x = new_offset.x;
    state.scroll_y = new_offset.y;
}

fn alloc_strip(ui: &mut egui::Ui, w: f32, h: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
}

// ── Griglia (colonna destra) ────────────────────────────────────────────────

fn grid(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let weeks = weeks_vec(app);
    let content_w = weeks.len() as f32 * COL_W;
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // striscia gialla superiore (color-start-stop)
    alloc_strip(ui, content_w, DEV_BORDER, START_STOP);

    for (_pi, (proj_id, _)) in app.projects.list().iter().enumerate() {
        if !app.projects.get_enable(proj_id).0 {
            continue;
        }
        let proj_start = app.projects.get_project_start_week(*proj_id).map(|w| w.0 as i32).unwrap_or(-1);
        let deadline = app.projects.get_project_end_week(*proj_id).map(|w| w.0 as i32).unwrap_or(-1);

        for dev_id in app.projects.list_devs(*proj_id) {
            let dev_color = dev_color(app, dev_id);
            let max_rows = dev_max_rows(app, *proj_id, dev_id);

            alloc_strip(ui, content_w, DEV_BORDER, dev_color);

            let block_h = (max_rows as f32 + 1.0) * ROW_H;
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(content_w, block_h), Sense::hover());
            draw_dev_cells(
                ui, rect, app, state, actions, *proj_id, dev_id, max_rows, &weeks, proj_start,
                deadline,
            );

            alloc_strip(ui, content_w, DEV_BORDER, dev_color);
        }

        alloc_strip(ui, content_w, DEV_BORDER, BETWEEN_PROJECTS);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_dev_cells(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    dev: DevId,
    max_rows: usize,
    weeks: &[i32],
    proj_start: i32,
    deadline: i32,
) {
    let planned = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| sd.planned_effort().0 as i32)
        .unwrap_or(0);
    let hide_effort = app.projects.get_single_dev(proj, dev).map(|sd| sd.get_hide_effort()).unwrap_or(false);

    let mut running = 0i32;
    for (ci, w) in weeks.iter().enumerate() {
        let x = rect.left() + ci as f32 * COL_W;
        let before_start = proj_start >= 0 && *w < proj_start;
        let after_deadline = deadline >= 0 && *w > deadline;
        let is_deadline = deadline >= 0 && *w == deadline;

        // colonna settimana: bg deadline/start
        let col_rect = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, rect.height()));
        if is_deadline {
            ui.painter().rect_filled(col_rect, 0.0, DEADLINE_BG);
        } else if proj_start >= 0 && *w == proj_start {
            ui.painter().rect_filled(col_rect, 0.0, START_BG);
        }
        if *w == state.this_week {
            ui.painter().rect_filled(col_rect, 0.0, THIS_WEEK.gamma_multiply(0.18));
        }

        let week_total = app
            .projects
            .get_single_dev(proj, dev)
            .map(|sd| sd.get_effort_by_week(WeekId(*w as usize)).0 as i32)
            .unwrap_or(0);
        running += week_total;

        // riga cumulativo (sola lettura)
        if !before_start && !after_deadline {
            let cum_rect = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, ROW_H));
            let has_workers = app
                .projects
                .get_single_dev(proj, dev)
                .and_then(|sd| sd.get_all(WeekId(*w as usize)))
                .map(|s| s.worker_id.keys().any(|k| *k != WORKER_ID_ZERO))
                .unwrap_or(false);
            if (has_workers || is_deadline) && !hide_effort {
                let remaining = planned - running;
                let txt = if running != remaining {
                    format!("{} | {}", running, remaining)
                } else {
                    format!("{}", remaining)
                };
                let color = if is_deadline { TEXT_WHITE } else { cumulative_color(week_total, planned) };
                ui.painter().text(cum_rect.center(), Align2::CENTER_CENTER, txt, cell_font(), color);
            }
        }

        // righe persona (editabili)
        if before_start || after_deadline {
            continue;
        }
        let slots = gather_slots(app, proj, dev, *w, max_rows);
        for (row, (text, note)) in slots.iter().enumerate() {
            let y = rect.top() + (row as f32 + 1.0) * ROW_H;
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(COL_W, ROW_H));

            let is_editing = state
                .editing
                .as_ref()
                .map(|e| e.proj == proj && e.dev == dev && e.week == *w && e.row == row)
                .unwrap_or(false);

            if is_editing {
                let mut ed = state.editing.take().unwrap();
                ui.painter().rect_filled(cell, 0.0, SEL_BG);
                let resp = ui.put(
                    cell,
                    egui::TextEdit::singleline(&mut ed.buf)
                        .font(cell_font())
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::symmetric(2, 0)),
                );
                if ed.just_opened {
                    resp.request_focus();
                    ed.just_opened = false;
                }
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                if esc {
                    // annulla
                } else if enter || resp.lost_focus() {
                    let mut s = gather_slots(app, proj, dev, *w, max_rows);
                    s[row].0 = ed.buf.clone();
                    let rows: Vec<String> = s.iter().map(|(t, _)| t.clone()).collect();
                    let notes: Vec<String> = s.iter().map(|(_, n)| n.clone()).collect();
                    actions.push(Action::CommitCell {
                        proj,
                        dev,
                        week: WeekId(*w as usize),
                        rows,
                        notes,
                    });
                } else {
                    state.editing = Some(ed);
                }
            } else {
                let resp = ui.interact(
                    cell,
                    egui::Id::new(("cell", proj.0, dev.0, *w, row)),
                    Sense::click(),
                );
                if resp.hovered() {
                    ui.painter().rect_filled(cell, 0.0, SEL_BG.gamma_multiply(0.4));
                }
                // bordo nota
                if !note.is_empty() {
                    draw_note_triangle(ui, cell);
                }
                if !text.is_empty() {
                    let parts: Vec<&str> = text.split('|').collect();
                    let sovra = app
                        .workers
                        .get_id_by_name(parts.first().copied().unwrap_or("").trim())
                        .map(|wid| {
                            app.sovra.get(&(WeekId(*w as usize), wid)).map_or(0, |e| e.0 as i32)
                        })
                        .unwrap_or(0);
                    let max_h = app
                        .workers
                        .get_id_by_name(parts.first().copied().unwrap_or("").trim())
                        .map(|wid| app.workers.get_effective_max_hours(wid, *w as usize) as i32)
                        .unwrap_or(DEFAULT_MAX_HOURS as i32);
                    let color = if sovra > max_h { Color32::RED } else { TEXT_WHITE };
                    ui.painter().text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        text,
                        cell_font(),
                        color,
                    );
                }
                if resp.clicked() {
                    state.editing = Some(Editing {
                        proj,
                        dev,
                        week: *w,
                        row,
                        buf: text.clone(),
                        just_opened: true,
                    });
                }
            }
        }
    }
}

fn draw_note_triangle(ui: &egui::Ui, cell: Rect) {
    let tr = cell.right_top();
    let pts = vec![
        egui::pos2(tr.x - 10.0, tr.y),
        egui::pos2(tr.x, tr.y),
        egui::pos2(tr.x, tr.y + 10.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(pts, NOTE_ORANGE, Stroke::NONE));
}

fn dev_color(app: &App, dev: DevId) -> Color32 {
    app.devs
        .list_full()
        .into_iter()
        .find(|(id, _, _, _)| *id == dev)
        .map(|(_, _, bg, _)| from_hex(bg as u32))
        .unwrap_or(Color32::from_rgb(0x00, 0x99, 0xFF))
}

fn dev_text_color(app: &App, dev: DevId) -> Color32 {
    app.devs
        .list_full()
        .into_iter()
        .find(|(id, _, _, _)| *id == dev)
        .map(|(_, _, _, font)| from_hex(font as u32))
        .unwrap_or(TEXT_WHITE)
}

fn dev_name(app: &App, dev: DevId) -> String {
    app.devs
        .list()
        .into_iter()
        .find(|(id, _)| *id == dev)
        .map(|(_, n)| n)
        .unwrap_or_default()
}

// ── Colonna sinistra ────────────────────────────────────────────────────────

fn left_column(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // striscia gialla superiore (allinea con la griglia)
    alloc_strip(ui, LEFT_W, DEV_BORDER, START_STOP);

    for (_pi, (proj_id, proj_name)) in app.projects.list().iter().enumerate() {
        if !app.projects.get_enable(proj_id).0 {
            continue;
        }
        let dev_ids = app.projects.list_devs(*proj_id);
        let proj_h: f32 = dev_ids
            .iter()
            .map(|d| dev_block_height(dev_max_rows(app, *proj_id, *d)))
            .sum();
        let proj_h = proj_h.max(dev_block_height(1));

        let (rect, _) = ui.allocate_exact_size(Vec2::new(LEFT_W, proj_h), Sense::hover());

        draw_project_info(ui, rect, app, state, actions, *proj_id, proj_name);
        draw_left_dev_strip(ui, rect);
        draw_left_devs(ui, rect, app, state, actions, *proj_id, &dev_ids);

        alloc_strip(ui, LEFT_W, DEV_BORDER, BETWEEN_PROJECTS);
    }
}

fn draw_project_info(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    proj_name: &str,
) {
    let x = rect.left();
    let w = LEFT_INFO_W;
    let mut y = rect.top();

    // tripletta
    let trip = app.projects.get_tripletta(proj);
    let trip_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    if trip.is_empty() {
        ui.painter().text(trip_rect.center(), Align2::CENTER_CENTER, "—", cell_font(), TEXT_FAINT);
    } else {
        ui.painter().text(trip_rect.center(), Align2::CENTER_CENTER, &trip, cell_font(), EFFORT_ORANGE);
    }
    y += ROW_H;

    // categoria
    let cat = app
        .projects
        .get_category(proj)
        .and_then(|c| app.categories.get_name(c))
        .unwrap_or("—")
        .to_string();
    let cat_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    let cat_col = if cat == "—" { TEXT_FAINT } else { CAT_BLUE };
    ui.painter().text(cat_rect.center(), Align2::CENTER_CENTER, cat, cell_font(), cat_col);
    y += ROW_H;

    // nome progetto (editabile)
    let name_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    let buf = state.name_buffers.entry(proj.0).or_insert_with(|| proj_name.to_string());
    let resp = ui.put(
        name_rect,
        egui::TextEdit::singleline(buf).font(cell_font()).frame(egui::Frame::NONE).horizontal_align(egui::Align::Center),
    );
    if resp.lost_focus() {
        actions.push(Action::SetProjectName { proj, name: buf.clone() });
    } else if !resp.has_focus() && *buf != proj_name {
        *buf = proj_name.to_string();
    }
    y += ROW_H;

    // inizio / fine (sola lettura in questa tappa)
    let start_txt = app
        .projects
        .get_project_start_week(proj)
        .map(|wk| format!("Inizio: {}", primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32)).format("%y-%m-%d")))
        .unwrap_or_else(|| "Inizio: —".to_string());
    let sr = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(sr.center(), Align2::CENTER_CENTER, start_txt, mono(FONT_SIZE - 2.0), TEXT_DIM);
    y += ROW_H;

    let end_txt = app
        .projects
        .get_project_end_week(proj)
        .map(|wk| format!("Fine: {}", primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32)).format("%y-%m-%d")))
        .unwrap_or_else(|| "Fine: —".to_string());
    let er = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(er.center(), Align2::CENTER_CENTER, end_txt, mono(FONT_SIZE - 2.0), TEXT_DIM);
}

fn draw_left_dev_strip(ui: &egui::Ui, rect: Rect) {
    let x = rect.left() + LEFT_INFO_W;
    let strip = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(DEV_STRIP_W, rect.height()));
    ui.painter().rect_filled(strip, 0.0, Color32::from_rgb(0x22, 0x22, 0x22));
    ui.painter().rect_stroke(
        strip,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(0x55, 0x55, 0x55)),
        egui::StrokeKind::Inside,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_left_devs(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    dev_ids: &[DevId],
) {
    let x0 = rect.left() + LEFT_INFO_W + DEV_STRIP_W;
    let mut y = rect.top();

    for dev in dev_ids {
        let max_rows = dev_max_rows(app, proj, *dev);
        let color = dev_color(app, *dev);
        let tcol = dev_text_color(app, *dev);
        let block_h = dev_block_height(max_rows);

        // bordo superiore
        let top_b = Rect::from_min_size(egui::pos2(x0, y), Vec2::new(LEFT_DEV_W, DEV_BORDER));
        ui.painter().rect_filled(top_b, 0.0, color);
        let inner_y = y + DEV_BORDER;
        let inner_h = (max_rows as f32 + 1.0) * ROW_H;

        // cella nome dev (90px, doppio click = add row)
        let name_rect = Rect::from_min_size(egui::pos2(x0, inner_y), Vec2::new(DEV_NAME_W, inner_h));
        ui.painter().rect_filled(name_rect, 0.0, color);
        ui.painter().text(name_rect.center(), Align2::CENTER_CENTER, dev_name(app, *dev), cell_font(), tcol);
        let nresp = ui.interact(name_rect, egui::Id::new(("devname", proj.0, dev.0)), Sense::click());
        if nresp.double_clicked() {
            actions.push(Action::AddRow { proj, dev: *dev });
        }

        // area effort/remains (65px)
        let eff_x = x0 + DEV_NAME_W;
        let eff_area = Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, inner_h));
        ui.painter().rect_filled(eff_area, 0.0, color);

        // effort pianificato (editabile)
        let planned = app.projects.get_single_dev(proj, *dev).map(|sd| sd.planned_effort().0).unwrap_or(0);
        let eff_rect = Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, ROW_H));
        let key = (proj.0, dev.0);
        let buf = state.effort_buffers.entry(key).or_insert_with(|| planned.to_string());
        let resp = ui.put(
            eff_rect,
            egui::TextEdit::singleline(buf).font(cell_font()).frame(egui::Frame::NONE).horizontal_align(egui::Align::Center),
        );
        if resp.lost_focus() {
            if let Ok(v) = buf.trim().parse::<usize>() {
                actions.push(Action::SetDevEffort { proj, dev: *dev, effort: v });
            }
        } else if !resp.has_focus() && *buf != planned.to_string() {
            *buf = planned.to_string();
        }

        // remains
        let total = app.projects.get_single_dev(proj, *dev).map(|sd| sd.get_effort_tot().0 as i32).unwrap_or(0);
        let remains = planned as i32 - total;
        let rem_rect = Rect::from_min_size(egui::pos2(eff_x, inner_y + ROW_H), Vec2::new(DEV_EFFORT_W, ROW_H));
        if (remains == planned as i32 && planned != 0) || remains < 0 {
            ui.painter().rect_filled(rem_rect, 0.0, Color32::RED);
        }
        ui.painter().text(rem_rect.center(), Align2::CENTER_CENTER, remains.to_string(), cell_font(), TEXT_WHITE);

        // bordo inferiore
        let bot_b = Rect::from_min_size(egui::pos2(x0, inner_y + inner_h), Vec2::new(LEFT_DEV_W, DEV_BORDER));
        ui.painter().rect_filled(bot_b, 0.0, color);

        y += block_h;
    }
}
