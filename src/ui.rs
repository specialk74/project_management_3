//! Interfaccia egui (immediate-mode) — sostituisce il layer Slint.
//! Tappa 1: griglia effort fedele, editing celle, scroll sincronizzato,
//! salva/apri, aggiunta worker/dev/progetto/categoria.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Utc};
use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Vec2};

use crate::app::App;
use crate::categories::CategoryId;
use crate::date_utils::dates::{
    days_to_local, local_to_days, parse_date_str, primo_giorno_settimana_corrente,
};
use crate::dev_utils::dev::DevId;
use crate::project_utils::project::{Enable, ProjectId};
use crate::single_dev_utils::single_dev::WeekId;
use crate::single_effort_utils::sinlge_effort::Effort;
use crate::ui_style::*;
use crate::workers_utils::worker::{WORKER_ID_ZERO, WorkerId};

// ── Stato di sola UI ────────────────────────────────────────────────────────

struct Editing {
    proj: ProjectId,
    dev: DevId,
    week: i32,
    row: usize,
    buf: String,         // testo mostrato (dopo autocomplete)
    typed: String,       // ciò che l'utente ha realmente digitato
    just_opened: bool,
    had_focus: bool,
    paste_note: Option<String>, // nota incollata (Ctrl+V) da applicare al commit
    orig_worker: String, // worker presente nella cella all'apertura (per commit mirato)
    orig_note: String,   // nota presente nella cella all'apertura
}

/// Bersaglio del note editor.
enum NoteTarget {
    Effort { proj: ProjectId, dev: DevId, week: i32, worker: String },
    Dev { proj: ProjectId, dev: DevId },
}

struct NoteEditing {
    target: NoteTarget,
    text: String,
}

/// Popup di modifica (tripletta / inizio / fine / categoria) aperti da right/left-click.
enum Popup {
    Tripletta { proj: ProjectId, text: String },
    Start { proj: ProjectId, text: String },
    End { proj: ProjectId, text: String },
    Category { proj: ProjectId },
    /// Ore max globali di un worker (click sul nome nel footer sinistro).
    WorkerMax { worker: WorkerId, name: String, text: String },
    /// Override ore max di un worker per una specifica settimana (click sulla cella sovra).
    WorkerWeekMax { worker: WorkerId, name: String, week: usize, text: String },
}

#[derive(Default)]
pub struct UiState {
    current_file: String,
    changed: bool,
    this_week: i32,
    scroll_x: f32,
    scroll_y: f32,
    // scroll orizzontale iniziale da applicare alla griglia (settimana corrente)
    pending_scroll_x: Option<f32>,
    editing: Option<Editing>,
    note_editor: Option<NoteEditing>,
    popup: Option<Popup>,
    dev_manage: Option<ProjectId>,
    confirm_del_dev: Option<(ProjectId, DevId)>,
    // filtro worker: None = nessun filtro (tutti); Some(set) = mostra solo questi nomi
    worker_filter: Option<HashSet<String>>,
    show_worker_filter: bool,
    show_project_filter: bool,
    // appunti per copia/incolla cella
    copied_text: String,
    copied_note: String,
    // 0 = tutti gli effort, 1 = solo nulli, 2 = solo >= 40
    effort_filter_mode: i32,
    compact_mode: bool,
    // selettori per i totali-anno per dev nel footer
    selected_year: i32, // 0 = nessuno
    selected_category: Option<CategoryId>, // None = tutte
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
    SetNote { proj: ProjectId, dev: DevId, week: WeekId, worker: String, note: String },
    SetDevNote { proj: ProjectId, dev: DevId, note: String },
    SetProjectTripletta { proj: ProjectId, text: String },
    SetProjectStartWeek { proj: ProjectId, date: String },
    SetProjectEndWeek { proj: ProjectId, date: String },
    SetProjectCategory { proj: ProjectId, cat: Option<CategoryId> },
    DelRow { proj: ProjectId, dev: DevId },
    SetDevHideEffort { proj: ProjectId, dev: DevId, hide: bool },
    AddDevToProject { proj: ProjectId, dev: DevId, add: bool },
    SetProjectEnabled { proj: ProjectId, enabled: bool },
    SetAllProjectsEnabled { enabled: bool },
    SetWorkerMaxHours { worker: WorkerId, hours: u32 },
    SetWorkerWeekOverride { worker: WorkerId, week: usize, hours: u32 },
    MoveProjectUp { proj: ProjectId },
    MoveProjectDown { proj: ProjectId },
}

pub struct PjmApp {
    app: App,
    ui: UiState,
}

/// Rende disponibili i simboli geometrici (es. "▼") nei pulsanti (che usano il
/// font proporzionale) aggiungendo come fallback i font monospace di egui —
/// "Hack" include "▼". Soluzione portabile (nessun font di sistema, nessun file
/// esterno): i font sono già impacchettati da egui su macOS/Windows/Linux.
fn install_symbol_fallback(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mono = fonts.families.get(&egui::FontFamily::Monospace).cloned().unwrap_or_default();
    let prop = fonts.families.entry(egui::FontFamily::Proportional).or_default();
    for name in mono {
        if !prop.contains(&name) {
            prop.push(name);
        }
    }
    ctx.set_fonts(fonts);
}

impl PjmApp {
    pub fn new(app: App, current_file: String, cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|s| s.interaction.tooltip_delay = 0.0);
        install_symbol_fallback(&cc.egui_ctx);

        let today = Utc::now().date_naive();
        let this_week = local_to_days(&primo_giorno_settimana_corrente(&today));

        // Scroll iniziale per centrare la settimana corrente (come nel main.rs Slint).
        let start_week = app.start_week.0 as i32;
        let col_index = (this_week - start_week) / 7;
        let pending_scroll_x = if col_index > 0 {
            const INITIAL_WINDOW_WIDTH: f32 = 1024.0;
            let visible_width = INITIAL_WINDOW_WIDTH - LEFT_W;
            let col_center = col_index as f32 * COL_W + COL_W / 2.0;
            Some((col_center - visible_width / 2.0).max(0.0))
        } else {
            None
        };

