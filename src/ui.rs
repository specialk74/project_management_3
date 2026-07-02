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
use crate::milestones::MilestoneId;
use crate::project_utils::project::{Enable, ProjectId};
use crate::single_dev_utils::single_dev::WeekId;
use crate::single_effort_utils::sinlge_effort::Effort;
use crate::ui_style::*;
use crate::workers_utils::worker::{WORKER_ID_ZERO, WeekStatus, WorkerId};

// ── Stato di sola UI ────────────────────────────────────────────────────────

struct Editing {
    proj: ProjectId,
    dev: DevId,
    week: i32,
    row: usize,
    buf: String,   // testo mostrato (dopo autocomplete)
    typed: String, // ciò che l'utente ha realmente digitato
    just_opened: bool,
    had_focus: bool,
    paste_note: Option<String>, // nota incollata (Ctrl+V) da applicare al commit
    orig_worker: String,        // worker presente nella cella all'apertura (per commit mirato)
    orig_note: String,          // nota presente nella cella all'apertura
}

/// Bersaglio del note editor.
enum NoteTarget {
    Effort {
        proj: ProjectId,
        dev: DevId,
        week: i32,
        worker: String,
    },
    Dev {
        proj: ProjectId,
        dev: DevId,
    },
    /// Nota legata a un worker per una specifica settimana (footer destro).
    WorkerWeek {
        worker: WorkerId,
        week: usize,
        name: String,
    },
}

struct NoteEditing {
    target: NoteTarget,
    text: String,
}

/// Popup di modifica (tripletta / inizio / fine / categoria) aperti da right/left-click.
enum Popup {
    Tripletta {
        proj: ProjectId,
        text: String,
    },
    Start {
        proj: ProjectId,
        text: String,
    },
    End {
        proj: ProjectId,
        text: String,
    },
    Category {
        proj: ProjectId,
    },
    /// Ore max globali di un worker (click sul nome nel footer sinistro).
    WorkerMax {
        worker: WorkerId,
        name: String,
        text: String,
    },
    /// Override ore max di un worker per una specifica settimana (click sulla cella sovra).
    WorkerWeekMax {
        worker: WorkerId,
        name: String,
        week: usize,
        text: String,
    },
    /// Limite ore settimana per TUTTI i worker (click sulla data della settimana nell'header).
    BulkWeekMax {
        week: usize,
        date: String,
        text: String,
    },
}

/// Conflitto in sospeso: il file su disco è cambiato mentre c'erano modifiche
/// locali che toccano gli stessi progetti/dev. L'utente sceglie via dialog.
struct PendingReload {
    /// Stato del file su disco (versione del collega).
    theirs: App,
    /// Merge con la mia versione vincente sui punti in conflitto.
    merged: App,
    /// Descrizioni dei conflitti da mostrare.
    conflicts: Vec<String>,
}

#[derive(Default)]
pub struct UiState {
    current_file: String,
    changed: bool,
    // ── Rilevamento modifiche esterne al file condiviso (es. OneDrive) ──
    // Snapshot RON dell'ultimo stato noto su disco (antenato comune per il merge).
    base_ron: String,
    // mtime dell'ultimo stato di disco osservato; None finché non inizializzato.
    file_mtime: Option<std::time::SystemTime>,
    // conflitto in sospeso in attesa di scelta utente.
    pending_reload: Option<PendingReload>,
    // messaggio (persistente) mostrato in toolbar dopo un aggiornamento esterno
    // applicato in automatico; resta finché non si salva o non lo si chiude.
    external_notice: Option<String>,
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
    show_closed_filter: bool,
    // conferma di uscita con modifiche non salvate
    show_exit_confirm: bool,
    allow_close: bool,
    // evita la chiusura "click-fuori" nello stesso frame in cui la finestra si apre
    worker_filter_just_opened: bool,
    project_filter_just_opened: bool,
    closed_filter_just_opened: bool,
    // true finché la finestra popup/nota è già stata mostrata almeno un frame:
    // serve a dare il focus al campo di testo solo alla prima comparsa.
    popup_was_open: bool,
    note_editor_was_open: bool,
    // posizione (angolo in basso a sx) del pulsante "Closed ▼", per ancorare la finestra
    closed_btn_pos: egui::Pos2,
    // appunti per copia/incolla cella
    copied_text: String,
    copied_note: String,
    // 0 = tutti gli effort, 1 = solo nulli, 2 = solo >= 40
    effort_filter_mode: i32,
    compact_mode: bool,
    // resa in bianco/nero (senza colori)
    bw_mode: bool,
    // selettori per i totali-anno per dev nel footer
    selected_year: i32,                    // 0 = nessuno
    selected_category: Option<CategoryId>, // None = tutte
    // input toolbar
    new_worker: String,
    new_dev: String,
    new_category: String,
    new_milestone: String,
    // finestra di gestione milestone (elenco, colore, elimina)
    show_milestone_manager: bool,
    milestone_manager_just_opened: bool,
    // buffer di editing per nomi progetto ed effort dev
    name_buffers: HashMap<usize, String>,
    effort_buffers: HashMap<(usize, usize), String>,
}

// Scelta dell'utente nella finestra di conferma uscita.
enum ExitChoice {
    Save,
    Discard,
    Cancel,
}

// ── Azioni differite (applicate dopo il rendering) ──────────────────────────

enum Action {
    Save,
    Open,
    NewProject,
    AddWorker(String),
    AddDev(String),
    AddCategory(String),
    SetProjectName {
        proj: ProjectId,
        name: String,
    },
    SetDevEffort {
        proj: ProjectId,
        dev: DevId,
        effort: usize,
    },
    AddRow {
        proj: ProjectId,
        dev: DevId,
    },
    CommitCell {
        proj: ProjectId,
        dev: DevId,
        week: WeekId,
        rows: Vec<String>,
        notes: Vec<String>,
    },
    SetNote {
        proj: ProjectId,
        dev: DevId,
        week: WeekId,
        worker: String,
        note: String,
    },
    SetDevNote {
        proj: ProjectId,
        dev: DevId,
        note: String,
    },
    SetProjectTripletta {
        proj: ProjectId,
        text: String,
    },
    SetProjectStartWeek {
        proj: ProjectId,
        date: String,
    },
    SetProjectEndWeek {
        proj: ProjectId,
        date: String,
    },
    SetProjectCategory {
        proj: ProjectId,
        cat: Option<CategoryId>,
    },
    DelRow {
        proj: ProjectId,
        dev: DevId,
    },
    SetDevHideEffort {
        proj: ProjectId,
        dev: DevId,
        hide: bool,
    },
    AddDevToProject {
        proj: ProjectId,
        dev: DevId,
        add: bool,
    },
    SetProjectEnabled {
        proj: ProjectId,
        enabled: bool,
    },
    SetAllProjectsEnabled {
        enabled: bool,
    },
    SetProjectClosed {
        proj: ProjectId,
        closed: bool,
    },
    SetWorkerMaxHours {
        worker: WorkerId,
        hours: u32,
    },
    SetWorkerWeekOverride {
        worker: WorkerId,
        week: usize,
        hours: u32,
    },
    SetWorkerWeekNote {
        worker: WorkerId,
        week: usize,
        note: String,
    },
    SetWorkerWeekStatus {
        worker: WorkerId,
        week: usize,
        status: Option<WeekStatus>,
    },
    SetBulkWeekLimit {
        week: usize,
        hours: u32,
    },
    MoveProjectUp {
        proj: ProjectId,
    },
    MoveProjectDown {
        proj: ProjectId,
    },
    ExportPdf,
    CreateMilestone(String),
    SetMilestoneColor {
        milestone: MilestoneId,
        color: u32,
    },
    DeleteMilestone {
        milestone: MilestoneId,
    },
    AddProjectMilestone {
        proj: ProjectId,
        milestone: MilestoneId,
        week: WeekId,
    },
    RemoveProjectMilestone {
        proj: ProjectId,
        milestone: MilestoneId,
    },
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
    let mono = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let prop = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
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
        // L'offset X tiene conto delle colonne di confine d'anno (più strette).
        let cols = columns_vec(&app);
        let col_pos = cols
            .iter()
            .position(|c| matches!(c, Col::Week(w) if *w == this_week));
        let pending_scroll_x = match col_pos {
            Some(idx) if idx > 0 => {
                const INITIAL_WINDOW_WIDTH: f32 = 1024.0;
                let visible_width = INITIAL_WINDOW_WIDTH - LEFT_W;
                let col_center = col_x_offset(&cols, idx, COL_W) + COL_W / 2.0;
                Some((col_center - visible_width / 2.0).max(0.0))
            }
            _ => None,
        };

        // Stato iniziale per il rilevamento di modifiche esterne al file.
        let base_ron = app.to_ron_string();
        let file_mtime = file_mtime_of(&current_file);

        Self {
            app,
            ui: UiState {
                current_file,
                base_ron,
                file_mtime,
                this_week,
                selected_year: today.year(),
                scroll_x: pending_scroll_x.unwrap_or(0.0),
                pending_scroll_x,
                ..Default::default()
            },
        }
    }
}

/// mtime del file, se leggibile.
fn file_mtime_of(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Testo di notifica con orario corrente (per gli aggiornamenti esterni).
fn notice_now(msg: &str) -> String {
    format!("🔄 {msg} ({})", chrono::Local::now().format("%H:%M"))
}

impl eframe::App for PjmApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions: Vec<Action> = Vec::new();
        {
            let app = &self.app;
            let state = &mut self.ui;

            // Resa in scala di grigi: impostata una volta per frame, prima di disegnare.
            set_bw_mode(state.bw_mode);

            // Scorciatoie globali (Cmd su macOS, Ctrl altrove). Calcolate in anticipo
            // per non trattenere un borrow di `ui` durante i pannelli.
            let (key_s, key_f, shift) = ui.ctx().input(|i| {
                let cmd = i.modifiers.command || i.modifiers.ctrl;
                (
                    cmd && i.key_pressed(egui::Key::S),
                    cmd && i.key_pressed(egui::Key::F),
                    i.modifiers.shift,
                )
            });
            if key_s {
                actions.push(Action::Save);
            }
            if key_f {
                if shift {
                    state.worker_filter = Some(HashSet::new()); // deseleziona tutti
                } else {
                    state.show_worker_filter = true;
                    state.worker_filter_just_opened = true;
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
                    .show_inside(ui, |ui| footer(ui, app, state, &mut actions));
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
            milestone_manager_window(ui.ctx(), app, state, &mut actions);
            closed_filter_window(ui.ctx(), app, state, &mut actions);
        }

        for a in actions {
            self.apply(a);
        }

        let ctx = ui.ctx().clone();
        self.check_external_change(&ctx);
        self.conflict_window(&ctx);
        self.handle_exit(&ctx);
    }
}

/// Converte una data `yy-mm-dd` nella settimana corrispondente. Stringa vuota
/// (o non valida) ⇒ `None`, cioè rimuove il limite di inizio/fine del progetto.
fn week_from_date_str(date: &str) -> Option<WeekId> {
    if date.trim().is_empty() {
        return None;
    }
    parse_date_str(date).map(|d| WeekId(d as usize))
}

/// Interpreta una stringa `yy-mm-dd` / `yyyy-mm-dd` come `jiff::civil::Date`
/// (il giorno esatto, non allineato al lunedì). Usata per inizializzare il
/// calendario `DatePickerButton`. `None` se vuota o non valida.
fn date_str_to_jiff(s: &str) -> Option<jiff::civil::Date> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: i8 = parts[1].parse().ok()?;
    let day: i8 = parts[2].parse().ok()?;
    let year = if year < 100 { year + 2000 } else { year };
    jiff::civil::Date::new(year as i16, month, day).ok()
}