        Self {
            app,
            ui: UiState {
                current_file,
                this_week,
                selected_year: today.year(),
                scroll_x: pending_scroll_x.unwrap_or(0.0),
                pending_scroll_x,
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

            // Scorciatoie globali (Cmd su macOS, Ctrl altrove). Calcolate in anticipo
            // per non trattenere un borrow di `ui` durante i pannelli.
            let (key_s, key_f, shift) = ui.ctx().input(|i| {
                let cmd = i.modifiers.command || i.modifiers.ctrl;
                (cmd && i.key_pressed(egui::Key::S), cmd && i.key_pressed(egui::Key::F), i.modifiers.shift)
            });
            if key_s {
                actions.push(Action::Save);
            }
            if key_f {
                if shift {
                    state.worker_filter = Some(HashSet::new()); // deseleziona tutti
                } else {
                    state.show_worker_filter = true;
                }
            }

            egui::TopBottomPanel::top("toolbar")
                .show_inside(ui, |ui| toolbar(ui, app, state, &mut actions));

            egui::TopBottomPanel::top("header")
                .frame(egui::Frame::NONE.fill(BG_DARK))
                .show_inside(ui, |ui| header(ui, app, state));

            // Footer (worker / sovra) — nascosto in vista compatta.
            if !state.compact_mode {
                egui::TopBottomPanel::bottom("footer")
                    .frame(egui::Frame::NONE.fill(BG_DARK))
                    .show_inside(ui, |ui| footer(ui, app, state));
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(BG_DARK))
                .show_inside(ui, |ui| body(ui, app, state, &mut actions));

            note_editor_window(ui.ctx(), state, &mut actions);
            popup_window(ui.ctx(), app, state, &mut actions);
            dev_manage_window(ui.ctx(), app, state, &mut actions);
            confirm_del_dev_window(ui.ctx(), state, &mut actions);
            worker_filter_window(ui.ctx(), app, state);
            project_filter_window(ui.ctx(), app, state, &mut actions);
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
            Action::SetNote { proj, dev, week, worker, note } => {
                if let Some(wid) = self.app.workers.get_id_by_name(&worker) {
                    self.app.projects.set_note(proj, dev, week, wid, &note);
                    self.mark_changed();
                }
            }
            Action::SetDevNote { proj, dev, note } => {
                self.app.projects.set_dev_note(proj, dev, &note);
                self.mark_changed();
            }
            Action::SetProjectTripletta { proj, text } => {
                self.app.projects.set_tripletta(proj, &text);
                self.mark_changed();
            }
            Action::SetProjectStartWeek { proj, date } => {
                let wk = if date.trim().is_empty() {
                    None
                } else {
                    parse_date_str(&date).map(|d| WeekId(d as usize))
                };
                self.app.projects.set_project_start_week(proj, wk);
                self.mark_changed();
            }
            Action::SetProjectEndWeek { proj, date } => {
                let wk = if date.trim().is_empty() {
                    None
                } else {
                    parse_date_str(&date).map(|d| WeekId(d as usize))
                };
                self.app.projects.set_project_end_week(proj, wk);
                self.mark_changed();
            }
            Action::SetProjectCategory { proj, cat } => {
                self.app.projects.set_category(proj, cat);
                self.mark_changed();
            }
            Action::DelRow { proj, dev } => {
                self.app.projects.del_row(proj, dev);
                self.mark_changed();
            }
            Action::SetDevHideEffort { proj, dev, hide } => {
                self.app.projects.set_dev_hide_effort(proj, dev, hide);
                self.mark_changed();
            }
            Action::AddDevToProject { proj, dev, add } => {
                if add {
                    self.app.projects.add_dev(proj, dev);
                } else {
                    self.app.projects.del_dev(proj, dev);
                }
                self.mark_changed();
            }
            Action::SetProjectEnabled { proj, enabled } => {
                self.app.projects.set_enable(proj, Enable(enabled));
                self.mark_changed();
            }
            Action::SetAllProjectsEnabled { enabled } => {
                let ids: Vec<_> = self.app.projects.list().iter().map(|(id, _)| *id).collect();
                for id in ids {
                    self.app.projects.set_enable(id, Enable(enabled));
                }
                self.mark_changed();
            }
            Action::SetWorkerMaxHours { worker, hours } => {
                self.app.workers.set_max_hours(worker, hours);
                self.mark_changed();
            }
            Action::SetWorkerWeekOverride { worker, week, hours } => {
                self.app.workers.set_week_override(worker, week, hours);
                self.mark_changed();
            }
            Action::MoveProjectUp { proj } => {
                if self.app.projects.move_up(proj) {
                    self.mark_changed();
                }
            }
            Action::MoveProjectDown { proj } => {
                if self.app.projects.move_down(proj) {
                    self.mark_changed();
                }
            }
        }
    }
}

// ── Autocomplete (replica members.rs::find_completion) ──────────────────────

fn common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = first.chars().zip(s.chars()).take_while(|(a, b)| a == b).count().min(len);
    }
    first[..len].to_string()
}

fn find_completion(app: &App, prefix: &str, pipe: bool) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    let matches: Vec<String> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, name)| *id != WORKER_ID_ZERO && name.starts_with(prefix))
        .map(|(_, name)| name)
        .collect();
    if matches.is_empty() {
        String::new()
    } else if matches.len() == 1 {
        if pipe { format!("{}|", matches[0]) } else { matches[0].clone() }
    } else {
        common_prefix(&matches)
    }
}

/// Logica Slint: completa `typed`; se nessun match tieni il digitato, altrimenti
/// adotta il completamento come nuovo prefisso.
fn recompute_completion(app: &App, typed: &mut String, buf: &mut String) {
    let c = find_completion(app, typed, true);
    if c.is_empty() {
        *buf = typed.clone();
    } else {
        *buf = c.clone();
        *typed = c;
    }
}

// ── Helper griglia ──────────────────────────────────────────────────────────

fn weeks_vec(app: &App) -> Vec<i32> {
    (app.start_week.0..=app.end_week.0).step_by(7).map(|w| w as i32).collect()
}

/// Anni disponibili (da inizio/fine progetti), ordinati.
fn available_years(app: &App) -> Vec<i32> {
    use std::collections::BTreeSet;
    let mut years = BTreeSet::new();
    for (proj_id, _) in app.projects.list() {
        if let Some(w) = app.projects.get_project_start_week(proj_id) {
            years.insert(days_to_local(w.0 as i32).year());
        }
        if let Some(w) = app.projects.get_project_end_week(proj_id) {
            years.insert(days_to_local(w.0 as i32).year());
        }
    }
    years.into_iter().collect()
}

/// Totale effort del dev nell'anno selezionato (filtrato per categoria).
fn dev_year_total(app: &App, dev: DevId, year: i32, cat: Option<CategoryId>) -> i32 {
    if year == 0 {
        return 0;
    }
    app.projects
        .list()
        .iter()
        .filter(|(pid, _)| match cat {
            None => true,
            Some(c) => app.projects.get_category(*pid) == Some(c),
        })
        .map(|(pid, _)| {
            app.projects
                .get_single_dev(*pid, dev)
                .map(|sd| {
                    sd.get_weeks()
                        .iter()
                        .filter(|w| days_to_local(w.0 as i32).year() == year)
                        .map(|w| sd.get_effort_by_week(*w).0 as i32)
                        .sum::<i32>()
                })
                .unwrap_or(0)
        })
        .sum()
}

type Filter = Option<HashSet<String>>;

fn worker_shown(filter: &Filter, name: &str) -> bool {
    match filter {
        None => true,
        Some(set) => set.contains(name),
    }
}

/// max_rows del dev tenendo conto del filtro.
/// None = dev da nascondere (filtro attivo e nessun worker selezionato con dati).
fn filtered_dev_max_rows(app: &App, proj: ProjectId, dev: DevId, filter: &Filter) -> Option<usize> {
    let Some(sd) = app.projects.get_single_dev(proj, dev) else {
        return if filter.is_some() { None } else { Some(1) };
    };
    match filter {
        None => Some(sd.max_num_efforts().max(1)),
        Some(set) => {
            let mut maxc = 0usize;
            for week in sd.get_weeks() {
                if let Some(sew) = sd.get_all(week) {
                    let c = sew
                        .worker_id
                        .iter()
                        .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                        .filter(|(wid, _)| set.contains(app.workers.get_name_by_id(**wid)))
                        .count();
                    maxc = maxc.max(c);
                }
            }
            if maxc == 0 { None } else { Some(maxc) }
        }
    }
}

fn col_w(compact: bool) -> f32 {
    if compact { COMPACT_W } else { COL_W }
}

fn dev_block_height(max_rows: usize, compact: bool) -> f32 {
    let inner = if compact { ROW_H } else { (max_rows as f32 + 1.0) * ROW_H };
    DEV_BORDER + inner + DEV_BORDER
}

/// Slot (testo "nome|effort", nota) per ogni riga della settimana, riempiti con vuoti.
/// Con filtro attivo include solo i worker selezionati.
fn gather_slots(
    app: &App,
    proj: ProjectId,
    dev: DevId,
    week: i32,
    max_rows: usize,
    filter: &Filter,
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
                .filter(|(wid, _)| worker_shown(filter, app.workers.get_name_by_id(**wid)))
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

fn toolbar(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if ui.button("+ Progetto").clicked() {
            actions.push(Action::NewProject);
        }
        if ui.button("Progetti ▼").clicked() {
            state.show_project_filter = !state.show_project_filter;
        }
        let filter_on = state.worker_filter.is_some();
        let wbtn = egui::Button::new("Workers ▼");
        let wbtn = if filter_on { wbtn.fill(Color32::from_rgb(0x2a, 0x50, 0x80)) } else { wbtn };
        if ui.add(wbtn).clicked() {
            state.show_worker_filter = !state.show_worker_filter;
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
        let compact_label = if state.compact_mode { "Vista normale" } else { "Vista compatta" };
        if ui.button(compact_label).clicked() {
            state.compact_mode = !state.compact_mode;
        }

        ui.separator();
        // Selettore anno (per i totali-anno nel footer)
        let year_label = if state.selected_year == 0 {
            "Anno".to_string()
        } else {
            state.selected_year.to_string()
        };
        egui::ComboBox::from_id_salt("year_combo")
            .selected_text(year_label)
            .width(70.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.selected_year, 0, "—");
                for y in available_years(app) {
                    ui.selectable_value(&mut state.selected_year, y, y.to_string());
                }
            });
        // Selettore categoria (scope dei totali-anno)
        let cat_label = match state.selected_category {
            None => "Tutte".to_string(),
            Some(c) => app.categories.get_name(c).unwrap_or("?").to_string(),
        };
        egui::ComboBox::from_id_salt("cat_combo")
            .selected_text(cat_label)
            .width(110.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.selected_category, None, "Tutte");
                for (id, name) in app.categories.list() {
                    ui.selectable_value(&mut state.selected_category, Some(id), name);
                }
            });

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
    // Riserva i 300px sinistri con lo stesso meccanismo della griglia (SidePanel),
    // così l'origine X delle colonne coincide esattamente.
    egui::SidePanel::left("hdr_left")
        .exact_width(LEFT_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(BG_DARK))
        .show_inside(ui, |_ui| {});

    egui::ScrollArea::horizontal()
        .id_salt("header_scroll")
        .scroll_offset(Vec2::new(state.scroll_x, 0.0))
        .enable_scrolling(false)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let compact = state.compact_mode;
            let cw = col_w(compact);
            let content_w = weeks.len() as f32 * cw;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, ROW_H), Sense::hover());
            for (i, w) in weeks.iter().enumerate() {
                let x = rect.left() + i as f32 * cw;
                let cell = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cw, ROW_H));
                if *w == state.this_week {
                    ui.painter().rect_filled(cell, 0.0, THIS_WEEK.gamma_multiply(0.5));
                }
                let txt = primo_giorno_settimana_corrente(&days_to_local(*w))
                    .format("%y-%m-%d")
                    .to_string();
                if compact {
                    // colonne troppo strette per il testo → la data nel tooltip
                    ui.interact(cell, egui::Id::new(("hdr", *w)), Sense::hover())
                        .on_hover_text(txt);
                } else {
                    ui.painter().text(cell.center(), Align2::CENTER_CENTER, txt, cell_font(), TEXT_WHITE);
                }
            }
        });
}

// ── Corpo: colonna sinistra + griglia con scroll sincronizzato ──────────────

fn body(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let top_down = egui::Layout::top_down(egui::Align::Min);
    // Il filtro è clonato una volta per frame per evitare conflitti di borrow.
    let filter = state.worker_filter.clone();

    // Colonna sinistra: larghezza fissa, scroll verticale che segue la griglia.
    egui::SidePanel::left("leftcol")
        .exact_width(LEFT_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(BG_DARK))
        .show_inside(ui, |ui| {
            let sy = state.scroll_y;
            egui::ScrollArea::vertical()
                .id_salt("left_scroll")
                .vertical_scroll_offset(sy)
                .enable_scrolling(false)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.with_layout(top_down, |ui| left_column(ui, app, state, actions, &filter));
                });
        });

    // Griglia: prende lo spazio restante, è la "master" dello scroll.
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DARK))
        .show_inside(ui, |ui| {
            let mut sa = egui::ScrollArea::both().id_salt("grid_scroll").auto_shrink([false, false]);
            if let Some(px) = state.pending_scroll_x {
                sa = sa.scroll_offset(Vec2::new(px, 0.0));
            }
            let out = sa.show(ui, |ui| {
                ui.with_layout(top_down, |ui| grid(ui, app, state, actions, &filter));
            });
            state.scroll_x = out.state.offset.x;
            state.scroll_y = out.state.offset.y;
            state.pending_scroll_x = None;
        });
}

fn alloc_strip(ui: &mut egui::Ui, w: f32, h: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
}

// ── Footer (worker + sovra) ─────────────────────────────────────────────────

// ── Note editor (finestra) ──────────────────────────────────────────────────

fn note_editor_window(ctx: &egui::Context, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(ne) = &mut state.note_editor else {
        return;
    };
    let title = match &ne.target {
        NoteTarget::Effort { worker, .. } => format!("Nota: {}", worker),
        NoteTarget::Dev { .. } => "Nota Dev".to_string(),
    };
    let mut open = true;
    let mut save = false;
    let mut cancel = false;

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut ne.text)
                    .desired_rows(4)
                    .desired_width(320.0)
                    .font(cell_font()),
            );
            ui.horizontal(|ui| {
                if ui.button("Salva").clicked() {
                    save = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });

    if save {
        let action = match &ne.target {
            NoteTarget::Effort { proj, dev, week, worker } => Action::SetNote {
                proj: *proj,
                dev: *dev,
                week: WeekId(*week as usize),
                worker: worker.clone(),
                note: ne.text.clone(),
            },
            NoteTarget::Dev { proj, dev } => Action::SetDevNote {
                proj: *proj,
                dev: *dev,
                note: ne.text.clone(),
            },
        };
        actions.push(action);
    }
    if save || cancel || !open {
        state.note_editor = None;
    }
}

// ── Gestione dev del progetto (+Dev / −Dev) ─────────────────────────────────

fn dev_manage_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(proj) = state.dev_manage else {
        return;
    };
    let in_proj = app.projects.list_devs(proj);
    let mut open = true;
    let mut close = false;

    egui::Window::new("Dev del progetto")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            for (dev, name) in app.devs.list() {
                let is_in = in_proj.contains(&dev);
                let col = dev_color(app, dev);
                let txt_col = if is_in { dev_text_color(app, dev) } else { Color32::from_gray(0x88) };
                let fill = if is_in { col } else { Color32::TRANSPARENT };
                let resp = ui.add(
                    egui::Button::new(egui::RichText::new(&name).color(txt_col).monospace())
                        .fill(fill)
                        .min_size(Vec2::new(160.0, 18.0)),
                );
                if resp.clicked() {
                    if is_in {
                        let has_data = app
                            .projects
                            .get_single_dev(proj, dev)
                            .map(|sd| sd.planned_effort().0 > 0 || !sd.get_weeks().is_empty())
                            .unwrap_or(false);
                        if has_data {
                            state.confirm_del_dev = Some((proj, dev));
                            close = true;
                        } else {
                            actions.push(Action::AddDevToProject { proj, dev, add: false });
                        }
                    } else {
                        actions.push(Action::AddDevToProject { proj, dev, add: true });
                    }
                }
            }
        });

    if close || !open {
        state.dev_manage = None;
    }
}

fn confirm_del_dev_window(ctx: &egui::Context, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some((proj, dev)) = state.confirm_del_dev else {
        return;
    };
    let mut open = true;
    let mut close = false;

    egui::Window::new("Conferma rimozione")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Questo dev ha già dati inseriti.\nRimuovere comunque?");
            ui.horizontal(|ui| {
                if ui.button("Sì, rimuovi").clicked() {
                    actions.push(Action::AddDevToProject { proj, dev, add: false });
                    close = true;
                }
                if ui.button("Annulla").clicked() {
                    close = true;
                }
            });
        });

    if close || !open {
        state.confirm_del_dev = None;
    }
}

// ── Filtro progetti (Progetti ▼) ────────────────────────────────────────────

fn project_filter_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    if !state.show_project_filter {
        return;
    }
    let projects = app.projects.list_full(); // (id, name, enable)
    let mut open = true;

    egui::Window::new("Progetti")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(90.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Seleziona tutti").clicked() {
                    actions.push(Action::SetAllProjectsEnabled { enabled: true });
                }
                if ui.button("Deseleziona tutti").clicked() {
                    actions.push(Action::SetAllProjectsEnabled { enabled: false });
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for (id, name, en) in &projects {
                    let mut on = en.0;
                    let trip = app.projects.get_tripletta(*id);
                    // solo la tripletta (fallback al nome se la tripletta è vuota)
                    let label = if trip.is_empty() { name.clone() } else { trip };
                    if ui.checkbox(&mut on, label).changed() {
                        actions.push(Action::SetProjectEnabled { proj: *id, enabled: on });
                    }
                }
            });
        });

    if !open {
        state.show_project_filter = false;
    }
}

// ── Filtro worker (Ctrl+F / Workers ▼) ──────────────────────────────────────