/// Formatta un `jiff::civil::Date` come `yyyy-mm-dd` (accettato da `parse_date_str`,
/// che poi allinea al lunedì della settimana).
fn jiff_to_date_str(d: &jiff::civil::Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

/// Data odierna come `jiff::civil::Date`, valore di default del calendario.
fn today_jiff() -> jiff::civil::Date {
    let t = chrono::Utc::now().date_naive();
    jiff::civil::Date::new(t.year() as i16, t.month() as i8, t.day() as i8)
        .unwrap_or_else(|_| jiff::civil::Date::new(2000, 1, 1).unwrap())
}

impl PjmApp {
    /// Intercetta la richiesta di chiusura finestra: se ci sono modifiche non
    /// salvate, annulla la chiusura e mostra una finestra di conferma con le
    /// opzioni "Salva ed esci", "Esci senza salvare", "Annulla".
    fn handle_exit(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.ui.changed && !self.ui.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.ui.show_exit_confirm = true;
        }

        if !self.ui.show_exit_confirm {
            return;
        }

        let mut choice: Option<ExitChoice> = None;
        egui::Window::new("Uscita")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Ci sono modifiche non salvate. Vuoi salvarle prima di uscire?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Salva ed esci").clicked() {
                        choice = Some(ExitChoice::Save);
                    }
                    if ui.button("Esci senza salvare").clicked() {
                        choice = Some(ExitChoice::Discard);
                    }
                    if ui.button("Annulla").clicked() {
                        choice = Some(ExitChoice::Cancel);
                    }
                });
            });

        match choice {
            Some(ExitChoice::Save) => {
                self.app.save(&self.ui.current_file);
                self.ui.changed = false;
                self.ui.show_exit_confirm = false;
                self.ui.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(ExitChoice::Discard) => {
                self.ui.show_exit_confirm = false;
                self.ui.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(ExitChoice::Cancel) => {
                self.ui.show_exit_confirm = false;
            }
            None => {}
        }
    }

    fn mark_changed(&mut self) {
        self.app.recompute_week_range();
        self.app.compute_sovra();
        self.ui.changed = true;
    }

    /// Aggiorna lo snapshot "base" e l'mtime dopo che lo stato è tornato
    /// coerente col disco (salvataggio o apertura).
    fn sync_baseline(&mut self) {
        self.ui.base_ron = self.app.to_ron_string();
        self.ui.file_mtime = file_mtime_of(&self.ui.current_file);
    }

    /// Adotta integralmente lo stato del disco (ricarica automatica: nessuna
    /// modifica locale da preservare).
    fn adopt_disk(&mut self, mut theirs: App, mtime: std::time::SystemTime) {
        theirs.compute_sovra();
        self.ui.base_ron = theirs.to_ron_string();
        self.app = theirs;
        self.ui.file_mtime = Some(mtime);
        self.ui.changed = false;
        self.ui.name_buffers.clear();
        self.ui.effort_buffers.clear();
        self.ui.editing = None;
        self.ui.external_notice = Some(notice_now("File changed..."));
    }

    /// Poll periodico dell'mtime del file condiviso: se un collega lo ha
    /// modificato, ricarica (nessuna modifica locale) o fonde a 3 vie
    /// (modifiche locali). Vedi modulo `sync_merge`.
    fn check_external_change(&mut self, ctx: &egui::Context) {
        // Continua a fare polling anche senza interazione utente.
        ctx.request_repaint_after(std::time::Duration::from_secs(2));

        // Conflitto già in attesa di scelta → non toccare nulla.
        if self.ui.pending_reload.is_some() {
            return;
        }

        let path = self.ui.current_file.clone();
        let Some(disk_mtime) = file_mtime_of(&path) else {
            return;
        };
        match self.ui.file_mtime {
            None => {
                self.ui.file_mtime = Some(disk_mtime);
                return;
            }
            Some(known) if known == disk_mtime => return,
            _ => {}
        }

        // Il file è cambiato: leggilo. Se lettura/parse falliscono (scrittura
        // parziale di OneDrive) non aggiorno l'mtime → riprovo al prossimo giro.
        let Ok(theirs) = App::load(&path) else {
            return;
        };

        // Nessuna modifica locale → ricarica automatica.
        if !self.ui.changed {
            self.adopt_disk(theirs, disk_mtime);
            return;
        }

        // Modifiche locali → merge a 3 vie con l'antenato comune.
        let Ok(base) = App::from_ron_str(&self.ui.base_ron) else {
            self.adopt_disk(theirs, disk_mtime);
            return;
        };
        let dev_names: HashMap<DevId, String> = self.app.devs.list().into_iter().collect();
        let name_fn = |d: DevId| {
            dev_names
                .get(&d)
                .cloned()
                .unwrap_or_else(|| "?".to_string())
        };
        let outcome = crate::sync_merge::merge(&base, &self.app, theirs.clone(), &name_fn);

        if outcome.conflicts.is_empty() {
            // Fusione pulita: applico e sposto la base sul disco. Restano le mie
            // modifiche non salvate → `changed` resta true.
            self.app = outcome.app;
            self.ui.base_ron = theirs.to_ron_string();
            self.ui.file_mtime = Some(disk_mtime);
            self.ui.external_notice = Some(notice_now("Uniti i cambiamenti di un collega"));
        } else {
            self.ui.file_mtime = Some(disk_mtime);
            self.ui.pending_reload = Some(PendingReload {
                theirs,
                merged: outcome.app,
                conflicts: outcome.conflicts,
            });
        }
    }

    /// Finestra di scelta quando ci sono conflitti tra le mie modifiche e quelle
    /// del collega.
    fn conflict_window(&mut self, ctx: &egui::Context) {
        let conflicts = match &self.ui.pending_reload {
            Some(p) => p.conflicts.clone(),
            None => return,
        };
        let mut choice: Option<bool> = None; // true = tieni le mie, false = scarta
        egui::Window::new("File modificato da un collega")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    "Il file è stato modificato esternamente e ci sono conflitti con le tue \
                     modifiche non salvate:",
                );
                ui.add_space(4.0);
                for c in &conflicts {
                    ui.label(format!("•  {c}"));
                }
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Mantieni le mie").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("Ricarica (scarta le mie)").clicked() {
                        choice = Some(false);
                    }
                });
            });

        match choice {
            Some(true) => {
                if let Some(p) = self.ui.pending_reload.take() {
                    self.app = p.merged; // sovra già ricalcolata nel merge
                    self.ui.base_ron = p.theirs.to_ron_string();
                    self.ui.changed = true;
                    self.ui.external_notice = Some(notice_now(
                        "Modifiche del collega unite (conflitti: tenute le tue)",
                    ));
                }
            }
            Some(false) => {
                if let Some(p) = self.ui.pending_reload.take() {
                    let mut theirs = p.theirs;
                    theirs.compute_sovra();
                    self.ui.base_ron = theirs.to_ron_string();
                    self.app = theirs;
                    self.ui.changed = false;
                    self.ui.name_buffers.clear();
                    self.ui.effort_buffers.clear();
                    self.ui.editing = None;
                    self.ui.external_notice =
                        Some(notice_now("File Changed (your change discarded)"));
                }
            }
            None => {}
        }
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::Save => {
                self.app.save(&self.ui.current_file);
                self.ui.changed = false;
                self.sync_baseline();
                self.ui.external_notice = None;
            }
            Action::Open => {
                if let Some(path_buf) = rfd::FileDialog::new()
                    .add_filter("RON files", &["ron"])
                    .pick_file()
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
                            self.sync_baseline();
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
                    self.app
                        .projects
                        .add_effort(proj, dev, week, WORKER_ID_ZERO, Effort(0));
                    self.mark_changed();
                }
            }
            Action::CommitCell {
                proj,
                dev,
                week,
                rows,
                notes,
            } => {
                self.app.projects.reset_effort(proj, dev, week);
                for (text, note) in rows.iter().zip(notes.iter()) {
                    let parts: Vec<&str> = text.split('|').collect();
                    if parts.len() != 2 {
                        continue;
                    }
                    if let Some(wid) = self.app.workers.get_id_by_name(parts[0].trim()) {
                        let e = parts[1].trim().parse::<usize>().unwrap_or(0);
                        self.app
                            .projects
                            .add_effort(proj, dev, week, wid, Effort(e));
                        if !note.is_empty() {
                            self.app.projects.set_note(proj, dev, week, wid, note);
                        }
                    }
                }
                self.mark_changed();
            }
            Action::SetNote {
                proj,
                dev,
                week,
                worker,
                note,
            } => {
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
                self.app
                    .projects
                    .set_project_start_week(proj, week_from_date_str(&date));
                self.mark_changed();
            }
            Action::SetProjectEndWeek { proj, date } => {
                self.app
                    .projects
                    .set_project_end_week(proj, week_from_date_str(&date));
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
                // Filtro di sola visualizzazione: non persistito, non marca come
                // modificato (come il filtro per worker).
                self.app.projects.set_enable(proj, Enable(enabled));
            }
            Action::SetAllProjectsEnabled { enabled } => {
                let ids: Vec<_> = self.app.projects.list().iter().map(|(id, _)| *id).collect();
                for id in ids {
                    // i progetti chiusi restano non-enabled
                    if self.app.projects.is_closed(id) {
                        continue;
                    }
                    self.app.projects.set_enable(id, Enable(enabled));
                }
                // Filtro di sola visualizzazione: non marca come modificato.
            }
            Action::SetProjectClosed { proj, closed } => {
                self.app.projects.set_closed(proj, closed);
                self.mark_changed();
            }
            Action::SetWorkerMaxHours { worker, hours } => {
                self.app.workers.set_max_hours(worker, hours);
                self.mark_changed();
            }
            Action::SetWorkerWeekOverride {
                worker,
                week,
                hours,
            } => {
                self.app.workers.set_week_override(worker, week, hours);
                // Override a zero ore ⇒ settimana di ferie: attiva lo stato "Ferie".
                if hours == 0 {
                    self.app
                        .workers
                        .set_week_status(worker, week, Some(WeekStatus::Ferie));
                }
                self.mark_changed();
            }
            Action::SetWorkerWeekNote { worker, week, note } => {
                self.app.workers.set_week_note(worker, week, &note);
                self.mark_changed();
            }
            Action::SetWorkerWeekStatus {
                worker,
                week,
                status,
            } => {
                self.app.workers.set_week_status(worker, week, status);
                self.mark_changed();
            }
            Action::SetBulkWeekLimit { week, hours } => {
                self.app.set_bulk_week_limit(week, hours);
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
            Action::ExportPdf => match crate::pdf_export::build_pdf(&self.app) {
                None => eprintln!("Nessun progetto visibile con inizio e fine: PDF non creato."),
                Some(bytes) => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("progetti.pdf")
                        .save_file()
                    {
                        let p = path.to_string_lossy().to_string();
                        if let Err(e) = std::fs::write(&p, bytes) {
                            eprintln!("Errore scrittura PDF '{p}': {e}");
                        }
                    }
                }
            },
            Action::CreateMilestone(name) => {
                self.app.milestones.add(&name);
                self.mark_changed();
            }
            Action::SetMilestoneColor { milestone, color } => {
                self.app.milestones.set_color(milestone, color);
                self.mark_changed();
            }
            Action::DeleteMilestone { milestone } => {
                self.app.milestones.del(milestone);
                // toglie ogni collocazione dai progetti
                self.app.projects.purge_milestone(milestone);
                self.mark_changed();
            }
            Action::AddProjectMilestone {
                proj,
                milestone,
                week,
            } => {
                self.app
                    .projects
                    .add_project_milestone(proj, milestone, week);
                self.mark_changed();
            }
            Action::RemoveProjectMilestone { proj, milestone } => {
                self.app.projects.remove_project_milestone(proj, milestone);
                self.mark_changed();
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
        len = first
            .chars()
            .zip(s.chars())
            .take_while(|(a, b)| a == b)
            .count()
            .min(len);
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
        if pipe {
            format!("{}|", matches[0])
        } else {
            matches[0].clone()
        }
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
    (app.start_week.0..=app.end_week.0)
        .step_by(7)
        .map(|w| w as i32)
        .collect()
}