fn worker_filter_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_worker_filter {
        return;
    }
    let all: Vec<String> = app.workers.list().into_iter().map(|(_, n)| n).collect();
    let mut open = true;
    let mut filter = state.worker_filter.clone();

    egui::Window::new("Filtro Workers")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(140.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Seleziona tutti").clicked() {
                    filter = None;
                }
                if ui.button("Deseleziona tutti").clicked() {
                    filter = Some(HashSet::new());
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for name in &all {
                    let mut sel = match &filter {
                        None => true,
                        Some(s) => s.contains(name),
                    };
                    if ui.checkbox(&mut sel, name).changed() {
                        let set = filter.get_or_insert_with(|| all.iter().cloned().collect());
                        if sel {
                            set.insert(name.clone());
                        } else {
                            set.remove(name);
                        }
                    }
                }
            });
        });

    // se tutti selezionati → nessun filtro
    if let Some(set) = &filter {
        if set.len() == all.len() && all.iter().all(|n| set.contains(n)) {
            filter = None;
        }
    }
    state.worker_filter = filter;
    if !open {
        state.show_worker_filter = false;
    }
}

// ── Popup di modifica (tripletta / inizio / fine / categoria) ───────────────

fn popup_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(popup) = &mut state.popup else {
        return;
    };
    let mut open = true;
    let mut close = false;

    match popup {
        Popup::Tripletta { proj, text } => {
            egui::Window::new("Tripletta")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    let le = ui.add(
                        egui::TextEdit::singleline(text).desired_width(220.0).font(cell_font()),
                    );
                    let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || entered {
                            actions.push(Action::SetProjectTripletta { proj: *proj, text: text.clone() });
                            close = true;
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
        Popup::Start { proj, text } => {
            date_popup_window(ctx, "Data inizio (yy-mm-dd)", text, &mut open, &mut close, |date| {
                actions.push(Action::SetProjectStartWeek { proj: *proj, date });
            });
        }
        Popup::End { proj, text } => {
            date_popup_window(ctx, "Data fine (yy-mm-dd)", text, &mut open, &mut close, |date| {
                actions.push(Action::SetProjectEndWeek { proj: *proj, date });
            });
        }
        Popup::Category { proj } => {
            egui::Window::new("Categoria")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    if ui.button("— Nessuna").clicked() {
                        actions.push(Action::SetProjectCategory { proj: *proj, cat: None });
                        close = true;
                    }
                    for (id, name) in app.categories.list() {
                        if ui.button(name).clicked() {
                            actions.push(Action::SetProjectCategory { proj: *proj, cat: Some(id) });
                            close = true;
                        }
                    }
                });
        }
        Popup::WorkerMax { worker, name, text } => {
            hours_popup_window(
                ctx,
                &format!("Ore max: {name}"),
                text,
                &mut open,
                &mut close,
                |hours| {
                    actions.push(Action::SetWorkerMaxHours { worker: *worker, hours });
                },
                DEFAULT_MAX_HOURS,
            );
        }
        Popup::WorkerWeekMax { worker, name, week, text } => {
            // "Default" = max globale del worker (azzera l'override per la settimana).
            let global_max = app.workers.get_max_hours(*worker);
            hours_popup_window(
                ctx,
                &format!("Override settimana - {name}"),
                text,
                &mut open,
                &mut close,
                |hours| {
                    actions.push(Action::SetWorkerWeekOverride { worker: *worker, week: *week, hours });
                },
                global_max,
            );
        }
    }

    if close || !open {
        state.popup = None;
    }
}

/// Finestra con campo data + OK / Rimuovi / Annulla. `on_confirm` riceve la stringa
/// data ("" = rimuovi).
fn date_popup_window(
    ctx: &egui::Context,
    title: &str,
    text: &mut String,
    open: &mut bool,
    close: &mut bool,
    mut on_confirm: impl FnMut(String),
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            let le =
                ui.add(egui::TextEdit::singleline(text).desired_width(220.0).font(cell_font()));
            let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || entered {
                    on_confirm(text.clone());
                    *close = true;
                }
                if ui.button("Rimuovi").clicked() {
                    on_confirm(String::new());
                    *close = true;
                }
                if ui.button("Annulla").clicked() {
                    *close = true;
                }
            });
        });
}

/// Finestra con campo numerico (ore max) + OK / Default / Annulla.
/// `on_confirm` riceve le ore (u32); il pulsante "Default" conferma `default_hours`,
/// che azzera l'override/limite (vedi `Worker::set_max_hours` / `set_week_override`).
fn hours_popup_window(
    ctx: &egui::Context,
    title: &str,
    text: &mut String,
    open: &mut bool,
    close: &mut bool,
    mut on_confirm: impl FnMut(u32),
    default_hours: u32,
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            let le =
                ui.add(egui::TextEdit::singleline(text).desired_width(120.0).font(cell_font()));
            let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || entered {
                    if let Ok(hours) = text.trim().parse::<u32>() {
                        on_confirm(hours);
                    }
                    *close = true;
                }
                if ui.button("Default").clicked() {
                    on_confirm(default_hours);
                    *close = true;
                }
                if ui.button("Annulla").clicked() {
                    *close = true;
                }
            });
        });
}

/// Worker visibili nel footer: esclude `hide_in_footer` e (se attivo) quelli fuori filtro.
fn footer_workers(app: &App, filter: &Filter) -> Vec<(crate::workers_utils::worker::WorkerId, String)> {
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| !app.workers.is_hidden_in_footer(*id))
        .filter(|(_, name)| worker_shown(filter, name))
        .collect()
}

fn footer(ui: &mut egui::Ui, app: &App, state: &mut UiState) {
    let filter = state.worker_filter.clone();
    let workers = footer_workers(app, &filter);
    let weeks = weeks_vec(app);
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // striscia gialla in testa al footer
    let full_w = ui.available_width();
    alloc_strip(ui, full_w, DEV_BORDER, START_STOP);

    // 1 riga header + una riga per worker o per dev (il più alto dei due)
    let n_devs = app.devs.list().len();
    let footer_h = (workers.len().max(n_devs) as f32 + 1.0) * ROW_H;

    // Footer sinistro: stessa riserva da 300px della griglia (SidePanel).
    egui::SidePanel::left("ftr_left")
        .exact_width(LEFT_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(BG_DARK))
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (lrect, _) = ui.allocate_exact_size(Vec2::new(LEFT_W, footer_h), Sense::hover());
            draw_left_footer(ui, lrect, app, state, &workers);
        });

    // Footer destro — scroll orizzontale sincronizzato con header/griglia.
    egui::ScrollArea::horizontal()
        .id_salt("footer_scroll")
        .scroll_offset(Vec2::new(state.scroll_x, 0.0))
        .enable_scrolling(false)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let content_w = weeks.len() as f32 * COL_W;
            let (rrect, _) =
                ui.allocate_exact_size(Vec2::new(content_w, footer_h), Sense::hover());
            draw_right_footer(ui, rrect, app, state, &workers, &weeks);
        });
}

fn draw_left_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    workers: &[(crate::workers_utils::worker::WorkerId, String)],
) {
    let total_x = rect.left() + DEV_NAME_W; // 90
    let dev_section_w = DEV_NAME_W + 60.0; // 150
    let wk_x = rect.left() + dev_section_w + 5.0;
    let year = state.selected_year;
    let cat = state.selected_category;

    // ── Riga header (row 0) ──
    // intestazione totale-anno sopra la colonna totali dev
    let th = Rect::from_min_size(egui::pos2(total_x, rect.top()), Vec2::new(60.0, ROW_H));
    let yhdr = if year == 0 { "Tot".to_string() } else { year.to_string() };
    ui.painter().text(th.center(), Align2::CENTER_CENTER, yhdr, mono(FONT_SIZE - 3.0), TEXT_DIM);
    // filtro effort sopra la sezione worker
    let labels = ["Tutti", "Nulli", "≥40"];
    let fbw = 46.0;
    for (i, lab) in labels.iter().enumerate() {
        let x = wk_x + i as f32 * (fbw + 2.0);
        let br = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(fbw, ROW_H));
        let active = state.effort_filter_mode == i as i32;
        if ui.put(br, egui::SelectableLabel::new(active, *lab)).clicked() {
            state.effort_filter_mode = i as i32;
        }
    }

    // ── Sezione dev: nome + totale-anno ──
    for (di, (dev, dname)) in app.devs.list().iter().enumerate() {
        let y = rect.top() + (di as f32 + 1.0) * ROW_H;
        let color = dev_color(app, *dev);
        let tcol = dev_text_color(app, *dev);
        let nrect = Rect::from_min_size(egui::pos2(rect.left(), y), Vec2::new(DEV_NAME_W, ROW_H));
        ui.painter().rect_filled(nrect, 0.0, color);
        ui.painter().text(nrect.center(), Align2::CENTER_CENTER, dname, cell_font(), tcol);
        let trect = Rect::from_min_size(egui::pos2(total_x, y), Vec2::new(60.0, ROW_H));
        let ttxt = if year == 0 {
            "—".to_string()
        } else {
            dev_year_total(app, *dev, year, cat).to_string()
        };
        ui.painter().text(trect.center(), Align2::CENTER_CENTER, ttxt, cell_font(), TEXT_WHITE);
    }

    // ── Sezione worker: nomi (allineati col sovra a destra) ──
    // Click sul nome → popup "Ore max" (limite settimanale globale del worker).
    for (idx, (wid, name)) in workers.iter().enumerate() {
        let y = rect.top() + (idx as f32 + 1.0) * ROW_H;
        let cell = Rect::from_min_size(egui::pos2(wk_x, y), Vec2::new(COL_W, ROW_H));
        if idx % 2 == 0 {
            ui.painter().rect_filled(cell, 0.0, BETWEEN_PROJECTS);
        }
        if app.workers.get_max_hours(*wid) < DEFAULT_MAX_HOURS {
            draw_corner_triangle_left(ui, cell);
        }
        ui.painter()
            .text(cell.center(), Align2::CENTER_CENTER, name, cell_font(), TEXT_WHITE);

        let resp = ui
            .interact(cell, ui.id().with(("wmax", wid.0)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            let cur = app.workers.get_max_hours(*wid);
            state.popup = Some(Popup::WorkerMax {
                worker: *wid,
                name: name.clone(),
                text: cur.to_string(),
            });
        }
    }
}