/// Una colonna della griglia: una settimana reale, oppure una colonna di
/// confine d'anno (sola lettura, sfondo giallo canarino) inserita tra l'ultima
/// settimana di un anno e la prima del successivo. `YearEnd(y)` rappresenta il
/// confine tra l'anno `y` e l'anno `y+1`.
#[derive(Clone, Copy, PartialEq)]
enum Col {
    Week(i32),
    YearEnd(i32),
}

/// Larghezza (px) della colonna di confine d'anno: più stretta di una settimana,
/// quel tanto che basta per il titolo "Effort residuo" e i valori.
const BOUNDARY_W: f32 = 64.0;

/// Larghezza di una colonna: le settimane usano `cw`, il confine è più stretto.
fn col_width(c: &Col, cw: f32) -> f32 {
    match c {
        Col::YearEnd(_) => BOUNDARY_W,
        Col::Week(_) => cw,
    }
}

/// Larghezza totale dell'asse colonne.
fn cols_width(cols: &[Col], cw: f32) -> f32 {
    cols.iter().map(|c| col_width(c, cw)).sum()
}

/// Offset X (dal bordo sinistro) dell'inizio della colonna all'indice `idx`.
fn col_x_offset(cols: &[Col], idx: usize, cw: f32) -> f32 {
    cols[..idx].iter().map(|c| col_width(c, cw)).sum()
}

/// Asse colonne condiviso da header, griglia e footer: le settimane di
/// `weeks_vec` con, dopo ogni transizione d'anno, una colonna di confine.
fn columns_vec(app: &App) -> Vec<Col> {
    let weeks = weeks_vec(app);
    let mut cols = Vec::with_capacity(weeks.len() + 2);
    for (i, w) in weeks.iter().enumerate() {
        cols.push(Col::Week(*w));
        if let Some(next) = weeks.get(i + 1) {
            let y0 = days_to_local(*w).year();
            let y1 = days_to_local(*next).year();
            // di norma un solo confine; il ciclo copre eventuali salti d'anno.
            for y in y0..y1 {
                cols.push(Col::YearEnd(y));
            }
        }
    }
    cols
}

/// Ore mancanti al dev per finire il progetto, al confine di fine `year_ending`.
/// `Some(_)` solo se il progetto è "a cavallo": il suo range inizio→fine
/// attraversa il confine (inizio nell'anno `<= year_ending`, fine `>= year_ending+1`).
/// Valore = effort pianificato del dev − effort assegnati nelle settimane di `year_ending`.
fn dev_missing_at_year_end(
    app: &App,
    proj: ProjectId,
    dev: DevId,
    year_ending: i32,
) -> Option<i32> {
    let start = app.projects.get_project_start_week(proj)?;
    let end = app.projects.get_project_end_week(proj)?;
    let start_y = days_to_local(start.0 as i32).year();
    let end_y = days_to_local(end.0 as i32).year();
    if !(start_y <= year_ending && end_y >= year_ending + 1) {
        return None;
    }
    let sd = app.projects.get_single_dev(proj, dev)?;
    let planned = sd.planned_effort().0 as i32;
    let assigned: i32 = sd
        .get_weeks()
        .iter()
        .filter(|w| days_to_local(w.0 as i32).year() == year_ending)
        .map(|w| sd.get_effort_by_week(*w).0 as i32)
        .sum();
    Some(planned - assigned)
}

/// Totale ore che restano al progetto per essere completato oltre il confine di
/// fine `year_ending`: somma dei residui per-dev (`dev_missing_at_year_end`),
/// scartando quelli negativi (dev già in pari o in eccesso). `None` se il progetto
/// non è a cavallo del confine (nessun dev produce un residuo).
fn project_missing_at_year_end(app: &App, proj: ProjectId, year_ending: i32) -> Option<i32> {
    let mut crosses = false;
    let mut total = 0i32;
    for dev in app.projects.list_devs(proj) {
        if let Some(missing) = dev_missing_at_year_end(app, proj, dev, year_ending) {
            crosses = true;
            if missing > 0 {
                total += missing;
            }
        }
    }
    crosses.then_some(total)
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
/// `projects` è la lista già pronta (evita di ricostruirla e riordinarla a ogni dev).
fn dev_year_total(
    app: &App,
    projects: &[(ProjectId, String)],
    dev: DevId,
    year: i32,
    cat: Option<CategoryId>,
) -> i32 {
    if year == 0 {
        return 0;
    }
    projects
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
    let inner = if compact {
        ROW_H
    } else {
        (max_rows as f32 + 1.0) * ROW_H
    };
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

fn toolbar(ui: &mut egui::Ui, _app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if ui.button("+ Progetto").clicked() {
            actions.push(Action::NewProject);
        }
        if ui.button("Progetti ▼").clicked() {
            state.show_project_filter = !state.show_project_filter;
            state.project_filter_just_opened = state.show_project_filter;
        }
        let filter_on = state.worker_filter.is_some();
        let wbtn = egui::Button::new("Workers ▼");
        let wbtn = if filter_on {
            wbtn.fill(g(Color32::from_rgb(0x2a, 0x50, 0x80)))
        } else {
            wbtn
        };
        if ui.add(wbtn).clicked() {
            state.show_worker_filter = !state.show_worker_filter;
            state.worker_filter_just_opened = state.show_worker_filter;
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

        let me = ui.add(
            egui::TextEdit::singleline(&mut state.new_milestone)
                .hint_text("Nome milestone…")
                .desired_width(120.0),
        );
        if (me.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.button("+ Milestone").clicked()
        {
            if !state.new_milestone.is_empty() {
                actions.push(Action::CreateMilestone(std::mem::take(
                    &mut state.new_milestone,
                )));
            }
        }
        if ui.button("Milestone ▼").clicked() {
            state.show_milestone_manager = !state.show_milestone_manager;
            state.milestone_manager_just_opened = state.show_milestone_manager;
        }

        ui.separator();
        if ui.button("Salva").clicked() {
            actions.push(Action::Save);
        }
        if ui.button("Apri").clicked() {
            actions.push(Action::Open);
        }
        if ui.button("PDF").clicked() {
            actions.push(Action::ExportPdf);
        }
        let compact_label = if state.compact_mode {
            "Vista normale"
        } else {
            "Vista compatta"
        };
        if ui.button(compact_label).clicked() {
            state.compact_mode = !state.compact_mode;
        }
        let bw_label = if state.bw_mode {
            "Colori"
        } else {
            "Bianco/Nero"
        };
        if ui.button(bw_label).clicked() {
            state.bw_mode = !state.bw_mode;
        }
        let closed_btn = ui.button("Closed ▼");
        state.closed_btn_pos = closed_btn.rect.left_bottom();
        if closed_btn.clicked() {
            state.show_closed_filter = !state.show_closed_filter;
            state.closed_filter_just_opened = state.show_closed_filter;
        }

        // I selettori Anno e Categoria sono stati spostati nel footer sinistro
        // (vedi `draw_left_footer`): l'anno sopra i totali-anno per dev, la
        // categoria sopra la colonna dei nomi dev.

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let col = if state.changed {
                g(EFFORT_ORANGE)
            } else {
                TEXT_DIM
            };
            let file_name = std::path::Path::new(&state.current_file)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| state.current_file.clone());
            let label = format!("{}{}", file_name, if state.changed { " (*)" } else { "" });
            ui.colored_label(col, label)
                .on_hover_text(&state.current_file);
            ui.separator();
            ui.weak(format!("v{}", env!("CARGO_PKG_VERSION")));

            // Notifica di aggiornamento esterno applicato in automatico.
            if let Some(msg) = state.external_notice.clone() {
                ui.separator();
                if ui.small_button("✕").on_hover_text("Nascondi").clicked() {
                    state.external_notice = None;
                }
                ui.colored_label(
                    g(egui::Color32::from_rgb(90, 200, 250)),
                    egui::RichText::new(msg).strong(),
                );
            }
        });
    });
}

// ── Header (date settimane) ─────────────────────────────────────────────────

fn header(ui: &mut egui::Ui, app: &App, state: &mut UiState) {
    let cols = columns_vec(app);
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
            let content_w = cols_width(&cols, cw);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, ROW_H), Sense::hover());
            let mut x = rect.left();
            for c in cols.iter() {
                let colw = col_width(c, cw);
                let cell = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(colw, ROW_H));
                x += colw;
                let w = match c {
                    Col::YearEnd(_) => {
                        // colonna di confine: sfondo canarino + titolo "Effort residuo"
                        ui.painter().rect_filled(cell, 0.0, g(CANARY));
                        draw_boundary_title(ui, cell);
                        continue;
                    }
                    Col::Week(w) => *w,
                };
                if w == state.this_week {
                    ui.painter()
                        .rect_filled(cell, 0.0, g(THIS_WEEK).gamma_multiply(0.5));
                }
                let txt = primo_giorno_settimana_corrente(&days_to_local(w))
                    .format("%y-%m-%d")
                    .to_string();
                // Click sulla data → popup limite ore settimana per TUTTI i worker.
                let mut resp = ui
                    .interact(cell, egui::Id::new(("hdr", w)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if compact {
                    // colonne troppo strette per il testo → la data nel tooltip
                    resp = resp.on_hover_text(txt.clone());
                } else {
                    ui.painter().text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        &txt,
                        cell_font(),
                        TEXT_WHITE,
                    );
                }
                if resp.clicked() {
                    state.popup = Some(Popup::BulkWeekMax {
                        week: w as usize,
                        date: txt,
                        text: DEFAULT_MAX_HOURS.to_string(),
                    });
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
            let mut sa = egui::ScrollArea::both()
                .id_salt("grid_scroll")
                .auto_shrink([false, false]);
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
    if state.note_editor.is_none() {
        return;
    }
    // Prima comparsa: dà il focus al campo nota.
    let just_opened = !state.note_editor_was_open;
    state.note_editor_was_open = true;
    let ne = state.note_editor.as_mut().unwrap();
    let title = match &ne.target {
        NoteTarget::Effort { worker, .. } => format!("Nota: {}", worker),
        NoteTarget::Dev { .. } => "Nota Dev".to_string(),
        NoteTarget::WorkerWeek { name, .. } => format!("Nota: {}", name),
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
            let te = ui.add(
                egui::TextEdit::multiline(&mut ne.text)
                    .desired_rows(4)
                    .desired_width(320.0)
                    .font(cell_font()),
            );
            if just_opened {
                te.request_focus();
            }
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
            NoteTarget::Effort {
                proj,
                dev,
                week,
                worker,
            } => Action::SetNote {
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
            NoteTarget::WorkerWeek { worker, week, .. } => Action::SetWorkerWeekNote {
                worker: *worker,
                week: *week,
                note: ne.text.clone(),
            },
        };
        actions.push(action);
    }
    if save || cancel || !open {
        state.note_editor = None;
        state.note_editor_was_open = false;
    }
}

// ── Gestione dev del progetto (+Dev / −Dev) ─────────────────────────────────

fn dev_manage_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
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
                let txt_col = if is_in {
                    dev_text_color(app, dev)
                } else {
                    Color32::from_gray(0x88)
                };
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
                            actions.push(Action::AddDevToProject {
                                proj,
                                dev,
                                add: false,
                            });
                        }
                    } else {
                        actions.push(Action::AddDevToProject {
                            proj,
                            dev,
                            add: true,
                        });
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
                    actions.push(Action::AddDevToProject {
                        proj,
                        dev,
                        add: false,
                    });
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

/// Larghezza (in punti) del titolo della finestra, più lo spazio per il
/// pulsante di chiusura, così da poter dare al contenuto una larghezza minima
/// pari almeno a quella della barra del titolo.
fn title_width(ui: &egui::Ui, title: &str) -> f32 {
    let font_id = egui::TextStyle::Heading.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font_id, egui::Color32::WHITE);
    galley.size().x + 36.0
}

fn project_filter_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_project_filter {
        return;
    }
    // (id, name, enable, label-mostrata) ordinati alfabeticamente per etichetta
    let mut projects: Vec<(ProjectId, String, Enable, String)> = app
        .projects
        .list_full()
        .into_iter()
        // i progetti chiusi non compaiono tra i progetti "attivi"
        .filter(|(id, _, _)| !app.projects.is_closed(*id))
        .map(|(id, name, en)| {
            let trip = app.projects.get_tripletta(id);
            let label = if trip.is_empty() { name.clone() } else { trip };
            (id, name, en, label)
        })
        .collect();
    projects.sort_by(|a, b| a.3.to_lowercase().cmp(&b.3.to_lowercase()));
    let mut open = true;

    let just_opened = state.project_filter_just_opened;
    state.project_filter_just_opened = false;

    let resp = egui::Window::new("Progetti")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(90.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            let min_w = title_width(ui, "Progetti");
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.set_min_width(min_w);
                    let mut all_on =
                        !projects.is_empty() && projects.iter().all(|(_, _, en, _)| en.0);
                    if ui.checkbox(&mut all_on, "Select All").changed() {
                        actions.push(Action::SetAllProjectsEnabled { enabled: all_on });
                    }
                    for (id, _name, en, label) in &projects {
                        let mut on = en.0;
                        if ui.checkbox(&mut on, label).changed() {
                            actions.push(Action::SetProjectEnabled {
                                proj: *id,
                                enabled: on,
                            });
                        }
                    }
                });
        });

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_project_filter = false;
    }
}

// ── Gestione milestone (Milestone ▼) ────────────────────────────────────────

/// Colore `u32` 0xRRGGBB → `Color32` grezzo (senza filtro B/N, per l'editing).
fn u32_to_color(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

fn color_to_u32(c: Color32) -> u32 {
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32)
}

fn milestone_manager_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_milestone_manager {
        return;
    }
    let milestones = app.milestones.list();
    let mut open = true;
    let just_opened = state.milestone_manager_just_opened;
    state.milestone_manager_just_opened = false;

    let resp = egui::Window::new("Milestone")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(120.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(title_width(ui, "Milestone").max(220.0));
            if milestones.is_empty() {
                ui.label("Nessuna milestone. Creane una dalla toolbar.");
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for (id, name, color) in &milestones {
                        ui.horizontal(|ui| {
                            let mut col = u32_to_color(*color);
                            if ui.color_edit_button_srgba(&mut col).changed() {
                                actions.push(Action::SetMilestoneColor {
                                    milestone: *id,
                                    color: color_to_u32(col),
                                });
                            }
                            ui.label(name);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("🗑").on_hover_text("Elimina milestone").clicked()
                                    {
                                        actions.push(Action::DeleteMilestone { milestone: *id });
                                    }
                                },
                            );
                        });
                    }
                });
        });

    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    // niente auto-chiusura al click-fuori: il color picker apre popup esterni.
    if !open {
        state.show_milestone_manager = false;
    }
    let _ = (just_opened, clicked_outside);
}

// ── Filtro progetti chiusi (Closed ▼) ───────────────────────────────────────

fn closed_filter_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_closed_filter {
        return;
    }
    // tutti i progetti, ordinati alfabeticamente per etichetta; il check indica "chiuso"
    let mut projects: Vec<(ProjectId, String)> = app
        .projects
        .list_full()
        .into_iter()
        .map(|(id, name, _)| {
            let trip = app.projects.get_tripletta(id);
            let label = if trip.is_empty() { name } else { trip };
            (id, label)
        })
        .collect();
    projects.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    let mut open = true;
    let just_opened = state.closed_filter_just_opened;
    state.closed_filter_just_opened = false;

    let mut window = egui::Window::new("Closed")
        .collapsible(false)
        .resizable(false)
        .open(&mut open);
    // all'apertura ancora la finestra sotto il pulsante "Closed ▼";
    // dopo resta trascinabile dall'utente
    window = if just_opened {
        window.current_pos(state.closed_btn_pos)
    } else {
        window.default_pos(state.closed_btn_pos)
    };
    let resp = window.show(ctx, |ui| {
        let min_w = title_width(ui, "Closed");
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                ui.set_min_width(min_w);
                for (id, label) in &projects {
                    let mut closed = app.projects.is_closed(*id);
                    if ui.checkbox(&mut closed, label).changed() {
                        actions.push(Action::SetProjectClosed { proj: *id, closed });
                    }
                }
            });
    });

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_closed_filter = false;
    }
}

// ── Filtro worker (Ctrl+F / Workers ▼) ──────────────────────────────────────

fn worker_filter_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_worker_filter {
        return;
    }
    // i worker nascosti nel footer non compaiono nemmeno nel filtro
    let all: Vec<String> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| !app.workers.is_hidden_in_footer(*id))
        .map(|(_, n)| n)
        .collect();
    let mut open = true;
    let mut filter = state.worker_filter.clone();

    let just_opened = state.worker_filter_just_opened;
    state.worker_filter_just_opened = false;

    let resp = egui::Window::new("Workers")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(140.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            let min_w = title_width(ui, "Workers");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(min_w);
                let mut all_on = match &filter {
                    None => !all.is_empty(),
                    Some(s) => !all.is_empty() && all.iter().all(|n| s.contains(n)),
                };
                if ui.checkbox(&mut all_on, "Select All").changed() {
                    filter = if all_on { None } else { Some(HashSet::new()) };
                }
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
    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_worker_filter = false;
    }
}

// ── Popup di modifica (tripletta / inizio / fine / categoria) ───────────────

fn popup_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    if state.popup.is_none() {
        return;
    }
    // Prima comparsa della finestra: dà il focus al campo di testo.
    let just_opened = !state.popup_was_open;
    state.popup_was_open = true;
    let popup = state.popup.as_mut().unwrap();
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
                        egui::TextEdit::singleline(text)
                            .desired_width(220.0)
                            .font(cell_font()),
                    );
                    if just_opened {
                        le.request_focus();
                    }
                    let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || entered {
                            actions.push(Action::SetProjectTripletta {
                                proj: *proj,
                                text: text.clone(),
                            });
                            close = true;
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
        Popup::Start { proj, text } => {
            date_popup_window(
                ctx,
                "Data inizio (yy-mm-dd)",
                text,
                &mut open,
                &mut close,
                just_opened,
                |date| {
                    actions.push(Action::SetProjectStartWeek { proj: *proj, date });
                },
            );
        }
        Popup::End { proj, text } => {
            date_popup_window(
                ctx,
                "Data fine (yy-mm-dd)",
                text,
                &mut open,
                &mut close,
                just_opened,
                |date| {
                    actions.push(Action::SetProjectEndWeek { proj: *proj, date });
                },
            );
        }
        Popup::Category { proj } => {
            egui::Window::new("Categoria")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    if ui.button("— Nessuna").clicked() {
                        actions.push(Action::SetProjectCategory {
                            proj: *proj,
                            cat: None,
                        });
                        close = true;
                    }
                    for (id, name) in app.categories.list() {
                        if ui.button(name).clicked() {
                            actions.push(Action::SetProjectCategory {
                                proj: *proj,
                                cat: Some(id),
                            });
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
                just_opened,
                |hours| {
                    actions.push(Action::SetWorkerMaxHours {
                        worker: *worker,
                        hours,
                    });
                },
                DEFAULT_MAX_HOURS,
            );
        }
        Popup::WorkerWeekMax {
            worker,
            name,
            week,
            text,
        } => {
            // "Default" = max globale del worker (azzera l'override per la settimana).
            let global_max = app.workers.get_max_hours(*worker);
            hours_popup_window(
                ctx,
                &format!("Override settimana - {name}"),
                text,
                &mut open,
                &mut close,
                just_opened,
                |hours| {
                    actions.push(Action::SetWorkerWeekOverride {
                        worker: *worker,
                        week: *week,
                        hours,
                    });
                },
                global_max,
            );
        }
        Popup::BulkWeekMax { week, date, text } => {
            // "Default" = 40 → azzera l'override per tutti i worker su questa settimana.
            hours_popup_window(
                ctx,
                &format!("Limite settimana (tutti) - {date}"),
                text,
                &mut open,
                &mut close,
                just_opened,
                |hours| {
                    actions.push(Action::SetBulkWeekLimit { week: *week, hours });
                },
                DEFAULT_MAX_HOURS,
            );
        }
    }

    if close || !open {
        state.popup = None;
        state.popup_was_open = false;
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
    just_opened: bool,
    mut on_confirm: impl FnMut(String),
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            let _ = just_opened;
            // Selezione col mouse: il calendario è inizializzato con la data
            // corrente del campo (o con oggi, se non impostata). Quando l'utente
            // sceglie un giorno, la stringa `text` viene aggiornata.
            let mut date = date_str_to_jiff(text).unwrap_or_else(today_jiff);
            let picked = ui.add(
                egui_extras::DatePickerButton::new(&mut date)
                    .id_salt(title)
                    .format("%Y-%m-%d"),
            );
            if picked.changed() {
                *text = jiff_to_date_str(&date);
            }
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
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
    just_opened: bool,
    mut on_confirm: impl FnMut(u32),
    default_hours: u32,
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            let le = ui.add(
                egui::TextEdit::singleline(text)
                    .desired_width(120.0)
                    .font(cell_font()),
            );
            if just_opened {
                le.request_focus();
            }
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
                if ui.button("Zero").clicked() {
                    on_confirm(0);
                    *close = true;
                }
                if ui.button("Annulla").clicked() {
                    *close = true;
                }
            });
        });
}