fn draw_right_footer(
    ui: &egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    workers: &[(crate::workers_utils::worker::WorkerId, String)],
    weeks: &[i32],
) {
    let lightgreen = Color32::from_rgb(0x90, 0xEE, 0x90);
    for (ci, w) in weeks.iter().enumerate() {
        let x = rect.left() + ci as f32 * COL_W;

        // tinta settimana corrente su tutta la colonna
        if *w == state.this_week {
            let col = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, rect.height()));
            ui.painter().rect_filled(col, 0.0, THIS_WEEK.gamma_multiply(0.18));
        }

        // header: data settimana
        let hdr = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, ROW_H));
        let date = primo_giorno_settimana_corrente(&days_to_local(*w)).format("%y-%m-%d").to_string();
        ui.painter().text(hdr.center(), Align2::CENTER_CENTER, date, cell_font(), TEXT_DIM);

        // celle sovra per worker
        for (idx, (wid, name)) in workers.iter().enumerate() {
            let y = rect.top() + (idx as f32 + 1.0) * ROW_H;
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(COL_W, ROW_H));

            let bg = if idx % 2 == 0 { BETWEEN_PROJECTS } else { Color32::BLACK };
            ui.painter().rect_filled(cell, 0.0, bg);

            let value = app.sovra.get(&(WeekId(*w as usize), *wid)).map_or(0, |e| e.0 as i32);
            let eff_max = app.workers.get_effective_max_hours(*wid, *w as usize) as i32;
            let global_max = app.workers.get_max_hours(*wid) as i32;

            let shown = state.effort_filter_mode == 0
                || (state.effort_filter_mode == 1 && value == 0)
                || (state.effort_filter_mode == 2 && value >= 40);
            if !shown {
                continue;
            }

            // triangolo se override settimana attivo
            if eff_max != global_max {
                draw_note_triangle(ui, cell);
            }

            let color = if value > eff_max {
                Color32::RED
            } else if value == 0 {
                Color32::YELLOW
            } else {
                lightgreen
            };
            let txt = if eff_max != global_max {
                format!("{} | {}", value, eff_max)
            } else {
                value.to_string()
            };
            ui.painter().text(cell.center(), Align2::CENTER_CENTER, txt, cell_font(), color);

            // Click sulla cella → popup override ore max per quella settimana.
            let resp = ui
                .interact(cell, ui.id().with(("wkmax", wid.0, *w)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                state.popup = Some(Popup::WorkerWeekMax {
                    worker: *wid,
                    name: name.clone(),
                    week: *w as usize,
                    text: eff_max.to_string(),
                });
            }
        }
    }
}