/// Worker visibili nel footer: esclude `hide_in_footer` e (se attivo) quelli fuori filtro.
fn footer_workers(
    app: &App,
    filter: &Filter,
) -> Vec<(crate::workers_utils::worker::WorkerId, String)> {
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| !app.workers.is_hidden_in_footer(*id))
        .filter(|(_, name)| worker_shown(filter, name))
        .collect()
}

fn footer(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let filter = state.worker_filter.clone();
    let workers = footer_workers(app, &filter);
    let cols = columns_vec(app);
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // striscia gialla in testa al footer
    let full_w = ui.available_width();
    alloc_strip(ui, full_w, DEV_BORDER, g(START_STOP));

    // 1 riga header + una riga per worker o per dev (il più alto dei due)
    let n_devs = app.devs.list().len();
    // +2 righe dev: una riga vuota di separazione + la riga "Totale" sotto l'elenco dev.
    let footer_h = (workers.len().max(n_devs + 2) as f32 + 1.0) * ROW_H;

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
            let content_w = cols_width(&cols, COL_W);
            let (rrect, _) = ui.allocate_exact_size(Vec2::new(content_w, footer_h), Sense::hover());
            draw_right_footer(ui, rrect, app, state, &workers, &cols, actions);
        });
}

/// Posiziona un `ComboBox` compatto a coordinate assolute dentro il footer
/// (il footer è disegnato a mano col painter, quindi serve un'ui figlia).
fn footer_combo(
    ui: &mut egui::Ui,
    rect: Rect,
    id_salt: &str,
    selected_text: String,
    width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().button_padding = Vec2::new(3.0, 0.0);
        ui.spacing_mut().interact_size.y = ROW_H;
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected_text)
            .width(width)
            .show_ui(ui, add_contents);
    });
}

/// Ore rimanenti per il worker fino alla fine dell'anno `year`.
///
/// Il periodo segue il selettore Anno del footer: se `year` è l'anno solare
/// corrente (quello di `this_week`) si parte dalla settimana corrente, altrimenti
/// dalla prima settimana di `year`. Per ogni settimana del periodo si somma il
/// monte ore effettivo del worker (`get_effective_max_hours`), così gli override
/// con orari diversi dal default riducono (o aumentano) il totale rispetto a
/// `default × n_settimane`.
///
/// Con il filtro "Nulli" attivo (`only_null`) si contano solo le settimane in cui
/// il worker non ha ancora effort assegnato (sovra = 0): il valore diventa così la
/// somma delle ore che potrebbero ancora essergli destinate.
fn worker_remaining_hours(
    app: &App,
    wid: WorkerId,
    year: i32,
    this_week: i32,
    only_null: bool,
) -> i32 {
    let start = if year == days_to_local(this_week).year() {
        this_week
    } else {
        i32::MIN // nessun limite inferiore: tutte le settimane dell'anno
    };
    weeks_vec(app)
        .into_iter()
        .filter(|w| *w >= start && days_to_local(*w).year() == year)
        .filter(|w| {
            !only_null
                || app
                    .sovra
                    .get(&(WeekId(*w as usize), wid))
                    .map_or(0, |e| e.0)
                    == 0
        })
        .map(|w| app.workers.get_effective_max_hours(wid, w as usize) as i32)
        .sum()
}

fn draw_left_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    workers: &[(crate::workers_utils::worker::WorkerId, String)],
) {
    let total_x = rect.left() + DEV_NAME_W; // 90
    let dev_section_w = DEV_NAME_W + DEV_TOTAL_W; // 150
    let wk_x = rect.left() + dev_section_w + 5.0;
    let year = state.selected_year;
    let cat = state.selected_category;

    // ── Riga header (row 0) ──
    // Selettore categoria (scope dei totali-anno) sopra la colonna dei nomi dev.
    let cat_rect = Rect::from_min_size(rect.left_top(), Vec2::new(DEV_NAME_W, ROW_H));
    let cat_label = match cat {
        None => "Tutte".to_string(),
        Some(c) => app.categories.get_name(c).unwrap_or("?").to_string(),
    };
    footer_combo(
        ui,
        cat_rect,
        "footer_cat_combo",
        cat_label,
        DEV_NAME_W - 6.0,
        |ui| {
            ui.selectable_value(&mut state.selected_category, None, "Tutte");
            for (id, name) in app.categories.list() {
                ui.selectable_value(&mut state.selected_category, Some(id), name);
            }
        },
    );

    // Selettore anno (totali-anno per dev) sopra la colonna dei totali.
    let th = Rect::from_min_size(
        egui::pos2(total_x, rect.top()),
        Vec2::new(DEV_TOTAL_W, ROW_H),
    );
    let year_label = if year == 0 {
        "Tot".to_string()
    } else {
        year.to_string()
    };
    footer_combo(ui, th, "footer_year_combo", year_label, 54.0, |ui| {
        ui.selectable_value(&mut state.selected_year, 0, "—");
        for y in available_years(app) {
            ui.selectable_value(&mut state.selected_year, y, y.to_string());
        }
    });

    // filtro effort sopra la sezione worker
    let labels = ["Tutti", "Nulli", "≥40"];
    let fbw = 46.0;
    for (i, lab) in labels.iter().enumerate() {
        let x = wk_x + i as f32 * (fbw + 2.0);
        let br = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(fbw, ROW_H));
        let active = state.effort_filter_mode == i as i32;
        if ui
            .put(
                br,
                egui::SelectableLabel::new(active, egui::RichText::new(*lab).color(TEXT_WHITE)),
            )
            .clicked()
        {
            state.effort_filter_mode = i as i32;
        }
    }

    // ── Sezione dev: nome + totale-anno ──
    // `grand_total` si accumula qui per evitare un secondo passaggio nella riga "Totale".
    // `projects` è costruita una volta sola e condivisa da tutte le chiamate a `dev_year_total`.
    let devs = app.devs.list();
    let projects = app.projects.list();
    let mut grand_total = 0i32;
    for (di, (dev, dname)) in devs.iter().enumerate() {
        let y = rect.top() + (di as f32 + 1.0) * ROW_H;
        let color = dev_color(app, *dev);
        let tcol = dev_text_color(app, *dev);
        let nrect = Rect::from_min_size(egui::pos2(rect.left(), y), Vec2::new(DEV_NAME_W, ROW_H));
        ui.painter().rect_filled(nrect, 0.0, color);
        ui.painter().text(
            nrect.center(),
            Align2::CENTER_CENTER,
            dname,
            cell_font(),
            tcol,
        );
        let dev_total = dev_year_total(app, &projects, *dev, year, cat);
        grand_total += dev_total;
        let trect = Rect::from_min_size(egui::pos2(total_x, y), Vec2::new(DEV_TOTAL_W, ROW_H));
        let ttxt = if year == 0 {
            "—".to_string()
        } else {
            dev_total.to_string()
        };
        ui.painter().text(
            trect.center(),
            Align2::CENTER_CENTER,
            ttxt,
            cell_font(),
            TEXT_WHITE,
        );
    }

    // ── Riga "Totale": somma dei totali-anno di tutti i dev ──
    // Rispetta i selettori anno e categoria attivi nel footer.
    // Una riga vuota (+1) la separa dall'elenco dev.
    let ty = rect.top() + (devs.len() as f32 + 2.0) * ROW_H;
    let nrect = Rect::from_min_size(egui::pos2(rect.left(), ty), Vec2::new(DEV_NAME_W, ROW_H));
    ui.painter().rect_filled(nrect, 0.0, g(START_STOP));
    ui.painter().text(
        nrect.center(),
        Align2::CENTER_CENTER,
        "Totale",
        cell_font(),
        Color32::BLACK,
    );
    let trect = Rect::from_min_size(egui::pos2(total_x, ty), Vec2::new(DEV_TOTAL_W, ROW_H));
    let ttxt = if year == 0 {
        "—".to_string()
    } else {
        grand_total.to_string()
    };
    ui.painter().text(
        trect.center(),
        Align2::CENTER_CENTER,
        ttxt,
        cell_font(),
        TEXT_WHITE,
    );

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
        ui.painter().text(
            cell.center(),
            Align2::CENTER_CENTER,
            name,
            cell_font(),
            TEXT_WHITE,
        );

        // Ore rimanenti dell'anno selezionato, nello spazio a destra del nome.
        // Su "Tot" (year == 0) la cella resta vuota.
        if year != 0 {
            let rem_left = wk_x + COL_W;
            let rem_rect = Rect::from_min_size(
                egui::pos2(rem_left, y),
                Vec2::new((rect.right() - rem_left).max(0.0), ROW_H),
            );
            let remaining = worker_remaining_hours(
                app,
                *wid,
                year,
                state.this_week,
                state.effort_filter_mode == 1,
            );
            ui.painter().text(
                rem_rect.right_center() - Vec2::new(12.0, 0.0),
                Align2::RIGHT_CENTER,
                remaining.to_string(),
                cell_font(),
                g(Color32::from_rgb(0x90, 0xEE, 0x90)),
            );
        }

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
    cols: &[Col],
    actions: &mut Vec<Action>,
) {
    let lightgreen = g(Color32::from_rgb(0x90, 0xEE, 0x90));
    let mut next_x = rect.left();
    for c in cols.iter() {
        let colw = col_width(c, COL_W);
        let x = next_x;
        next_x += colw;

        // colonna di confine d'anno: sfondo canarino su tutta l'altezza, niente valori
        let w = match c {
            Col::YearEnd(_) => {
                let col =
                    Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(colw, rect.height()));
                ui.painter().rect_filled(col, 0.0, g(CANARY));
                continue;
            }
            Col::Week(w) => *w,
        };

        // tinta settimana corrente su tutta la colonna
        if w == state.this_week {
            let col =
                Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, rect.height()));
            ui.painter()
                .rect_filled(col, 0.0, g(THIS_WEEK).gamma_multiply(0.18));
        }

        // header: data settimana
        let hdr = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, ROW_H));
        let date = primo_giorno_settimana_corrente(&days_to_local(w))
            .format("%y-%m-%d")
            .to_string();
        ui.painter().text(
            hdr.center(),
            Align2::CENTER_CENTER,
            date,
            cell_font(),
            TEXT_WHITE,
        );

        // celle sovra per worker
        for (idx, (wid, name)) in workers.iter().enumerate() {
            let y = rect.top() + (idx as f32 + 1.0) * ROW_H;
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(COL_W, ROW_H));

            let bg = if idx % 2 == 0 {
                BETWEEN_PROJECTS
            } else {
                Color32::BLACK
            };
            ui.painter().rect_filled(cell, 0.0, bg);

            let value = app
                .sovra
                .get(&(WeekId(w as usize), *wid))
                .map_or(0, |e| e.0 as i32);
            let eff_max = app.workers.get_effective_max_hours(*wid, w as usize) as i32;
            let global_max = app.workers.get_max_hours(*wid) as i32;

            let shown = state.effort_filter_mode == 0
                || (state.effort_filter_mode == 1 && value == 0 && eff_max != 0)
                || (state.effort_filter_mode == 2 && value >= 40);
            if !shown {
                continue;
            }

            // triangolo se esiste una nota collegata al worker per questa settimana
            let has_note = app.workers.has_week_note(*wid, w as usize);
            if has_note {
                draw_note_triangle(ui, cell.shrink(STATUS_TRI_INSET));
            }

            // triangoli di stato: ferie (verde, alto-sx) / malattia (rosso, basso-sx)
            match app.workers.get_week_status(*wid, w as usize) {
                Some(WeekStatus::Ferie) => {
                    draw_status_triangle_top_left(ui, cell, g(Color32::from_rgb(0x33, 0x99, 0xFF)))
                }
                Some(WeekStatus::Malattia) => {
                    draw_status_triangle_bottom_left(ui, cell, g(Color32::RED))
                }
                None => {}
            }

            let color = if value > eff_max {
                g(Color32::RED)
            } else if eff_max == 0 {
                g(Color32::from_rgb(0xCD, 0x85, 0x3F)) // marrone chiaro: override a zero
            } else if value == 0 {
                g(Color32::YELLOW)
            } else {
                lightgreen
            };
            let txt = if eff_max != global_max {
                format!("{} | {}", value, eff_max)
            } else {
                value.to_string()
            };
            ui.painter().text(
                cell.center(),
                Align2::CENTER_CENTER,
                txt,
                cell_font(),
                color,
            );

            // Tasto sinistro → override ore max; tasto destro → nota worker/settimana.
            let resp = ui
                .interact(cell, ui.id().with(("wkmax", wid.0, w)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                state.popup = Some(Popup::WorkerWeekMax {
                    worker: *wid,
                    name: name.clone(),
                    week: w as usize,
                    text: eff_max.to_string(),
                });
            }
            let cur_status = app.workers.get_week_status(*wid, w as usize);
            resp.context_menu(|ui| {
                // "Ferie" e "Malattia" fanno da toggle: riscegliere lo stato attivo lo rimuove.
                if ui
                    .selectable_label(cur_status == Some(WeekStatus::Ferie), "Ferie")
                    .clicked()
                {
                    let status =
                        (cur_status != Some(WeekStatus::Ferie)).then_some(WeekStatus::Ferie);
                    actions.push(Action::SetWorkerWeekStatus {
                        worker: *wid,
                        week: w as usize,
                        status,
                    });
                    ui.close();
                }
                if ui
                    .selectable_label(cur_status == Some(WeekStatus::Malattia), "Malattia")
                    .clicked()
                {
                    let status =
                        (cur_status != Some(WeekStatus::Malattia)).then_some(WeekStatus::Malattia);
                    actions.push(Action::SetWorkerWeekStatus {
                        worker: *wid,
                        week: w as usize,
                        status,
                    });
                    ui.close();
                }
                if ui.button("Note").clicked() {
                    state.note_editor = Some(NoteEditing {
                        target: NoteTarget::WorkerWeek {
                            worker: *wid,
                            week: w as usize,
                            name: name.clone(),
                        },
                        text: app.workers.get_week_note(*wid, w as usize),
                    });
                    ui.close();
                }
            });

            // Passando sopra il triangolo si vede il testo della nota.
            if has_note {
                let tri = Rect::from_min_size(
                    egui::pos2(cell.right() - 12.0, cell.top()),
                    Vec2::new(12.0, 12.0),
                );
                if resp.hover_pos().is_some_and(|p| tri.contains(p)) {
                    let note = app.workers.get_week_note(*wid, w as usize);
                    // stesse impostazioni di `Response::on_hover_text` (tooltip
                    // ancorato alla cella, non al puntatore).
                    resp.show_tooltip_ui(|ui| {
                        ui.set_max_width(ui.spacing().tooltip_width);
                        ui.add(egui::widgets::Label::new(note));
                    });
                }
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
    ui.painter().add(egui::Shape::convex_polygon(
        pts,
        g(EFFORT_ORANGE),
        Stroke::NONE,
    ));
}

// ── Griglia (colonna destra) ────────────────────────────────────────────────

// Righe minime occupate dal nome progetto (campo multiriga; può crescere).
const NAME_ROWS: usize = 2;

/// Altezza effettiva del blocco nome: il `TextEdit` multiriga cresce con testi
/// lunghi, quindi misuriamo il galley wrappato a `LEFT_INFO_W` (mai meno di
/// `NAME_ROWS` righe). Senza questo, l'altezza riservata al progetto sarebbe
/// troppo piccola e inizio/fine sforerebbero nel progetto successivo.
fn name_block_height(ui: &egui::Ui, name: &str) -> f32 {
    let galley = ui
        .painter()
        .layout(name.to_string(), cell_font(), TEXT_WHITE, LEFT_INFO_W);
    galley.size().y.max(NAME_ROWS as f32 * ROW_H)
}

/// Layout condiviso da colonna sinistra e griglia: stessi progetti, stessi dev,
/// stesse altezze. Garantisce che le due colonne non possano divergere.
struct ProjLayout {
    proj: ProjectId,
    name: String,
    devs: Vec<(DevId, usize)>, // (dev, max_rows)
    proj_h: f32,
}

fn project_layout(ui: &egui::Ui, app: &App, filter: &Filter, compact: bool) -> Vec<ProjLayout> {
    // Righe info oltre al nome: compatta = solo tripletta; normale = tripletta +
    // categoria + inizio + fine.
    let extra_rows = if compact { 1.0 } else { 4.0 };
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
        let sum_devs: f32 = devs
            .iter()
            .map(|(_, m)| dev_block_height(*m, compact))
            .sum();
        // l'info riserva lo spazio reale del nome (può crescere su più righe).
        let info_h = extra_rows * ROW_H + name_block_height(ui, &name);
        let proj_h = sum_devs.max(info_h);
        out.push(ProjLayout {
            proj: proj_id,
            name,
            devs,
            proj_h,
        });
    }
    out
}

fn total_content_h(layout: &[ProjLayout]) -> f32 {
    DEV_BORDER + layout.iter().map(|p| p.proj_h + DEV_BORDER).sum::<f32>()
}

/// Titolo "Effort residuo" su due righe, centrato nella cella della colonna di
/// confine (testo scuro su sfondo canarino).
fn draw_boundary_title(ui: &egui::Ui, cell: Rect) {
    let font = mono(7.0);
    let cx = cell.center().x;
    ui.painter().text(
        egui::pos2(cx, cell.top() + cell.height() * 0.30),
        Align2::CENTER_CENTER,
        "Effort",
        font.clone(),
        BG_DARK,
    );
    ui.painter().text(
        egui::pos2(cx, cell.top() + cell.height() * 0.72),
        Align2::CENTER_CENTER,
        "residuo",
        font,
        BG_DARK,
    );
}

/// Striscia orizzontale piena (bordo dev / separatore progetto), disegnata a y assoluta.
fn paint_hstrip(ui: &egui::Ui, left: f32, w: f32, y: f32, color: Color32) {
    ui.painter().rect_filled(
        Rect::from_min_size(egui::pos2(left, y), Vec2::new(w, DEV_BORDER)),
        0.0,
        color,
    );
}

/// Come `paint_hstrip` ma limitata all'intervallo X `[x0, x1)`: usata per le
/// bande colorate dei dev, che non devono comparire fuori dal periodo attivo
/// del progetto. Se l'intervallo è vuoto non disegna nulla.
fn paint_hstrip_range(ui: &egui::Ui, x0: f32, x1: f32, y: f32, color: Color32) {
    if x1 <= x0 {
        return;
    }
    ui.painter().rect_filled(
        Rect::from_min_size(egui::pos2(x0, y), Vec2::new(x1 - x0, DEV_BORDER)),
        0.0,
        color,
    );
}

/// Intervallo X `[x0, x1)` delle colonne-settimana "attive" del progetto, cioè
/// comprese tra inizio e fine (estremi inclusi). `proj_start`/`deadline` a -1
/// significano "nessun limite" su quel lato. Se nessuna settimana visibile è
/// attiva restituisce un intervallo vuoto (`x0 == x1`).
fn active_hstrip_range(
    cols: &[Col],
    cw: f32,
    left: f32,
    content_w: f32,
    proj_start: i32,
    deadline: i32,
) -> (f32, f32) {
    // Nessun limite su entrambi i lati: banda a piena larghezza (comportamento storico).
    if proj_start < 0 && deadline < 0 {
        return (left, left + content_w);
    }
    let is_active = |c: &Col| {
        matches!(c, Col::Week(w)
            if (proj_start < 0 || *w >= proj_start) && (deadline < 0 || *w <= deadline))
    };
    match (
        cols.iter().position(is_active),
        cols.iter().rposition(is_active),
    ) {
        (Some(first), Some(last)) => (
            left + col_x_offset(cols, first, cw),
            left + col_x_offset(cols, last + 1, cw),
        ),
        _ => (left, left), // nessuna settimana attiva in vista: niente banda
    }
}

/// Etichetta data (sempre visibile) sopra la colonna inizio/fine di un progetto
/// in vista compatta. Sfondo colorato come la colonna, testo centrato sulla colonna.
fn draw_compact_date_marker(
    ui: &egui::Ui,
    left: f32,
    cw: f32,
    cols: &[Col],
    week: i32,
    top_y: f32,
    bg: Color32,
) {
    if week < 0 {
        return;
    }
    let Some(ci) = cols
        .iter()
        .position(|c| matches!(c, Col::Week(w) if *w == week))
    else {
        return;
    };
    let cx = left + col_x_offset(cols, ci, cw) + cw / 2.0;
    let date = primo_giorno_settimana_corrente(&days_to_local(week))
        .format("%y-%m-%d")
        .to_string();
    let galley = ui.painter().layout_no_wrap(date, mono(9.0), TEXT_WHITE);
    let pos = egui::pos2(cx - galley.size().x / 2.0, top_y + 1.0);
    let bgrect = Rect::from_min_size(pos, galley.size()).expand(1.5);
    ui.painter().rect_filled(bgrect, 2.0, bg);
    ui.painter().galley(pos, galley, TEXT_WHITE);
}