/// Triangolo arancio nell'angolo in alto a SINISTRA (ore ridotte).
fn draw_corner_triangle_left(ui: &egui::Ui, cell: Rect) {
    let tl = cell.left_top();
    let pts = vec![
        egui::pos2(tl.x, tl.y),
        egui::pos2(tl.x + 10.0, tl.y),
        egui::pos2(tl.x, tl.y + 10.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(pts, EFFORT_ORANGE, Stroke::NONE));
}

// ── Griglia (colonna destra) ────────────────────────────────────────────────

// Righe occupate dal nome progetto (campo multiriga).
const NAME_ROWS: usize = 2;
// Altezza minima di un progetto: spazio per le righe info a sinistra
// (tripletta + categoria + nome[NAME_ROWS] + inizio + fine).
const INFO_MIN_H: f32 = (4.0 + NAME_ROWS as f32) * ROW_H;

/// Layout condiviso da colonna sinistra e griglia: stessi progetti, stessi dev,
/// stesse altezze. Garantisce che le due colonne non possano divergere.
struct ProjLayout {
    proj: ProjectId,
    name: String,
    devs: Vec<(DevId, usize)>, // (dev, max_rows)
    proj_h: f32,
}

fn project_layout(app: &App, filter: &Filter, compact: bool) -> Vec<ProjLayout> {
    // in compatta l'info è solo tripletta + nome
    let info_min = if compact { (1.0 + NAME_ROWS as f32) * ROW_H } else { INFO_MIN_H };
    let mut out = Vec::new();
    for (proj_id, name) in app.projects.list() {
        if !app.projects.get_enable(&proj_id).0 {
            continue;
        }
        let devs: Vec<(DevId, usize)> = app
            .projects
            .list_devs(proj_id)
            .into_iter()
            .filter_map(|d| filtered_dev_max_rows(app, proj_id, d, filter).map(|m| (d, m)))
            .collect();
        // con filtro attivo, nascondi i progetti senza dev corrispondenti
        if filter.is_some() && devs.is_empty() {
            continue;
        }
        let sum_devs: f32 = devs.iter().map(|(_, m)| dev_block_height(*m, compact)).sum();
        let proj_h = sum_devs.max(info_min);
        out.push(ProjLayout { proj: proj_id, name, devs, proj_h });
    }
    out
}

fn total_content_h(layout: &[ProjLayout]) -> f32 {
    DEV_BORDER + layout.iter().map(|p| p.proj_h + DEV_BORDER).sum::<f32>()
}

/// Striscia orizzontale piena (bordo dev / separatore progetto), disegnata a y assoluta.
fn paint_hstrip(ui: &egui::Ui, left: f32, w: f32, y: f32, color: Color32) {
    ui.painter()
        .rect_filled(Rect::from_min_size(egui::pos2(left, y), Vec2::new(w, DEV_BORDER)), 0.0, color);
}

/// Etichetta data (sempre visibile) sopra la colonna inizio/fine di un progetto
/// in vista compatta. Sfondo colorato come la colonna, testo centrato sulla colonna.
fn draw_compact_date_marker(
    ui: &egui::Ui,
    left: f32,
    cw: f32,
    weeks: &[i32],
    week: i32,
    top_y: f32,
    bg: Color32,
) {
    if week < 0 {
        return;
    }
    let Some(ci) = weeks.iter().position(|w| *w == week) else {
        return;
    };
    let cx = left + ci as f32 * cw + cw / 2.0;
    let date = primo_giorno_settimana_corrente(&days_to_local(week)).format("%y-%m-%d").to_string();
    let galley = ui.painter().layout_no_wrap(date, mono(9.0), TEXT_WHITE);
    let pos = egui::pos2(cx - galley.size().x / 2.0, top_y + 1.0);
    let bgrect = Rect::from_min_size(pos, galley.size()).expand(1.5);
    ui.painter().rect_filled(bgrect, 2.0, bg);
    ui.painter().galley(pos, galley, TEXT_WHITE);
}

fn grid(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>, filter: &Filter) {
    let compact = state.compact_mode;
    let cw = col_w(compact);
    let weeks = weeks_vec(app);
    let content_w = weeks.len() as f32 * cw;
    let layout = project_layout(app, filter, compact);
    let total_h = total_content_h(&layout);

    // Un'unica allocazione: tutto il resto è disegno a coordinate assolute.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, total_h), Sense::hover());
    let left = rect.left();
    let mut y = rect.top();

    paint_hstrip(ui, left, content_w, y, START_STOP);
    y += DEV_BORDER;

    for p in &layout {
        let proj_start =
            app.projects.get_project_start_week(p.proj).map(|w| w.0 as i32).unwrap_or(-1);
        let deadline =
            app.projects.get_project_end_week(p.proj).map(|w| w.0 as i32).unwrap_or(-1);
        let proj_top = y;
        let mut dy = y;

        for (dev_id, max_rows) in &p.devs {
            let color = dev_color(app, *dev_id);
            // i bordi dev sono trasparenti in compatta
            let border = if compact { BG_DARK } else { color };
            paint_hstrip(ui, left, content_w, dy, border);
            dy += DEV_BORDER;
            let inner_h = if compact { ROW_H } else { (*max_rows as f32 + 1.0) * ROW_H };
            let block = Rect::from_min_size(egui::pos2(left, dy), Vec2::new(content_w, inner_h));
            draw_dev_cells(
                ui, block, app, state, actions, p.proj, *dev_id, *max_rows, &weeks, proj_start,
                deadline, filter, compact, cw,
            );
            dy += inner_h;
            paint_hstrip(ui, left, content_w, dy, border);
            dy += DEV_BORDER;
        }

        // in compatta: etichetta data sopra le colonne inizio (azzurra) e fine (verde)
        if compact {
            draw_compact_date_marker(ui, left, cw, &weeks, proj_start, proj_top, START_BG);
            draw_compact_date_marker(ui, left, cw, &weeks, deadline, proj_top, DEADLINE_BG);
        }

        y = proj_top + p.proj_h;
        paint_hstrip(ui, left, content_w, y, BETWEEN_PROJECTS);
        y += DEV_BORDER;
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
    filter: &Filter,
    compact: bool,
    cw: f32,
) {
    let planned = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| sd.planned_effort().0 as i32)
        .unwrap_or(0);
    let hide_effort = app.projects.get_single_dev(proj, dev).map(|sd| sd.get_hide_effort()).unwrap_or(false);

    // range di attività del dev (per le barre della vista compatta)
    let (act_start, act_end) = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| {
            let ws = sd.get_weeks();
            (
                ws.iter().map(|w| w.0 as i32).min().unwrap_or(-1),
                ws.iter().map(|w| w.0 as i32).max().unwrap_or(-1),
            )
        })
        .unwrap_or((-1, -1));
    let dcolor = dev_color(app, dev);

    let mut running = 0i32;
    for (ci, w) in weeks.iter().enumerate() {
        let x = rect.left() + ci as f32 * cw;
        let before_start = proj_start >= 0 && *w < proj_start;
        let after_deadline = deadline >= 0 && *w > deadline;
        let is_deadline = deadline >= 0 && *w == deadline;

        // colonna settimana: bg deadline/start
        let col_rect = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cw, rect.height()));
        if is_deadline {
            ui.painter().rect_filled(col_rect, 0.0, DEADLINE_BG);
        } else if proj_start >= 0 && *w == proj_start {
            ui.painter().rect_filled(col_rect, 0.0, START_BG);
        }
        if !compact && *w == state.this_week {
            ui.painter().rect_filled(col_rect, 0.0, THIS_WEEK.gamma_multiply(0.18));
        }

        let week_total = app
            .projects
            .get_single_dev(proj, dev)
            .map(|sd| sd.get_effort_by_week(WeekId(*w as usize)).0 as i32)
            .unwrap_or(0);

        // ── Vista compatta: una barra (altezza ∝ effort) per settimana attiva ──
        if compact {
            if !before_start && !after_deadline {
                let in_activity = act_start >= 0 && *w >= act_start && *w <= act_end;
                if in_activity {
                    let ratio = (week_total as f32 / 40.0).min(1.0);
                    let bar_h = if week_total == 0 { 1.0 } else { (ROW_H * ratio).max(1.0) };
                    let bar = Rect::from_min_size(
                        egui::pos2(x, rect.bottom() - bar_h),
                        Vec2::new(cw, bar_h),
                    );
                    ui.painter().rect_filled(bar, 0.0, dcolor);
                    // tooltip con data + effort della settimana
                    let date = primo_giorno_settimana_corrente(&days_to_local(*w))
                        .format("%y-%m-%d")
                        .to_string();
                    ui.interact(col_rect, egui::Id::new(("cbar", proj.0, dev.0, *w)), Sense::hover())
                        .on_hover_text(format!("{}  ·  {}h", date, week_total));
                }
            }
            continue;
        }

        running += week_total;

        // riga cumulativo (sola lettura)
        if !before_start && !after_deadline {
            let cum_rect = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cw, ROW_H));
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
        let slots = gather_slots(app, proj, dev, *w, max_rows, filter);
        for (row, (text, note)) in slots.iter().enumerate() {
            let y = rect.top() + (row as f32 + 1.0) * ROW_H;
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(cw, ROW_H));

            let is_editing = state
                .editing
                .as_ref()
                .map(|e| e.proj == proj && e.dev == dev && e.week == *w && e.row == row)
                .unwrap_or(false);

            if is_editing {
                let mut ed = state.editing.take().unwrap();
                let id = egui::Id::new(("celledit", proj.0, dev.0, *w, row));
                let resp = ui.interact(cell, id, Sense::click());
                if ed.just_opened {
                    resp.request_focus();
                    ed.just_opened = false;
                }
                let has_focus = resp.has_focus();

                let mut commit = false;
                let mut cancel = false;
                if has_focus {
                    ed.had_focus = true;
                    let events = ui.input(|i| i.events.clone());
                    for ev in events {
                        match ev {
                            egui::Event::Text(t) => {
                                for ch in t.chars() {
                                    if ch != '\n' && ch != '\t' {
                                        ed.typed.push(ch);
                                    }
                                }
                                recompute_completion(app, &mut ed.typed, &mut ed.buf);
                            }
                            egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                                // Cancellazione letterale: niente re-completamento, altrimenti
                                // un nome completo verrebbe ri-completato e non si potrebbe svuotare.
                                ed.typed.pop();
                                ed.buf = ed.typed.clone();
                            }
                            egui::Event::Key { key: egui::Key::Delete, pressed: true, .. } => {
                                ed.typed.clear();
                                ed.buf.clear();
                            }
                            egui::Event::Key { key: egui::Key::Enter, pressed: true, .. }
                            | egui::Event::Key { key: egui::Key::Tab, pressed: true, .. } => {
                                commit = true;
                            }
                            egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                                cancel = true;
                            }
                            // Copia / Taglia / Incolla (Cmd su macOS, Ctrl altrove)
                            egui::Event::Key { key: egui::Key::C, pressed: true, modifiers, .. }
                                if modifiers.command || modifiers.ctrl =>
                            {
                                state.copied_text = ed.buf.clone();
                                state.copied_note = note.clone();
                            }
                            egui::Event::Key { key: egui::Key::X, pressed: true, modifiers, .. }
                                if modifiers.command || modifiers.ctrl =>
                            {
                                state.copied_text = ed.buf.clone();
                                state.copied_note = note.clone();
                                ed.buf.clear();
                                ed.typed.clear();
                            }
                            egui::Event::Key { key: egui::Key::V, pressed: true, modifiers, .. }
                                if modifiers.command || modifiers.ctrl =>
                            {
                                ed.buf = state.copied_text.clone();
                                ed.typed = state.copied_text.clone();
                                ed.paste_note = Some(state.copied_note.clone());
                            }
                            _ => {}
                        }
                    }
                } else if ed.had_focus {
                    commit = true; // focus perso → conferma
                }

                // disegno cella in editing + caret
                ui.painter().rect_filled(cell, 0.0, SEL_BG);
                ui.painter().rect_stroke(
                    cell,
                    0.0,
                    Stroke::new(1.0, FOCUS_BORDER),
                    egui::StrokeKind::Inside,
                );
                let trect = ui.painter().text(
                    egui::pos2(cell.left() + 3.0, cell.center().y),
                    Align2::LEFT_CENTER,
                    &ed.buf,
                    person_font(),
                    TEXT_WHITE,
                );
                let caret_x = (trect.right() + 1.0).min(cell.right() - 1.0);
                ui.painter().vline(
                    caret_x,
                    (cell.top() + 2.0)..=(cell.bottom() - 2.0),
                    Stroke::new(1.0, TEXT_WHITE),
                );

                if cancel {
                    // scarta
                } else if commit {
                    actions.push(commit_editing(app, &ed));
                } else {
                    state.editing = Some(ed);
                }
            } else {
                let resp = ui.interact(
                    cell,
                    egui::Id::new(("cell", proj.0, dev.0, *w, row)),
                    Sense::click(),
                );
                let hovered = resp.hovered();
                let clicked = resp.clicked();
                let secondary = resp.secondary_clicked();

                if hovered {
                    ui.painter().rect_filled(cell, 0.0, SEL_BG.gamma_multiply(0.4));
                }
                if !note.is_empty() {
                    draw_note_triangle(ui, cell);
                }
                if !text.is_empty() {
                    let wname = text.split('|').next().unwrap_or("").trim();
                    let wid = app.workers.get_id_by_name(wname);
                    let hidden = wid.map_or(false, |id| app.workers.is_hidden_in_footer(id));
                    let sovra = wid
                        .map(|id| app.sovra.get(&(WeekId(*w as usize), id)).map_or(0, |e| e.0 as i32))
                        .unwrap_or(0);
                    let max_h = wid
                        .map(|id| app.workers.get_effective_max_hours(id, *w as usize) as i32)
                        .unwrap_or(DEFAULT_MAX_HOURS as i32);
                    // worker nascosti nel footer → grigi, a prescindere dall'effort
                    let color = if hidden {
                        Color32::from_gray(0x80)
                    } else if sovra > max_h {
                        Color32::RED
                    } else {
                        TEXT_WHITE
                    };
                    paint_person_cell(ui, cell, text, color);
                }
                if !note.is_empty() {
                    resp.on_hover_text(note.clone());
                }

                if clicked {
                    if let Some(old) = state.editing.take() {
                        actions.push(commit_editing(app, &old));
                    }
                    let orig_worker = text.split('|').next().unwrap_or("").trim().to_string();
                    state.editing = Some(Editing {
                        proj,
                        dev,
                        week: *w,
                        row,
                        buf: text.clone(),
                        typed: text.clone(),
                        just_opened: true,
                        had_focus: false,
                        paste_note: None,
                        orig_worker,
                        orig_note: note.clone(),
                    });
                }
                if secondary && !text.is_empty() {
                    let wname = text.split('|').next().unwrap_or("").trim().to_string();
                    state.note_editor = Some(NoteEditing {
                        target: NoteTarget::Effort { proj, dev, week: *w, worker: wname },
                        text: note.clone(),
                    });
                }
            }
        }
    }
}