fn grid(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    filter: &Filter,
) {
    let compact = state.compact_mode;
    let cw = col_w(compact);
    let cols = columns_vec(app);
    let content_w = cols_width(&cols, cw);
    let layout = project_layout(ui, app, filter, compact);
    let total_h = total_content_h(&layout);

    // Un'unica allocazione: tutto il resto è disegno a coordinate assolute.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, total_h), Sense::hover());
    let left = rect.left();
    let mut y = rect.top();

    paint_hstrip(ui, left, content_w, y, g(START_STOP));
    y += DEV_BORDER;

    for p in &layout {
        let proj_start = app
            .projects
            .get_project_start_week(p.proj)
            .map(|w| w.0 as i32)
            .unwrap_or(-1);
        let deadline = app
            .projects
            .get_project_end_week(p.proj)
            .map(|w| w.0 as i32)
            .unwrap_or(-1);
        // Le bande colorate dei dev sono limitate al periodo attivo del progetto:
        // niente riga colorata prima dell'inizio o dopo la fine.
        let (act_x0, act_x1) =
            active_hstrip_range(&cols, cw, left, content_w, proj_start, deadline);
        let proj_top = y;
        let mut dy = y;

        for (dev_id, max_rows) in &p.devs {
            let color = dev_color(app, *dev_id);
            // i bordi dev sono trasparenti in compatta
            let border = if compact { BG_DARK } else { color };
            paint_hstrip_range(ui, act_x0, act_x1, dy, border);
            dy += DEV_BORDER;
            let inner_h = if compact {
                ROW_H
            } else {
                (*max_rows as f32 + 1.0) * ROW_H
            };
            let block = Rect::from_min_size(egui::pos2(left, dy), Vec2::new(content_w, inner_h));
            draw_dev_cells(
                ui, block, app, state, actions, p.proj, *dev_id, *max_rows, &cols, proj_start,
                deadline, filter, compact, cw,
            );
            dy += inner_h;
            paint_hstrip_range(ui, act_x0, act_x1, dy, border);
            dy += DEV_BORDER;
        }

        // Totale progetto in cima a ogni colonna di confine d'anno (solo progetti a
        // cavallo): somma dei residui positivi dei dev per completare il progetto.
        if !compact {
            let mut cx = left;
            for c in &cols {
                let colw = col_width(c, cw);
                if let Col::YearEnd(year_ending) = c {
                    if let Some(total) = project_missing_at_year_end(app, p.proj, *year_ending) {
                        let cell =
                            Rect::from_min_size(egui::pos2(cx, proj_top), Vec2::new(colw, ROW_H));
                        ui.painter().rect_filled(cell, 0.0, BG_DARK);
                        ui.painter().text(
                            cell.center(),
                            Align2::CENTER_CENTER,
                            format!("T:{total}"),
                            cell_font(),
                            g(EFFORT_ORANGE),
                        );
                    }
                }
                cx += colw;
            }
        }

        // in compatta: etichetta data sopra le colonne inizio (azzurra) e fine (verde)
        if compact {
            draw_compact_date_marker(ui, left, cw, &cols, proj_start, proj_top, g(START_BG));
            draw_compact_date_marker(ui, left, cw, &cols, deadline, proj_top, g(DEADLINE_BG));
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
    cols: &[Col],
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
    let hide_effort = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| sd.get_hide_effort())
        .unwrap_or(false);

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
    let mut next_x = rect.left();
    for c in cols.iter() {
        let colw = col_width(c, cw);
        let x = next_x;
        next_x += colw;

        // ── Colonna di confine d'anno: sola lettura, sfondo canarino ──
        if let Col::YearEnd(year_ending) = c {
            let col_rect =
                Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(colw, rect.height()));
            ui.painter().rect_filled(col_rect, 0.0, g(CANARY));
            // ore mancanti del dev (solo se il progetto è a cavallo del confine)
            if let Some(missing) = dev_missing_at_year_end(app, proj, dev, *year_ending) {
                if !hide_effort {
                    // In basso, per non collidere col totale progetto in cima alla colonna.
                    ui.painter().text(
                        col_rect.center_bottom() - Vec2::new(0.0, 2.0),
                        Align2::CENTER_BOTTOM,
                        missing.to_string(),
                        cell_font(),
                        BG_DARK,
                    );
                }
            }
            continue;
        }
        let w = match c {
            Col::Week(w) => w,
            Col::YearEnd(_) => unreachable!(),
        };

        let before_start = proj_start >= 0 && *w < proj_start;
        let after_deadline = deadline >= 0 && *w > deadline;
        let is_deadline = deadline >= 0 && *w == deadline;

        // colonna settimana: bg deadline/start
        let col_rect = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cw, rect.height()));
        if is_deadline {
            ui.painter().rect_filled(col_rect, 0.0, g(DEADLINE_BG));
        } else if proj_start >= 0 && *w == proj_start {
            ui.painter().rect_filled(col_rect, 0.0, g(START_BG));
        }
        if !compact && *w == state.this_week {
            ui.painter()
                .rect_filled(col_rect, 0.0, g(THIS_WEEK).gamma_multiply(0.18));
        }

        // ── Milestone: tinta colonna col colore della milestone + tooltip;
        //    tasto destro sulla riga in alto per aggiungere/rimuovere. ──
        let ms_here = app
            .projects
            .project_milestones_at_week(proj, WeekId(*w as usize));
        if let Some(first) = ms_here.first() {
            if let Some(color) = app.milestones.get_color(*first) {
                ui.painter().rect_filled(col_rect, 0.0, from_hex(color));
            }
        }
        {
            let top = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cw, ROW_H));
            let mut resp = ui.interact(
                top,
                egui::Id::new(("msrow", proj.0, dev.0, *w)),
                Sense::click(),
            );
            if !ms_here.is_empty() {
                let names: Vec<String> = ms_here
                    .iter()
                    .filter_map(|m| app.milestones.get_name(*m).map(|s| s.to_string()))
                    .collect();
                resp = resp.on_hover_text(names.join(", "));
            }
            let week_id = WeekId(*w as usize);
            resp.context_menu(|ui| {
                ui.label("Aggiungi milestone qui:");
                let all = app.milestones.list();
                if all.is_empty() {
                    ui.label("(nessuna — creane dalla toolbar)");
                }
                for (id, name, color) in &all {
                    let here = ms_here.contains(id);
                    let mark = if here { "● " } else { "" };
                    let label =
                        egui::RichText::new(format!("{mark}{name}")).color(u32_to_color(*color));
                    if ui.button(label).clicked() {
                        actions.push(Action::AddProjectMilestone {
                            proj,
                            milestone: *id,
                            week: week_id,
                        });
                        ui.close();
                    }
                }
                if !ms_here.is_empty() {
                    ui.separator();
                    for m in &ms_here {
                        if let Some(name) = app.milestones.get_name(*m) {
                            if ui.button(format!("Rimuovi: {name}")).clicked() {
                                actions.push(Action::RemoveProjectMilestone {
                                    proj,
                                    milestone: *m,
                                });
                                ui.close();
                            }
                        }
                    }
                }
            });
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
                    let bar_h = if week_total == 0 {
                        1.0
                    } else {
                        (ROW_H * ratio).max(1.0)
                    };
                    let bar = Rect::from_min_size(
                        egui::pos2(x, rect.bottom() - bar_h),
                        Vec2::new(cw, bar_h),
                    );
                    ui.painter().rect_filled(bar, 0.0, dcolor);
                    // tooltip con data + effort della settimana
                    let date = primo_giorno_settimana_corrente(&days_to_local(*w))
                        .format("%y-%m-%d")
                        .to_string();
                    ui.interact(
                        col_rect,
                        egui::Id::new(("cbar", proj.0, dev.0, *w)),
                        Sense::hover(),
                    )
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
                let color = if is_deadline {
                    TEXT_WHITE
                } else {
                    cumulative_color(week_total, planned)
                };
                ui.painter().text(
                    cum_rect.center(),
                    Align2::CENTER_CENTER,
                    txt,
                    cell_font(),
                    color,
                );
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
                            egui::Event::Key {
                                key: egui::Key::Backspace,
                                pressed: true,
                                ..
                            } => {
                                // Cancellazione letterale: niente re-completamento, altrimenti
                                // un nome completo verrebbe ri-completato e non si potrebbe svuotare.
                                ed.typed.pop();
                                ed.buf = ed.typed.clone();
                            }
                            egui::Event::Key {
                                key: egui::Key::Delete,
                                pressed: true,
                                ..
                            } => {
                                ed.typed.clear();
                                ed.buf.clear();
                            }
                            egui::Event::Key {
                                key: egui::Key::Enter,
                                pressed: true,
                                ..
                            }
                            | egui::Event::Key {
                                key: egui::Key::Tab,
                                pressed: true,
                                ..
                            } => {
                                commit = true;
                            }
                            egui::Event::Key {
                                key: egui::Key::Escape,
                                pressed: true,
                                ..
                            } => {
                                cancel = true;
                            }
                            // Copia / Taglia / Incolla: eventi semantici di egui,
                            // portabili (Cmd su macOS, Ctrl su Windows/Linux). Si scrive
                            // anche nella clipboard di SISTEMA: senza, su Windows egui non
                            // genera mai `Event::Paste` (lo emette solo se la clipboard di
                            // sistema non è vuota — vedi egui-winit), e l'incolla non parte.
                            // La nota della cella resta nella clipboard interna.
                            egui::Event::Copy => {
                                state.copied_text = ed.buf.clone();
                                state.copied_note = note.clone();
                                ui.ctx().copy_text(ed.buf.clone());
                            }
                            egui::Event::Cut => {
                                state.copied_text = ed.buf.clone();
                                state.copied_note = note.clone();
                                ui.ctx().copy_text(ed.buf.clone());
                                ed.buf.clear();
                                ed.typed.clear();
                            }
                            egui::Event::Paste(text) => {
                                // Se l'incolla viene dalla nostra copia (stesso testo),
                                // ripristina anche la nota; altrimenti incolla il testo
                                // esterno senza nota.
                                let note = if text == state.copied_text {
                                    state.copied_note.clone()
                                } else {
                                    String::new()
                                };
                                ed.buf = text.clone();
                                ed.typed = text;
                                ed.paste_note = Some(note);
                            }
                            _ => {}
                        }
                    }
                } else if ed.had_focus {
                    commit = true; // focus perso → conferma
                }

                // disegno cella in editing + caret
                ui.painter().rect_filled(cell, 0.0, g(SEL_BG));
                ui.painter().rect_stroke(
                    cell,
                    0.0,
                    Stroke::new(1.0, g(FOCUS_BORDER)),
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
                    ui.painter()
                        .rect_filled(cell, 0.0, g(SEL_BG).gamma_multiply(0.4));
                }
                if !note.is_empty() {
                    draw_note_triangle(ui, cell);
                }
                if !text.is_empty() {
                    let wname = text.split('|').next().unwrap_or("").trim();
                    let wid = app.workers.get_id_by_name(wname);
                    let hidden = wid.map_or(false, |id| app.workers.is_hidden_in_footer(id));
                    let sovra = wid
                        .map(|id| {
                            app.sovra
                                .get(&(WeekId(*w as usize), id))
                                .map_or(0, |e| e.0 as i32)
                        })
                        .unwrap_or(0);
                    let max_h = wid
                        .map(|id| app.workers.get_effective_max_hours(id, *w as usize) as i32)
                        .unwrap_or(DEFAULT_MAX_HOURS as i32);
                    // worker nascosti nel footer → grigi, a prescindere dall'effort
                    let color = if hidden {
                        Color32::from_gray(0x80)
                    } else if sovra > max_h {
                        g(Color32::RED)
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
                        target: NoteTarget::Effort {
                            proj,
                            dev,
                            week: *w,
                            worker: wname,
                        },
                        text: note.clone(),
                    });
                }
            }
        }
    }
}

/// Tutte le voci (worker, effort, nota) della settimana, ignorando il filtro.
fn full_week_entries(
    app: &App,
    proj: ProjectId,
    dev: DevId,
    week: i32,
) -> Vec<(String, usize, String)> {
    app.projects
        .get_single_dev(proj, dev)
        .and_then(|sd| sd.get_all(WeekId(week as usize)))
        .map(|sew| {
            sew.worker_id
                .iter()
                .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                .map(|(wid, se)| {
                    (
                        app.workers.get_name_by_id(*wid).to_string(),
                        se.get_effort().0,
                        se.get_note(),
                    )
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
            let note = ed.paste_note.clone().unwrap_or_else(|| {
                if wname == ed.orig_worker {
                    ed.orig_note.clone()
                } else {
                    String::new()
                }
            });
            entries.retain(|(n, _, _)| n != &wname);
            entries.push((wname, eff, note));
        }
    }
    let rows: Vec<String> = entries
        .iter()
        .map(|(n, e, _)| format!("{}|{}", n, e))
        .collect();
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
    let measure = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), color)
            .size()
            .x
    };

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
            let cand: String = text
                .chars()
                .take(keep)
                .chain(std::iter::once('…'))
                .collect();
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

/// Scostamento dei triangoli di stato dagli angoli, verso il centro cella,
/// così ferie (alto-sx) e malattia (basso-sx) non si toccano.
const STATUS_TRI_INSET: f32 = 3.0;

/// Triangolo di stato nell'angolo in alto a SINISTRA (ferie).
fn draw_status_triangle_top_left(ui: &egui::Ui, cell: Rect, color: Color32) {
    let x = cell.left() + STATUS_TRI_INSET;
    let y = cell.top() + STATUS_TRI_INSET;
    let pts = vec![
        egui::pos2(x, y),
        egui::pos2(x + 10.0, y),
        egui::pos2(x, y + 10.0),
    ];
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Triangolo di stato nell'angolo in basso a SINISTRA (malattia).
fn draw_status_triangle_bottom_left(ui: &egui::Ui, cell: Rect, color: Color32) {
    let x = cell.left() + STATUS_TRI_INSET;
    let y = cell.bottom() - STATUS_TRI_INSET;
    let pts = vec![
        egui::pos2(x, y),
        egui::pos2(x + 10.0, y),
        egui::pos2(x, y - 10.0),
    ];
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

fn draw_note_triangle(ui: &egui::Ui, cell: Rect) {
    let tr = cell.right_top();
    let pts = vec![
        egui::pos2(tr.x - 10.0, tr.y),
        egui::pos2(tr.x, tr.y),
        egui::pos2(tr.x, tr.y + 10.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        pts,
        g(NOTE_ORANGE),
        Stroke::NONE,
    ));
}

fn dev_color(app: &App, dev: DevId) -> Color32 {
    app.devs
        .list_full()
        .into_iter()
        .find(|(id, _, _, _)| *id == dev)
        .map(|(_, _, bg, _)| from_hex(bg as u32))
        .unwrap_or(g(Color32::from_rgb(0x00, 0x99, 0xFF)))
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

fn left_column(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    filter: &Filter,
) {
    let compact = state.compact_mode;
    let layout = project_layout(ui, app, filter, compact);
    let total_h = total_content_h(&layout);

    // Stessa altezza totale e stessa allocazione singola della griglia.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(LEFT_W, total_h), Sense::hover());
    let left = rect.left();
    let mut y = rect.top();

    paint_hstrip(ui, left, LEFT_W, y, g(START_STOP));
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
        ui.painter().text(
            trip_rect.center(),
            Align2::CENTER_CENTER,
            "—",
            cell_font(),
            TEXT_FAINT,
        );
    } else {
        ui.painter().text(
            trip_rect.center(),
            Align2::CENTER_CENTER,
            &trip,
            cell_font(),
            g(EFFORT_ORANGE),
        );
    }
    let tr = ui.interact(trip_rect, egui::Id::new(("trip", proj.0)), Sense::click());
    if tr.secondary_clicked() {
        state.popup = Some(Popup::Tripletta {
            proj,
            text: trip.clone(),
        });
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
        let cat_col = if cat == "—" {
            TEXT_FAINT
        } else {
            g(CAT_BLUE)
        };
        ui.painter().text(
            cat_rect.center(),
            Align2::CENTER_CENTER,
            cat,
            cell_font(),
            cat_col,
        );
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
    let buf = state
        .name_buffers
        .entry(proj.0)
        .or_insert_with(|| proj_name.to_string());
    let resp = ui.put(
        name_rect,
        egui::TextEdit::multiline(buf)
            .font(cell_font())
            .text_color(TEXT_WHITE)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_rows(NAME_ROWS)
            .desired_width(w),
    );
    if resp.lost_focus() {
        actions.push(Action::SetProjectName {
            proj,
            name: buf.clone(),
        });
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
        .map(|wk| {
            primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32))
                .format("%y-%m-%d")
                .to_string()
        })
        .unwrap_or_default();
    let start_txt = if start_date.is_empty() {
        "Inizio: —".to_string()
    } else {
        format!("Inizio: {}", start_date)
    };
    let sr = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(
        sr.center(),
        Align2::CENTER_CENTER,
        start_txt,
        mono(FONT_SIZE - 2.0),
        TEXT_DIM,
    );
    let srr = ui.interact(sr, egui::Id::new(("start", proj.0)), Sense::click());
    if srr.secondary_clicked() {
        state.popup = Some(Popup::Start {
            proj,
            text: start_date,
        });
    }
    srr.on_hover_text("Tasto destro: modifica data inizio");
    y += ROW_H;

    // fine (right-click per modificare)
    let end_date = app
        .projects
        .get_project_end_week(proj)
        .map(|wk| {
            primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32))
                .format("%y-%m-%d")
                .to_string()
        })
        .unwrap_or_default();
    let end_txt = if end_date.is_empty() {
        "Fine: —".to_string()
    } else {
        format!("Fine: {}", end_date)
    };
    let er = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(
        er.center(),
        Align2::CENTER_CENTER,
        end_txt,
        mono(FONT_SIZE - 2.0),
        TEXT_DIM,
    );
    let err = ui.interact(er, egui::Id::new(("end", proj.0)), Sense::click());
    if err.secondary_clicked() {
        state.popup = Some(Popup::End {
            proj,
            text: end_date,
        });
    }
    err.on_hover_text("Tasto destro: modifica deadline");
}

fn draw_left_dev_strip(ui: &mut egui::Ui, rect: Rect, proj: ProjectId, state: &mut UiState) {
    let x = rect.left() + LEFT_INFO_W;
    let strip = Rect::from_min_size(
        egui::pos2(x, rect.top()),
        Vec2::new(DEV_STRIP_W, rect.height()),
    );
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
        let inner_h = if compact {
            ROW_H
        } else {
            (max_rows as f32 + 1.0) * ROW_H
        };

        // cella nome dev (90px, doppio click = add row)
        let name_rect =
            Rect::from_min_size(egui::pos2(x0, inner_y), Vec2::new(DEV_NAME_W, inner_h));
        ui.painter().rect_filled(name_rect, 0.0, color);
        ui.painter().text(
            name_rect.center(),
            Align2::CENTER_CENTER,
            dev_name(app, *dev),
            cell_font(),
            tcol,
        );
        let dev_note = app
            .projects
            .get_single_dev(proj, *dev)
            .map(|sd| sd.get_note())
            .unwrap_or_default();
        let hide_effort = app
            .projects
            .get_single_dev(proj, *dev)
            .map(|sd| sd.get_hide_effort())
            .unwrap_or(false);
        if !dev_note.is_empty() {
            draw_note_triangle(ui, name_rect);
        }
        let nresp = ui.interact(
            name_rect,
            egui::Id::new(("devname", proj.0, dev.0)),
            Sense::click(),
        );
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
            let label = if hide_effort {
                "Visualizza effort"
            } else {
                "Nascondi effort"
            };
            if ui.button(label).clicked() {
                actions.push(Action::SetDevHideEffort {
                    proj,
                    dev: *dev,
                    hide: !hide_effort,
                });
                ui.close_menu();
            }
        });
        if !dev_note.is_empty() {
            nresp.on_hover_text(dev_note);
        }

        // area effort/remains (65px) — i campi sono nascosti in compatta
        let eff_x = x0 + DEV_NAME_W;
        let eff_area =
            Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, inner_h));
        ui.painter().rect_filled(eff_area, 0.0, color);

        if !compact {
            // effort pianificato (editabile)
            let planned = app
                .projects
                .get_single_dev(proj, *dev)
                .map(|sd| sd.planned_effort().0)
                .unwrap_or(0);
            let eff_rect =
                Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, ROW_H));
            let key = (proj.0, dev.0);
            let buf = state
                .effort_buffers
                .entry(key)
                .or_insert_with(|| planned.to_string());
            let resp = ui.put(
                eff_rect,
                egui::TextEdit::singleline(buf)
                    .font(cell_font())
                    .frame(egui::Frame::NONE)
                    .horizontal_align(egui::Align::Center),
            );
            if resp.lost_focus() {
                if let Ok(v) = buf.trim().parse::<usize>() {
                    actions.push(Action::SetDevEffort {
                        proj,
                        dev: *dev,
                        effort: v,
                    });
                }
            } else if !resp.has_focus() && *buf != planned.to_string() {
                *buf = planned.to_string();
            }

            // remains
            let total = app
                .projects
                .get_single_dev(proj, *dev)
                .map(|sd| sd.get_effort_tot().0 as i32)
                .unwrap_or(0);
            let remains = planned as i32 - total;
            let rem_rect = Rect::from_min_size(
                egui::pos2(eff_x, inner_y + ROW_H),
                Vec2::new(DEV_EFFORT_W, ROW_H),
            );
            if (remains == planned as i32 && planned != 0) || remains < 0 {
                ui.painter().rect_filled(rem_rect, 0.0, g(Color32::RED));
            }
            ui.painter().text(
                rem_rect.center(),
                Align2::CENTER_CENTER,
                remains.to_string(),
                cell_font(),
                TEXT_WHITE,
            );
        }

        // bordo inferiore
        let bot_b = Rect::from_min_size(
            egui::pos2(x0, inner_y + inner_h),
            Vec2::new(LEFT_DEV_W, DEV_BORDER),
        );
        ui.painter().rect_filled(bot_b, 0.0, color);

        y += block_h;
    }
}