/// Tutte le voci (worker, effort, nota) della settimana, ignorando il filtro.
fn full_week_entries(app: &App, proj: ProjectId, dev: DevId, week: i32) -> Vec<(String, usize, String)> {
    app.projects
        .get_single_dev(proj, dev)
        .and_then(|sd| sd.get_all(WeekId(week as usize)))
        .map(|sew| {
            sew.worker_id
                .iter()
                .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                .map(|(wid, se)| {
                    (app.workers.get_name_by_id(*wid).to_string(), se.get_effort().0, se.get_note())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Commit mirato: rimpiazza solo il worker della cella editata, preservando gli
/// altri worker della settimana (compresi quelli nascosti da un filtro attivo).
fn commit_editing(app: &App, ed: &Editing) -> Action {
    let mut entries = full_week_entries(app, ed.proj, ed.dev, ed.week);
    // rimuovi il worker originale della cella (sarà sostituito o svuotato)
    if !ed.orig_worker.is_empty() {
        entries.retain(|(n, _, _)| n != &ed.orig_worker);
    }
    // interpreta il nuovo contenuto "worker|effort"
    let parts: Vec<&str> = ed.buf.split('|').collect();
    if parts.len() == 2 {
        let wname = parts[0].trim().to_string();
        if !wname.is_empty() {
            let eff = parts[1].trim().parse::<usize>().unwrap_or(0);
            let note = ed
                .paste_note
                .clone()
                .unwrap_or_else(|| if wname == ed.orig_worker { ed.orig_note.clone() } else { String::new() });
            entries.retain(|(n, _, _)| n != &wname);
            entries.push((wname, eff, note));
        }
    }
    let rows: Vec<String> = entries.iter().map(|(n, e, _)| format!("{}|{}", n, e)).collect();
    let notes: Vec<String> = entries.iter().map(|(_, _, n)| n.clone()).collect();
    Action::CommitCell {
        proj: ed.proj,
        dev: ed.dev,
        week: WeekId(ed.week as usize),
        rows,
        notes,
    }
}

/// Disegna il testo "nome|effort" centrato nella cella, troncando il NOME con "…"
/// quando non entra ma mantenendo sempre visibile la parte "|effort".
fn paint_person_cell(ui: &egui::Ui, cell: Rect, text: &str, color: Color32) {
    let font = person_font();
    let avail = COL_W - 4.0;
    let measure =
        |s: &str| ui.painter().layout_no_wrap(s.to_string(), font.clone(), color).size().x;

    let display = if measure(text) <= avail {
        text.to_string()
    } else if let Some(p) = text.rfind('|') {
        let name = &text[..p];
        let suffix = &text[p..]; // "|effort"
        let mut keep = name.chars().count();
        loop {
            if keep == 0 {
                break format!("…{}", suffix);
            }
            let trimmed: String = name.chars().take(keep).collect();
            let cand = format!("{}…{}", trimmed, suffix);
            if measure(&cand) <= avail {
                break cand;
            }
            keep -= 1;
        }
    } else {
        let mut keep = text.chars().count();
        loop {
            if keep == 0 {
                break String::from("…");
            }
            let cand: String = text.chars().take(keep).chain(std::iter::once('…')).collect();
            if measure(&cand) <= avail {
                break cand;
            }
            keep -= 1;
        }
    };

    let galley = ui.painter().layout_no_wrap(display, font, color);
    let pos = egui::pos2(
        cell.center().x - galley.size().x / 2.0,
        cell.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, color);
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

fn left_column(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>, filter: &Filter) {
    let compact = state.compact_mode;
    let layout = project_layout(app, filter, compact);
    let total_h = total_content_h(&layout);

    // Stessa altezza totale e stessa allocazione singola della griglia.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(LEFT_W, total_h), Sense::hover());
    let left = rect.left();
    let mut y = rect.top();

    paint_hstrip(ui, left, LEFT_W, y, START_STOP);
    y += DEV_BORDER;

    for p in &layout {
        let proj_rect = Rect::from_min_size(egui::pos2(left, y), Vec2::new(LEFT_W, p.proj_h));
        draw_project_info(ui, proj_rect, app, state, actions, p.proj, &p.name, compact);
        draw_left_dev_strip(ui, proj_rect, p.proj, state);
        draw_left_devs(ui, proj_rect, app, state, actions, p.proj, &p.devs, compact);

        y += p.proj_h;
        paint_hstrip(ui, left, LEFT_W, y, BETWEEN_PROJECTS);
        y += DEV_BORDER;
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
    compact: bool,
) {
    let x = rect.left();
    let w = LEFT_INFO_W;
    let mut y = rect.top();

    // tripletta (right-click per modificare)
    let trip = app.projects.get_tripletta(proj);
    let trip_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    if trip.is_empty() {
        ui.painter().text(trip_rect.center(), Align2::CENTER_CENTER, "—", cell_font(), TEXT_FAINT);
    } else {
        ui.painter().text(trip_rect.center(), Align2::CENTER_CENTER, &trip, cell_font(), EFFORT_ORANGE);
    }
    let tr = ui.interact(trip_rect, egui::Id::new(("trip", proj.0)), Sense::click());
    if tr.secondary_clicked() {
        state.popup = Some(Popup::Tripletta { proj, text: trip.clone() });
    }
    tr.on_hover_text("Tasto destro: modifica tripletta");

    // Pulsanti sposta su/giù all'estrema destra della riga tripletta.
    let btn_w = 16.0;
    let up_rect = Rect::from_min_size(egui::pos2(x + w - 2.0 * btn_w, y), Vec2::new(btn_w, ROW_H));
    let down_rect = Rect::from_min_size(egui::pos2(x + w - btn_w, y), Vec2::new(btn_w, ROW_H));
    if ui
        .put(up_rect, egui::Button::new("▲").frame(false).small())
        .on_hover_text("Sposta progetto su")
        .clicked()
    {
        actions.push(Action::MoveProjectUp { proj });
    }
    if ui
        .put(down_rect, egui::Button::new("▼").frame(false).small())
        .on_hover_text("Sposta progetto giù")
        .clicked()
    {
        actions.push(Action::MoveProjectDown { proj });
    }
    y += ROW_H;

    // categoria (click per scegliere) — nascosta in compatta
    if !compact {
        let cat = app
            .projects
            .get_category(proj)
            .and_then(|c| app.categories.get_name(c))
            .unwrap_or("—")
            .to_string();
        let cat_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
        let cat_col = if cat == "—" { TEXT_FAINT } else { CAT_BLUE };
        ui.painter().text(cat_rect.center(), Align2::CENTER_CENTER, cat, cell_font(), cat_col);
        let cr = ui.interact(cat_rect, egui::Id::new(("cat", proj.0)), Sense::click());
        if cr.clicked() {
            state.popup = Some(Popup::Category { proj });
        }
        y += ROW_H;
    }

    // nome progetto (editabile, multiriga)
    let name_top = y;
    let name_h = NAME_ROWS as f32 * ROW_H;
    let name_rect = Rect::from_min_size(egui::pos2(x, name_top), Vec2::new(w, name_h));
    let buf = state.name_buffers.entry(proj.0).or_insert_with(|| proj_name.to_string());
    let resp = ui.put(
        name_rect,
        egui::TextEdit::multiline(buf)
            .font(cell_font())
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_rows(NAME_ROWS)
            .desired_width(w),
    );
    if resp.lost_focus() {
        actions.push(Action::SetProjectName { proj, name: buf.clone() });
    } else if !resp.has_focus() && *buf != proj_name {
        *buf = proj_name.to_string();
    }
    // inizio/fine vanno sotto il fondo REALE del campo (che può crescere su più righe)
    y = resp.rect.bottom().max(name_top + name_h);

    // inizio/fine nascosti in compatta
    if compact {
        return;
    }

    // inizio (right-click per modificare)
    let start_date = app
        .projects
        .get_project_start_week(proj)
        .map(|wk| primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32)).format("%y-%m-%d").to_string())
        .unwrap_or_default();
    let start_txt = if start_date.is_empty() {
        "Inizio: —".to_string()
    } else {
        format!("Inizio: {}", start_date)
    };
    let sr = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(sr.center(), Align2::CENTER_CENTER, start_txt, mono(FONT_SIZE - 2.0), TEXT_DIM);
    let srr = ui.interact(sr, egui::Id::new(("start", proj.0)), Sense::click());
    if srr.secondary_clicked() {
        state.popup = Some(Popup::Start { proj, text: start_date });
    }
    srr.on_hover_text("Tasto destro: modifica data inizio");
    y += ROW_H;

    // fine (right-click per modificare)
    let end_date = app
        .projects
        .get_project_end_week(proj)
        .map(|wk| primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32)).format("%y-%m-%d").to_string())
        .unwrap_or_default();
    let end_txt = if end_date.is_empty() {
        "Fine: —".to_string()
    } else {
        format!("Fine: {}", end_date)
    };
    let er = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(er.center(), Align2::CENTER_CENTER, end_txt, mono(FONT_SIZE - 2.0), TEXT_DIM);
    let err = ui.interact(er, egui::Id::new(("end", proj.0)), Sense::click());
    if err.secondary_clicked() {
        state.popup = Some(Popup::End { proj, text: end_date });
    }
    err.on_hover_text("Tasto destro: modifica deadline");
}

fn draw_left_dev_strip(ui: &mut egui::Ui, rect: Rect, proj: ProjectId, state: &mut UiState) {
    let x = rect.left() + LEFT_INFO_W;
    let strip = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(DEV_STRIP_W, rect.height()));
    let resp = ui.interact(strip, egui::Id::new(("devstrip", proj.0)), Sense::click());
    let bg = if resp.hovered() {
        Color32::from_rgb(0x3a, 0x3a, 0x3a)
    } else {
        Color32::from_rgb(0x22, 0x22, 0x22)
    };
    ui.painter().rect_filled(strip, 0.0, bg);
    ui.painter().rect_stroke(
        strip,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(0x55, 0x55, 0x55)),
        egui::StrokeKind::Inside,
    );
    // etichetta "Dev" verticale (lettere impilate)
    for (i, ch) in "Dev".chars().enumerate() {
        let cy = strip.top() + 10.0 + i as f32 * 11.0;
        if cy > strip.bottom() {
            break;
        }
        ui.painter().text(
            egui::pos2(strip.center().x, cy),
            Align2::CENTER_CENTER,
            ch.to_string(),
            mono(FONT_SIZE - 3.0),
            TEXT_DIM,
        );
    }
    if resp.clicked() {
        state.dev_manage = Some(proj);
    }
    resp.on_hover_text("Aggiungi / rimuovi dev dal progetto");
}

#[allow(clippy::too_many_arguments)]
fn draw_left_devs(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    devs: &[(DevId, usize)],
    compact: bool,
) {
    let x0 = rect.left() + LEFT_INFO_W + DEV_STRIP_W;
    let mut y = rect.top();

    for (dev, max_rows) in devs {
        let max_rows = *max_rows;
        let color = dev_color(app, *dev);
        let tcol = dev_text_color(app, *dev);
        let block_h = dev_block_height(max_rows, compact);

        // bordo superiore
        let top_b = Rect::from_min_size(egui::pos2(x0, y), Vec2::new(LEFT_DEV_W, DEV_BORDER));
        ui.painter().rect_filled(top_b, 0.0, color);
        let inner_y = y + DEV_BORDER;
        let inner_h = if compact { ROW_H } else { (max_rows as f32 + 1.0) * ROW_H };

        // cella nome dev (90px, doppio click = add row)
        let name_rect = Rect::from_min_size(egui::pos2(x0, inner_y), Vec2::new(DEV_NAME_W, inner_h));
        ui.painter().rect_filled(name_rect, 0.0, color);
        ui.painter().text(name_rect.center(), Align2::CENTER_CENTER, dev_name(app, *dev), cell_font(), tcol);
        let dev_note = app.projects.get_single_dev(proj, *dev).map(|sd| sd.get_note()).unwrap_or_default();
        let hide_effort = app.projects.get_single_dev(proj, *dev).map(|sd| sd.get_hide_effort()).unwrap_or(false);
        if !dev_note.is_empty() {
            draw_note_triangle(ui, name_rect);
        }
        let nresp = ui.interact(name_rect, egui::Id::new(("devname", proj.0, dev.0)), Sense::click());
        if nresp.double_clicked() {
            actions.push(Action::AddRow { proj, dev: *dev });
        }
        nresp.context_menu(|ui| {
            if ui.button("Aggiungi riga").clicked() {
                actions.push(Action::AddRow { proj, dev: *dev });
                ui.close_menu();
            }
            if ui.button("Elimina riga").clicked() {
                actions.push(Action::DelRow { proj, dev: *dev });
                ui.close_menu();
            }
            if ui.button("Nota Dev…").clicked() {
                state.note_editor = Some(NoteEditing {
                    target: NoteTarget::Dev { proj, dev: *dev },
                    text: dev_note.clone(),
                });
                ui.close_menu();
            }
            ui.separator();
            let label = if hide_effort { "Visualizza effort" } else { "Nascondi effort" };
            if ui.button(label).clicked() {
                actions.push(Action::SetDevHideEffort { proj, dev: *dev, hide: !hide_effort });
                ui.close_menu();
            }
        });
        if !dev_note.is_empty() {
            nresp.on_hover_text(dev_note);
        }

        // area effort/remains (65px) — i campi sono nascosti in compatta
        let eff_x = x0 + DEV_NAME_W;
        let eff_area = Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, inner_h));
        ui.painter().rect_filled(eff_area, 0.0, color);

        if !compact {
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
        }

        // bordo inferiore
        let bot_b = Rect::from_min_size(egui::pos2(x0, inner_y + inner_h), Vec2::new(LEFT_DEV_W, DEV_BORDER));
        ui.painter().rect_filled(bot_b, 0.0, color);

        y += block_h;
    }
}
