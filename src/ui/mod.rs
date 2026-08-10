//! Interfaccia egui (immediate-mode) — sostituisce il layer Slint.
//! Tappa 1: griglia effort fedele, editing celle, scroll sincronizzato,
//! salva/apri, aggiunta worker/dev/progetto/categoria.

pub(crate) use std::collections::{HashMap, HashSet};

pub(crate) use chrono::{Datelike, Utc};
pub(crate) use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Vec2};
pub(crate) use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

pub(crate) use crate::app::App;
pub(crate) use crate::categories::CategoryId;
pub(crate) use crate::date_utils::dates::{
    days_to_local, local_to_days, parse_date_str, primo_giorno_settimana_corrente,
};
pub(crate) use crate::dev_utils::dev::DevId;
pub(crate) use crate::milestones::{MilestoneId, MilestoneKind};
pub(crate) use crate::project_utils::project::{Enable, OverflowResolution, ProjectId};
pub(crate) use crate::single_dev_utils::single_dev::{WEEK_STEP, WeekId};
pub(crate) use crate::single_effort_utils::sinlge_effort::Effort;
pub(crate) use crate::ui_style::*;
pub(crate) use crate::workers_utils::worker::{WORKER_ID_ZERO, WeekStatus, WorkerId};

mod compare;
mod dialogs;
mod export;
mod filters;
mod footer;
mod grid;
mod help;
mod saturation;
mod toasts;
mod toolbar;
pub(crate) use compare::*;
pub(crate) use dialogs::*;
pub(crate) use export::*;
pub(crate) use filters::*;
pub(crate) use footer::*;
pub(crate) use grid::*;
pub(crate) use help::*;
pub(crate) use saturation::*;
pub(crate) use toasts::*;
pub(crate) use toolbar::*;

// ── Stato di sola UI ────────────────────────────────────────────────────────

pub(crate) struct Editing {
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
pub(crate) enum Popup {
    Tripletta {
        proj: ProjectId,
        text: String,
    },
    /// Note del progetto per settimana, aperte con tasto destro sulla tripletta.
    /// `weeks` è il buffer editabile in ordine di visualizzazione: la settimana
    /// corrente (`current`) sempre in cima ed evidenziata, poi le altre per data
    /// decrescente. Al salvataggio le voci vuote vengono scartate.
    ProjectNotes {
        proj: ProjectId,
        weeks: Vec<(WeekId, String)>,
        current: WeekId,
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

/// Flusso multi-passo per lo spostamento di effort (menù "Sposta").
pub(crate) enum MoveDialog {
    /// Selezione dei dev da spostare (solo per "Sposta devs").
    SelectDevs {
        proj: ProjectId,
        candidates: Vec<(DevId, String)>,
        selected: HashSet<DevId>,
    },
    /// Numero di settimane + milestone da trascinare insieme.
    Params {
        proj: ProjectId,
        moves: Vec<(DevId, Vec<WeekId>)>,
        weeks_text: String,
        milestones: Vec<(MilestoneId, String, bool)>,
    },
    /// Conferma di uno sforamento del confine di progetto.
    Overflow {
        proj: ProjectId,
        moves: Vec<(DevId, Vec<WeekId>)>,
        delta: i64,
        milestones: Vec<MilestoneId>,
        /// true = sfora la fine (delta>0); false = sfora l'inizio (delta<0).
        end_side: bool,
    },
}

/// Inserimento multiplo di effort, aperto col tasto destro su una cella
/// **vuota** della griglia: si scelgono più worker, l'effort settimanale e per
/// quante settimane, e il sistema li scrive in blocco a partire dalla settimana
/// cliccata (inclusa) andando in avanti, sovrascrivendo eventuali valori già
/// presenti per quel worker.
pub(crate) struct BulkFill {
    pub proj: ProjectId,
    pub dev: DevId,
    /// Settimana della cella cliccata (giorno, non indice): prima da riempire.
    pub week: i32,
    /// Ricerca sull'elenco worker (come nella dialog dei filtri).
    pub search: String,
    pub selected: HashSet<WorkerId>,
    pub effort_text: String,
    pub weeks_text: String,
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

/// Preferenza tema: Auto segue il sistema (di giorno chiaro, la sera scuro se
/// macOS è su Aspetto "Automatico"); Light/Dark forzano manualmente.
#[derive(Clone, Copy, PartialEq, Default)]
enum ThemePref {
    #[default]
    Auto,
    Light,
    Dark,
}

/// Modalità di visualizzazione dei progetti nel corpo centrale (menù Vista).
/// Filtra quali progetti compaiono, in aggiunta al filtro di visibilità
/// (enable) e a quello worker. **Non è persistita**: si riparte da `Open`
/// (solo aperti) a ogni avvio.
#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) enum ProjectViewMode {
    /// Solo progetti aperti (non chiusi).
    #[default]
    Open,
    /// Solo progetti chiusi.
    Closed,
    /// Tutti i progetti (aperti e chiusi).
    All,
}

/// Colonna della dialog unica dei filtri: decide quale campo di ricerca riceve
/// il focus quando la finestra compare (Ctrl+F/G → Workers, Ctrl+P → Progetti,
/// Ctrl+D → Dev).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterPane {
    Workers,
    Projects,
    Devs,
}

/// Voce di menù «Filtri ▸ …»: apre la dialog unica (o la chiude se era già
/// aperta sulla stessa colonna) portando il focus su `pane`.
pub(crate) fn toggle_filters(state: &mut UiState, pane: FilterPane) {
    if state.show_filters && state.filters_focus == Some(pane) {
        state.show_filters = false;
    } else {
        open_filters(state, pane);
    }
}

/// Azzera **tutti** i filtri della dialog «Filtri» (Ctrl+J): worker e dev
/// tornano "tutti selezionati", tutti i progetti aperti tornano visibili, la
/// modalità "solo settimana corrente" si spegne e le ricerche si svuotano.
/// Non è un toggle: ripetuto, riporta sempre a "si vede tutto".
pub(crate) fn reset_all_filters(app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    state.worker_filter = None;
    state.dev_filter = None;
    state.worker_filter_current_week = false;
    state.worker_search.clear();
    state.project_search.clear();
    state.dev_search.clear();
    // Eventuali toggle richiesti nello stesso frame non devono ri-deselezionare.
    state.worker_filter_toggle_all = false;
    state.dev_filter_toggle_all = false;
    state.project_filter_toggle_all = false;
    // Visibilità progetti: i chiusi restano fuori (chiudere forza `enable = false`).
    for (proj, _) in app.projects.list() {
        if !app.projects.is_closed(proj) {
            actions.push(Action::SetProjectEnabled {
                proj,
                enabled: true,
            });
        }
    }
}

/// Apre la dialog unica dei filtri con il focus sulla colonna `pane`.
fn open_filters(state: &mut UiState, pane: FilterPane) {
    state.show_filters = true;
    state.filters_just_opened = true;
    focus_pane(state, pane);
}

/// Chiede il focus per il campo di ricerca della colonna `pane`.
fn focus_pane(state: &mut UiState, pane: FilterPane) {
    state.filters_focus = Some(pane);
    state.filters_focus_dirty = true;
}

/// True se il progetto va mostrato nel corpo centrale in base alla modalità
/// Vista corrente.
///
/// Nota importante: chiudere un progetto ne forza `enable = false` (vedi
/// `Project::set_closed`), quindi il flag `enable` distingue solo gli **aperti**
/// nascosti dal filtro «Filtri ▸ Progetti…». Per questo il filtro `enable` si
/// applica ai soli progetti aperti; i chiusi vengono decisi dal solo stato
/// `closed`.
fn project_in_body(app: &App, view: ProjectViewMode, proj: ProjectId) -> bool {
    let closed = app.projects.is_closed(proj);
    let enabled = app.projects.get_enable(&proj).0;
    match view {
        ProjectViewMode::Open => !closed && enabled,
        ProjectViewMode::Closed => closed,
        // Aperti solo se non nascosti dal filtro; chiusi sempre visibili.
        ProjectViewMode::All => closed || enabled,
    }
}

/// Progetti attualmente presenti nel corpo centrale, coerenti con la modalità
/// Vista (aperti / chiusi / tutti) e con il filtro di visibilità. Non tiene
/// conto del filtro worker (che nasconde solo dev, non progetti interi ai fini
/// dell'export). Usato per allineare gli elenchi di PDF/SVG/minuta a ciò che si
/// vede a schermo, in ordine di visualizzazione.
fn body_projects(app: &App, view: ProjectViewMode) -> Vec<ProjectId> {
    app.projects
        .list_full()
        .into_iter()
        .filter(|(id, _, _)| project_in_body(app, view, *id))
        .map(|(id, _, _)| id)
        .collect()
}

/// Stato della dialog "Esporta PDF" del singolo progetto: i dev del progetto con
/// il flag di selezione, nell'ordine (riordinabile con ▲▼) scelto dall'utente.
struct PdfExport {
    proj: ProjectId,
    entries: Vec<(DevId, bool)>,
}

/// Stato della dialog "Esporta PDF" con più progetti visibili: l'elenco dei
/// progetti esportabili con il flag di selezione (una pagina Gantt ciascuno).
struct PdfMultiExport {
    entries: Vec<(ProjectId, bool)>,
}

/// Stato della dialog "Minuta": progetti non chiusi con flag di selezione e la
/// scelta se includere solo le note della settimana corrente oppure tutte.
struct MinutaState {
    entries: Vec<(ProjectId, bool)>,
    /// true = solo settimana corrente; false = tutte le note.
    only_current: bool,
    /// true = escludi i progetti senza note nell'ambito scelto (default).
    only_with_notes: bool,
    /// true = aggiungi anche le note delle celle effort (nota per worker).
    worker_notes: bool,
}

// ── Notifiche in-app (toast) ────────────────────────────────────────────────

/// Gravità di una notifica: decide icona, colore e durata.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ToastLevel {
    /// Informazione (es. il file è cambiato ed è stato ricaricato).
    Info,
    /// Operazione riuscita (salvataggio, export scritto).
    Success,
    /// Nulla di rotto, ma l'operazione non ha prodotto niente.
    Warning,
    /// Errore: resta finché non lo si chiude.
    Error,
}

impl ToastLevel {
    pub(crate) fn icon(self) -> &'static str {
        match self {
            ToastLevel::Info => "ℹ",
            ToastLevel::Success => "✔",
            ToastLevel::Warning => "⚠",
            ToastLevel::Error => "⛔",
        }
    }

    fn color(self) -> Color32 {
        match self {
            ToastLevel::Info => info_blue(),
            ToastLevel::Success => ok_green(),
            ToastLevel::Warning => warn_amber(),
            ToastLevel::Error => error_red(),
        }
    }

    /// Da quanto tempo la notifica sparisce da sola; `None` = resta finché non
    /// la si chiude (gli errori non devono poter passare inosservati).
    pub(crate) fn ttl(self) -> Option<std::time::Duration> {
        let secs = match self {
            ToastLevel::Success => 4,
            ToastLevel::Info => 10,
            ToastLevel::Warning => 15,
            ToastLevel::Error => return None,
        };
        Some(std::time::Duration::from_secs(secs))
    }
}

/// Una notifica in-app: sostituisce gli `eprintln!` invisibili.
pub(crate) struct Toast {
    pub(crate) level: ToastLevel,
    pub(crate) text: String,
    /// Quando è stata mostrata (o rinfrescata): base per la scadenza.
    pub(crate) born: std::time::Instant,
}

/// Oltre questo numero le notifiche più vecchie vengono scartate.
const MAX_TOASTS: usize = 5;

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
    // notifiche in-app (errori, esiti di salvataggio/export, aggiornamenti dal
    // file condiviso): disegnate in basso a destra da `toasts_layer`.
    toasts: Vec<Toast>,
    this_week: i32,
    scroll_x: f32,
    scroll_y: f32,
    // scroll orizzontale iniziale da applicare alla griglia (settimana corrente)
    pending_scroll_x: Option<f32>,
    // scroll verticale da applicare alla griglia (salto rapido a un progetto)
    pending_scroll_y: Option<f32>,
    // ricerche delle tre colonne della dialog "Filtri" (⌘/Ctrl+F, +P, +D).
    // Quella dei progetti unifica filtro visibilità + salto rapido.
    project_search: String,
    worker_search: String,
    dev_search: String,
    // progetto verso cui scrollare al prossimo frame (risolto in `body`)
    jump_to_project: Option<ProjectId>,
    editing: Option<Editing>,
    note_editor: Option<NoteEditing>,
    popup: Option<Popup>,
    // inserimento multiplo (tasto destro su cella vuota): worker + ore + settimane
    bulk_fill: Option<BulkFill>,
    bulk_fill_was_open: bool,
    dev_manage: Option<ProjectId>,
    confirm_del_dev: Option<(ProjectId, DevId)>,
    // flusso "Sposta" (blocco / devs) aperto dal menù contestuale milestone
    move_dialog: Option<MoveDialog>,
    move_dialog_was_open: bool,
    // filtro worker: None = nessun filtro (tutti); Some(set) = mostra solo questi nomi
    worker_filter: Option<HashSet<String>>,
    // Modalità "settimana corrente" del filtro worker (Ctrl+G): con filtro attivo
    // un progetto compare solo se un worker selezionato ha effort nella settimana
    // corrente. Dentro il progetto la resa resta come Ctrl+F (tutte le settimane).
    // Ctrl+F la disattiva, Ctrl+G la riattiva. Non persistita.
    worker_filter_current_week: bool,
    // filtro dev (Ctrl+D): None = nessun filtro; Some(set) = mostra solo i
    // progetti in cui almeno un dev selezionato ha effort > 0, e dentro il
    // progetto solo quei dev. Si combina in AND con il filtro worker. Non persistito.
    dev_filter: Option<HashSet<DevId>>,
    // dialog unica dei filtri (Workers + Progetti + Dev): aperta da Ctrl+F/G/P/D.
    show_filters: bool,
    // ultima colonna richiesta (scorciatoia/menù): `filters_focus_dirty` chiede di
    // dare il focus al suo campo di ricerca al prossimo frame, poi si azzera.
    filters_focus: Option<FilterPane>,
    filters_focus_dirty: bool,
    show_closed_filter: bool,
    // conferma di uscita con modifiche non salvate
    show_exit_confirm: bool,
    allow_close: bool,
    // evita la chiusura "click-fuori" nello stesso frame in cui la finestra si apre
    filters_just_opened: bool,
    closed_filter_just_opened: bool,
    // richiesta (da Ctrl+F / Ctrl+G / Ctrl+P / Ctrl+D a dialog già aperta) di fare
    // il toggle del "Select All" della rispettiva colonna; consumata dalla dialog
    // "Filtri" nello stesso frame.
    worker_filter_toggle_all: bool,
    dev_filter_toggle_all: bool,
    project_filter_toggle_all: bool,
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
    // zoom settimane (solo vista NON compatta): 0 = normale, 1 = merge 2 settimane,
    // 2 = merge 4 settimane. Le colonne mergiate sono sola lettura (somma effort).
    zoom_level: u8,
    // footer collassato tramite la maniglia col triangolino (indipendente da compact_mode)
    footer_hidden: bool,
    // resa in bianco/nero (senza colori)
    bw_mode: bool,
    // modalità progetti (Vista ▸ Solo aperti / Solo chiusi / Tutti). Filtro di
    // sola visualizzazione, non persistito: riparte da "Tutti" a ogni avvio.
    project_view: ProjectViewMode,
    // formato barre scelto per l'export PDF/SVG (ricordato tra un export e
    // l'altro, non persistito su file).
    bar_format: crate::pdf_export::BarFormat,
    // tema chiaro/scuro: Auto = segue il sistema, altrimenti forzato
    theme_pref: ThemePref,
    // dialog "Esporta PDF" del singolo progetto (aperta quando resta visibile
    // un solo progetto e si lancia l'export)
    pdf_export: Option<PdfExport>,
    // dialog "Esporta PDF" con più progetti visibili: selezione dei progetti
    pdf_multi_export: Option<PdfMultiExport>,
    // dialog "Minuta" (File ▸ Minuta…): selezione progetti + scelta note
    minuta: Option<MinutaState>,
    // autosave: istante (secondi, orologio egui) dell'ultimo salvataggio
    last_save_time: f64,
    // messaggio di errore/incoerenza da mostrare dopo un caricamento fallito o
    // con riferimenti pendenti (validazione schema)
    load_error: Option<String>,
    // finestra "Manuale d'uso" (Aiuto ▸ Manuale d'uso…)
    show_help: bool,
    // cruscotto saturazione worker (Vista ▸ Saturazione worker…)
    show_saturation: bool,
    saturation_monthly: bool,     // false = per settimana, true = per mese
    saturation_future_only: bool, // mostra solo dalla settimana corrente in poi
    // worker con la riga effort selezionata nel cruscotto (multi-selezione a
    // toggle: click seleziona, ri-click deseleziona, altri click si sommano)
    sat_selected: HashSet<WorkerId>,
    // offset verticale condiviso tra colonna fissa (nome+Σ) e heatmap, così le
    // due parti scorrono insieme mentre la heatmap scorre in orizzontale
    sat_scroll_y: f32,
    // testo di ricerca nel manuale (filtra le sezioni)
    help_search: String,
    // capitolo del manuale a cui scrollare al prossimo frame (indice di sezione,
    // impostato dal click sull'indice laterale del manuale)
    help_scroll_to: Option<usize>,
    // cache di rendering Markdown (immagini/impostazioni), persistente tra i frame
    help_md_cache: CommonMarkCache,
    // selettori per i totali-anno per dev nel footer
    selected_year: i32,                    // 0 = nessuno
    selected_category: Option<CategoryId>, // None = tutte
    // input toolbar
    new_worker: String,
    new_dev: String,
    new_category: String,
    new_milestone: String,
    // tipo scelto per la PROSSIMA milestone creata dal menù Aggiungi
    new_milestone_kind: MilestoneKind,
    // finestra di gestione milestone (elenco, colore, tipo, elimina)
    show_milestone_manager: bool,
    milestone_manager_just_opened: bool,
    // finestra di gestione "ghost": elenca TUTTI i worker (anche nascosti nel
    // footer o filtrati) con una spunta Ghost ciascuno.
    show_ghost_manager: bool,
    ghost_manager_just_opened: bool,
    // buffer di editing per nomi progetto ed effort dev
    name_buffers: HashMap<usize, String>,
    effort_buffers: HashMap<(usize, usize), String>,
    // buffer di editing per la % dichiarata dal dev (chiave: proj, dev)
    declared_buffers: HashMap<(usize, usize), String>,
    // includere le percentuali (presunta/dichiarata) nel PDF/SVG esportato;
    // scelta chiesta nelle dialog di export, ricordata tra un export e l'altro.
    export_progress_pct: bool,
    // finestra "Confronta/Importa progetto…": presente quando un file parallelo è
    // stato caricato per il confronto. Mentre è aperta l'autosave è sospeso.
    compare: Option<CompareState>,
}

impl UiState {
    /// Mostra una notifica in-app. Se lo stesso testo è già in elenco ne rinfresca
    /// solo l'istante (un errore che si ripete a ogni autosave non impila copie).
    pub(crate) fn toast(&mut self, level: ToastLevel, text: impl Into<String>) {
        let text = text.into();
        if let Some(t) = self.toasts.iter_mut().find(|t| t.text == text) {
            t.level = level;
            t.born = std::time::Instant::now();
            return;
        }
        self.toasts.push(Toast {
            level,
            text,
            born: std::time::Instant::now(),
        });
        // Tieni solo le più recenti: le vecchie escono dalla cima.
        while self.toasts.len() > MAX_TOASTS {
            self.toasts.remove(0);
        }
    }

    /// Errore: resta finché non lo si chiude.
    pub(crate) fn toast_error(&mut self, text: impl Into<String>) {
        self.toast(ToastLevel::Error, text);
    }

    /// Operazione riuscita (sparisce da sola in pochi secondi).
    pub(crate) fn toast_ok(&mut self, text: impl Into<String>) {
        self.toast(ToastLevel::Success, text);
    }

    /// Avviso: l'operazione non ha prodotto nulla, ma non è un errore.
    pub(crate) fn toast_warn(&mut self, text: impl Into<String>) {
        self.toast(ToastLevel::Warning, text);
    }

    /// Informazione (es. il file condiviso è cambiato ed è stato ricaricato).
    pub(crate) fn toast_info(&mut self, text: impl Into<String>) {
        self.toast(ToastLevel::Info, text);
    }
}

// ── Stato della finestra di confronto tra due file .ron ─────────────────────

/// Una coppia di progetti "stesso progetto" nei due file, individuata per
/// tripletta. Gli id coincidono (il file parallelo è una copia), ma li teniamo
/// entrambi per chiarezza.
pub(crate) struct ComparePair {
    pub tripletta: String,
    pub off: ProjectId, // id nel file ufficiale (self.app)
    pub par: ProjectId, // id nel file parallelo (other_app)
    /// Incluso nella vista di confronto impilata.
    pub selected: bool,
}

/// Fase della finestra: prima si scelgono i progetti, poi si confrontano.
#[derive(PartialEq, Eq)]
pub(crate) enum CompareStage {
    Select,
    View,
}

/// Stato completo del confronto: il file parallelo caricato in memoria, le coppie
/// di progetti in comune e lo scroll condiviso tra i due pannelli.
pub(crate) struct CompareState {
    pub other_app: App,
    pub other_path: String,
    pub pairs: Vec<ComparePair>,
    pub stage: CompareStage,
    // scroll condiviso tra pannello sinistro (ufficiale) e destro (parallelo)
    pub scroll_x: f32,
    pub scroll_y: f32,
}

// Scelta dell'utente nella finestra di conferma uscita.
enum ExitChoice {
    Save,
    Discard,
    Cancel,
}

// ── Azioni differite (applicate dopo il rendering) ──────────────────────────

pub(crate) enum Action {
    Save,
    Open,
    /// Apre la finestra di confronto: chiede un file .ron parallelo e carica lo
    /// stato in `UiState.compare`.
    OpenCompare,
    /// Copia un dev da un lato all'altro nella finestra di confronto.
    /// `to_official = true` → parallelo→ufficiale (import reale su `self.app`);
    /// `false` → ufficiale→parallelo (solo in memoria su `other_app`).
    CompareCopyDev {
        off: ProjectId,
        par: ProjectId,
        dev: DevId,
        to_official: bool,
    },
    /// Copia un intero progetto da un lato all'altro nella finestra di confronto.
    CompareCopyProject {
        off: ProjectId,
        par: ProjectId,
        to_official: bool,
    },
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
    SetDevDeclaredPct {
        proj: ProjectId,
        dev: DevId,
        pct: u8,
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
    /// Inserimento multiplo: assegna `effort` a ciascun worker di `workers` in
    /// `weeks` settimane consecutive a partire da `start` (inclusa), andando in
    /// avanti. Sovrascrive il valore eventualmente già presente.
    BulkFillEffort {
        proj: ProjectId,
        dev: DevId,
        start: WeekId,
        workers: Vec<WorkerId>,
        effort: Effort,
        weeks: usize,
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
    SetProjectNotes {
        proj: ProjectId,
        notes: HashMap<WeekId, String>,
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
    SetProjectClosed {
        proj: ProjectId,
        closed: bool,
    },
    SetWorkerMaxHours {
        worker: WorkerId,
        hours: u32,
    },
    SetWorkerGhost {
        worker: WorkerId,
        ghost: bool,
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
    ExportTrend,
    ExportPdfSelected {
        projects: Vec<ProjectId>,
    },
    ExportPdfProject {
        proj: ProjectId,
        devs: Vec<DevId>,
    },
    ExportSvgProject {
        proj: ProjectId,
        devs: Vec<DevId>,
    },
    GenerateMinuta {
        projects: Vec<ProjectId>,
        only_current: bool,
        only_with_notes: bool,
        worker_notes: bool,
    },
    CreateMilestone(String, MilestoneKind),
    SetMilestoneColor {
        milestone: MilestoneId,
        color: u32,
    },
    SetMilestoneKind {
        milestone: MilestoneId,
        kind: MilestoneKind,
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
    MoveEffort {
        proj: ProjectId,
        moves: Vec<(DevId, Vec<WeekId>)>,
        delta: i64,
        milestones: Vec<MilestoneId>,
        resolution: OverflowResolution,
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
    pub fn new(
        app: App,
        current_file: String,
        startup_error: Option<String>,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|s| s.interaction.tooltip_delay = 0.0);
        install_symbol_fallback(&cc.egui_ctx);

        let today = Utc::now().date_naive();
        let this_week = local_to_days(&primo_giorno_settimana_corrente(&today));

        // Scroll iniziale per centrare la settimana corrente (come nel main.rs Slint).
        // L'offset X tiene conto delle colonne di confine d'anno (più strette).
        let cols = columns_vec(&app, 0);
        let col_pos = cols.iter().position(|c| c.contains_week(this_week));
        let pending_scroll_x = match col_pos {
            Some(idx) if idx > 0 => {
                const INITIAL_WINDOW_WIDTH: f32 = 1024.0;
                let visible_width = INITIAL_WINDOW_WIDTH - LEFT_W;
                Some(centered_scroll_x(&cols, idx, COL_W, visible_width))
            }
            _ => None,
        };

        // Stato iniziale per il rilevamento di modifiche esterne al file.
        let base_ron = app.to_ron_string();
        let file_mtime = file_mtime_of(&current_file);

        // Errore di caricamento all'avvio + eventuali incoerenze di schema.
        let load_error = combine_issues(startup_error, app.validate());

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
                load_error,
                ..Default::default()
            },
        }
    }
}

/// Unisce un eventuale errore di caricamento e la lista di incoerenze di schema
/// in un unico messaggio (o `None` se non c'è nulla da segnalare).
fn combine_issues(err: Option<String>, warnings: Vec<String>) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(e) = err {
        parts.push(e);
    }
    if !warnings.is_empty() {
        parts.push(format!("Incoerenze nel file:\n• {}", warnings.join("\n• ")));
    }
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Nome file predefinito per gli export, con la **data corrente** (locale)
/// aggiunta in formato `AAAA_MM_GG`, es. `progetti_2026_07_13.pdf`.
fn dated_file_name(stem: &str, ext: &str) -> String {
    let date = chrono::Local::now().format("%Y_%m_%d");
    format!("{stem}_{date}.{ext}")
}

/// Dialog di salvataggio + scrittura del file, con l'esito riportato come
/// notifica in-app: il nome del file scritto in caso di successo, il messaggio di
/// errore (persistente) altrimenti. Se l'utente annulla il dialog non fa nulla.
fn save_export_dialog(
    state: &mut UiState,
    kind: &str,
    ext: &str,
    default_name: &str,
    data: impl AsRef<[u8]>,
) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(kind, &[ext])
        .set_file_name(default_name)
        .save_file()
    else {
        return;
    };
    let p = path.to_string_lossy().to_string();
    match std::fs::write(&p, data) {
        Ok(()) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.clone());
            state.toast_ok(format!("{kind} salvato: {name}"));
        }
        Err(e) => state.toast_error(format!("Errore scrittura {kind} '{p}': {e}")),
    }
}

/// Mostra il dialog di salvataggio PDF e scrive i byte nel file scelto.
fn save_pdf_dialog(state: &mut UiState, bytes: Vec<u8>, default_name: &str) {
    save_export_dialog(state, "PDF", "pdf", default_name, bytes);
}

/// Mostra il dialog di salvataggio SVG e scrive il contenuto nel file scelto.
fn save_svg_dialog(state: &mut UiState, svg: String, default_name: &str) {
    save_export_dialog(state, "SVG", "svg", default_name, svg);
}

/// Mostra il dialog di salvataggio Markdown e scrive il contenuto nel file scelto.
fn save_md_dialog(state: &mut UiState, md: String, default_name: &str) {
    save_export_dialog(state, "Markdown", "md", default_name, md);
}

/// mtime del file, se leggibile.
fn file_mtime_of(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Testo di notifica con orario corrente (per gli aggiornamenti esterni):
/// l'icona la mette il toast, qui serve solo l'ora.
fn notice_now(msg: &str) -> String {
    format!("{msg} ({})", chrono::Local::now().format("%H:%M"))
}

impl eframe::App for PjmApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions: Vec<Action> = Vec::new();
        {
            let app = &self.app;
            let state = &mut self.ui;

            // Resa in scala di grigi: impostata una volta per frame, prima di disegnare.
            set_bw_mode(state.bw_mode);

            // Tema chiaro/scuro: risolto a inizio frame. In Auto segue il tema del
            // sistema (macOS "Automatico" → chiaro di giorno, scuro la sera).
            let dark = match state.theme_pref {
                ThemePref::Dark => true,
                ThemePref::Light => false,
                ThemePref::Auto => {
                    ui.ctx().system_theme().unwrap_or(egui::Theme::Dark) == egui::Theme::Dark
                }
            };
            set_dark_theme(dark);
            // Allinea anche i widget nativi di egui (menù, popup, campi di testo).
            ui.ctx().set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            // Scorciatoie globali (Cmd su macOS, Ctrl altrove). Calcolate in anticipo
            // per non trattenere un borrow di `ui` durante i pannelli.
            let (key_s, key_f, key_g, key_d, key_p, key_t, key_j, shift, key_1, key_2, key_3) =
                ui.ctx().input(|i| {
                    let cmd = i.modifiers.command || i.modifiers.ctrl;
                    (
                        cmd && i.key_pressed(egui::Key::S),
                        cmd && i.key_pressed(egui::Key::F),
                        cmd && i.key_pressed(egui::Key::G),
                        cmd && i.key_pressed(egui::Key::D),
                        cmd && i.key_pressed(egui::Key::P),
                        cmd && i.key_pressed(egui::Key::T),
                        cmd && i.key_pressed(egui::Key::J),
                        i.modifiers.shift,
                        cmd && i.key_pressed(egui::Key::Num1),
                        cmd && i.key_pressed(egui::Key::Num2),
                        cmd && i.key_pressed(egui::Key::Num3),
                    )
                });
            if key_s {
                actions.push(Action::Save);
            }
            // Modalità progetti (non persistita): Ctrl+1 solo aperti, Ctrl+2 solo
            // chiusi, Ctrl+3 tutti.
            if key_1 {
                state.project_view = ProjectViewMode::Open;
            }
            if key_2 {
                state.project_view = ProjectViewMode::Closed;
            }
            if key_3 {
                state.project_view = ProjectViewMode::All;
            }
            // Ctrl+F / Ctrl+G / Ctrl+P / Ctrl+D aprono tutti la **stessa** dialog
            // "Filtri" (Workers + Progetti + Dev), portando il focus sulla ricerca
            // della propria colonna. A dialog già aperta la stessa scorciatoia fa
            // il toggle del "Select All" di quella colonna (Shift = deseleziona
            // tutti). Fa eccezione la coppia F/G, che condivide la colonna Workers:
            // premere l'altra commuta solo la modalità "settimana corrente".
            if key_f {
                if shift {
                    state.worker_filter = Some(HashSet::new()); // deseleziona tutti
                    state.worker_filter_current_week = false;
                } else if state.show_filters && !state.worker_filter_current_week {
                    state.worker_filter_toggle_all = true;
                } else if state.show_filters {
                    state.worker_filter_current_week = false; // commuta a "tutte le settimane"
                } else {
                    state.worker_filter_current_week = false;
                    open_filters(state, FilterPane::Workers);
                }
                focus_pane(state, FilterPane::Workers);
            }
            if key_g {
                if shift {
                    state.worker_filter = Some(HashSet::new()); // deseleziona tutti
                    state.worker_filter_current_week = true;
                } else if state.show_filters && state.worker_filter_current_week {
                    state.worker_filter_toggle_all = true;
                } else if state.show_filters {
                    state.worker_filter_current_week = true; // commuta a "settimana corrente"
                } else {
                    state.worker_filter_current_week = true;
                    open_filters(state, FilterPane::Workers);
                }
                focus_pane(state, FilterPane::Workers);
            }
            if key_d {
                if shift {
                    state.dev_filter = Some(HashSet::new()); // deseleziona tutti
                } else if state.show_filters {
                    state.dev_filter_toggle_all = true;
                } else {
                    open_filters(state, FilterPane::Devs);
                }
                focus_pane(state, FilterPane::Devs);
            }
            if key_p {
                if state.show_filters {
                    // dialog già aperta → toggle del "Select All" dei progetti
                    state.project_filter_toggle_all = true;
                } else {
                    open_filters(state, FilterPane::Projects);
                }
                focus_pane(state, FilterPane::Projects);
            }
            // Ctrl+J: azzera tutti i filtri (tutto selezionato). Non è un toggle —
            // ripremuto, anche a dialog aperta, riporta sempre allo stesso stato.
            if key_j {
                reset_all_filters(app, state, &mut actions);
            }
            // Ctrl+T: riporta la griglia sulla settimana di oggi, centrandola
            // (stesso criterio dello scroll iniziale). Rispetta zoom e vista
            // compatta, così la colonna trovata è quella davvero disegnata.
            if key_t {
                let level = if state.compact_mode {
                    0
                } else {
                    state.zoom_level
                };
                let cols = columns_vec(app, level);
                let wk = current_week_id().0 as i32;
                if let Some(idx) = cols.iter().position(|c| c.contains_week(wk)) {
                    let visible_w = (ui.available_width() - LEFT_W).max(1.0);
                    state.pending_scroll_x = Some(centered_scroll_x(
                        &cols,
                        idx,
                        col_w(state.compact_mode),
                        visible_w,
                    ));
                }
            }

            egui::TopBottomPanel::top("toolbar")
                .show_inside(ui, |ui| toolbar(ui, app, state, &mut actions));

            egui::TopBottomPanel::top("header")
                .frame(egui::Frame::NONE.fill(bg()))
                .show_inside(ui, |ui| header(ui, app, state));

            // Footer (worker / sovra) — nascosto in vista compatta.
            // La maniglia col triangolino permette di collassarlo/riaprirlo.
            if !state.compact_mode {
                if state.footer_hidden {
                    // Solo la maniglia: triangolino verso l'alto per riaprire.
                    egui::TopBottomPanel::bottom("footer_handle")
                        .frame(egui::Frame::NONE.fill(bg()))
                        .show_inside(ui, |ui| footer_handle(ui, state));
                } else {
                    egui::TopBottomPanel::bottom("footer")
                        .frame(egui::Frame::NONE.fill(bg()))
                        .show_inside(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            // Maniglia (triangolino verso il basso) in cima al footer.
                            footer_handle(ui, state);
                            footer(ui, app, state, &mut actions);
                        });
                }
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(bg()))
                .show_inside(ui, |ui| body(ui, app, state, &mut actions));

            note_editor_window(ui.ctx(), state, &mut actions);
            popup_window(ui.ctx(), app, state, &mut actions);
            bulk_fill_window(ui.ctx(), app, state, &mut actions);
            dev_manage_window(ui.ctx(), app, state, &mut actions);
            pdf_export_window(ui.ctx(), app, state, &mut actions);
            pdf_multi_export_window(ui.ctx(), app, state, &mut actions);
            minuta_window(ui.ctx(), app, state, &mut actions);
            help_window(ui.ctx(), state);
            confirm_del_dev_window(ui.ctx(), state, &mut actions);
            filters_window(ui.ctx(), app, state, &mut actions);
            milestone_manager_window(ui.ctx(), app, state, &mut actions);
            ghost_manager_window(ui.ctx(), app, state, &mut actions);
            closed_filter_window(ui.ctx(), app, state, &mut actions);
            move_dialog_window(ui.ctx(), app, state, &mut actions);
            saturation_window(ui.ctx(), app, state);
            compare_window(ui.ctx(), app, state, &mut actions);
            // Sopra a tutto: le notifiche in-app.
            toasts_layer(ui.ctx(), state);
        }

        for a in actions {
            self.apply(a);
        }

        let ctx = ui.ctx().clone();
        self.poll_background_errors();
        self.check_external_change(&ctx);
        self.conflict_window(&ctx);
        self.maybe_autosave(&ctx);
        self.load_error_window(&ctx);
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
                // Se il salvataggio fallisce **non** si esce: chiudere qui vorrebbe
                // dire perdere le modifiche mostrando un errore che nessuno legge.
                if let Err(e) = self.app.save(&self.ui.current_file) {
                    self.ui.toast_error(e);
                    self.ui.show_exit_confirm = false;
                    return;
                }
                // In uscita il push è bloccante: il thread di background verrebbe
                // ucciso dalla chiusura del processo prima di completare.
                crate::git_autosync::commit_and_push_blocking(&self.ui.current_file);
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

    /// Salva su disco e, se la cartella del file è un repo git, committa e pusha
    /// il file in background (non blocca la UI). Vedi `git_autosync`.
    /// Un errore di scrittura diventa una notifica in-app (persistente).
    /// Ritorna `false` se il salvataggio è fallito.
    fn save_to_disk(&mut self) -> bool {
        if let Err(e) = self.app.save(&self.ui.current_file) {
            self.ui.toast_error(e);
            return false;
        }
        crate::git_autosync::commit_and_push(&self.ui.current_file);
        true
    }

    /// Travasa in notifiche gli errori prodotti dai thread di background (per ora
    /// solo la sincronizzazione git, che non può toccare lo stato della UI).
    fn poll_background_errors(&mut self) {
        if let Some(msg) = crate::git_autosync::take_error() {
            self.ui.toast_error(msg);
        }
    }

    /// Aggiorna lo snapshot "base" e l'mtime dopo che lo stato è tornato
    /// coerente col disco (salvataggio o apertura).
    fn sync_baseline(&mut self) {
        self.ui.base_ron = self.app.to_ron_string();
        self.ui.file_mtime = file_mtime_of(&self.ui.current_file);
    }

    /// Salvataggio automatico: se ci sono modifiche non salvate e sono trascorsi
    /// almeno `AUTOSAVE_SECS` secondi dall'ultimo salvataggio, salva (con backup
    /// a rotazione e scrittura atomica). Non interviene mentre è in attesa la
    /// scelta su un conflitto con un collega.
    fn maybe_autosave(&mut self, ctx: &egui::Context) {
        const AUTOSAVE_SECS: f64 = 120.0;
        if self.ui.pending_reload.is_some() {
            return;
        }
        // Durante il confronto tra due file l'autosave è sospeso: le modifiche
        // (import di un dev) restano volontarie e vengono salvate solo dopo aver
        // chiuso la finestra di confronto.
        if self.ui.compare.is_some() {
            return;
        }
        let now = ctx.input(|i| i.time);
        if !self.ui.changed {
            // niente da salvare: mantieni il timer allineato ad "adesso"
            self.ui.last_save_time = now;
            return;
        }
        if now - self.ui.last_save_time >= AUTOSAVE_SECS {
            // Anche fallendo aggiorniamo il timer: l'errore è già notificato e non
            // ha senso ritentare a ogni frame.
            self.ui.last_save_time = now;
            if self.save_to_disk() {
                self.ui.changed = false;
                self.sync_baseline();
                self.ui.toast_ok(notice_now("Salvataggio automatico"));
            }
        }
    }

    /// Finestra che segnala un problema di caricamento o incoerenze nel file
    /// (validazione schema): invece di fallire in silenzio.
    fn load_error_window(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.ui.load_error.clone() else {
            return;
        };
        let mut close = false;
        egui::Window::new("Problema nel file")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.label(msg);
                ui.add_space(8.0);
                ui.separator();
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
        if close {
            self.ui.load_error = None;
        }
    }

    /// Il flag `enable` (filtro «Filtri ▸ Progetti…») è uno stato di sola
    /// visualizzazione **non persistito**: quando lo stato viene ricaricato o
    /// fuso da un aggiornamento del file condiviso, `enable` verrebbe azzerato
    /// (ricalcolato da `closed` in `from_ron_str`). Qui ripristiniamo le scelte
    /// locali per i progetti ancora presenti, così il filtro attivo non si
    /// resetta a ogni aggiornamento in background fatto da un collega.
    fn preserve_project_filter(&self, target: &mut App) {
        for id in target.projects.ids() {
            // Un progetto chiuso (nel nuovo stato) resta sempre non-enabled.
            if target.projects.is_closed(id) {
                target.projects.set_enable(id, Enable(false));
                continue;
            }
            // Progetto ancora presente nel mio stato ⇒ mantengo la mia scelta di
            // filtro. NB: NON richiamare `reset_enable_from_closed` qui, perché
            // rimetterebbe `enable = true` su tutti i progetti aperti, azzerando
            // il filtro. I progetti nuovi del collega restano come dal load
            // (enabled, cioè visibili).
            if self.app.projects.get(id).is_some() {
                let mine = self.app.projects.get_enable(&id);
                target.projects.set_enable(id, mine);
            }
        }
    }

    /// Adotta integralmente lo stato del disco (ricarica automatica: nessuna
    /// modifica locale da preservare).
    fn adopt_disk(&mut self, mut theirs: App, mtime: std::time::SystemTime) {
        theirs.compute_sovra();
        self.preserve_project_filter(&mut theirs);
        self.ui.base_ron = theirs.to_ron_string();
        self.app = theirs;
        self.ui.file_mtime = Some(mtime);
        self.ui.changed = false;
        self.ui.name_buffers.clear();
        self.ui.effort_buffers.clear();
        self.ui.declared_buffers.clear();
        self.ui.editing = None;
        self.ui
            .toast_info(notice_now("Il file è stato aggiornato da un collega"));
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
            let mut merged = outcome.app;
            self.preserve_project_filter(&mut merged);
            self.app = merged;
            self.ui.base_ron = theirs.to_ron_string();
            self.ui.file_mtime = Some(disk_mtime);
            self.ui
                .toast_info(notice_now("Uniti i cambiamenti di un collega"));
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
                    let mut merged = p.merged; // sovra già ricalcolata nel merge
                    self.preserve_project_filter(&mut merged);
                    self.app = merged;
                    self.ui.base_ron = p.theirs.to_ron_string();
                    self.ui.changed = true;
                    self.ui.toast_info(notice_now(
                        "Modifiche del collega unite (conflitti: tenute le tue)",
                    ));
                }
            }
            Some(false) => {
                if let Some(p) = self.ui.pending_reload.take() {
                    let mut theirs = p.theirs;
                    theirs.compute_sovra();
                    self.ui.base_ron = theirs.to_ron_string();
                    self.preserve_project_filter(&mut theirs);
                    self.app = theirs;
                    self.ui.changed = false;
                    self.ui.name_buffers.clear();
                    self.ui.effort_buffers.clear();
                    self.ui.declared_buffers.clear();
                    self.ui.editing = None;
                    self.ui.toast_info(notice_now(
                        "Ricaricato dal disco (le tue modifiche sono state scartate)",
                    ));
                }
            }
            None => {}
        }
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::Save => {
                // Solo un salvataggio riuscito azzera "modificato": altrimenti il
                // file resterebbe segnato come allineato al disco senza esserlo.
                if self.save_to_disk() {
                    self.ui.changed = false;
                    self.sync_baseline();
                    self.ui.toast_ok(notice_now("File salvato"));
                }
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
                            self.ui.declared_buffers.clear();
                            self.ui.editing = None;
                            self.app.compute_sovra();
                            self.ui.changed = false;
                            self.sync_baseline();
                            // Validazione schema: segnala riferimenti pendenti.
                            self.ui.load_error = combine_issues(None, self.app.validate());
                        }
                        Err(e) => {
                            self.ui.load_error = Some(format!("Impossibile aprire «{path}»:\n{e}"));
                        }
                    }
                }
            }
            Action::OpenCompare => {
                if let Some(path_buf) = rfd::FileDialog::new()
                    .add_filter("RON files", &["ron"])
                    .pick_file()
                {
                    let path = path_buf.to_string_lossy().to_string();
                    match App::load(&path) {
                        Ok(mut loaded) => {
                            loaded.compute_sovra();
                            let pairs = common_project_pairs(&self.app, &loaded);
                            if pairs.is_empty() {
                                self.ui.load_error = Some(format!(
                                    "Nessun progetto diverso: i progetti in comune (per \
                                     tripletta) col file «{path}» sono identici (oppure non \
                                     ce ne sono in comune)."
                                ));
                            } else {
                                self.ui.compare = Some(CompareState {
                                    other_app: loaded,
                                    other_path: path,
                                    pairs,
                                    stage: CompareStage::Select,
                                    scroll_x: 0.0,
                                    scroll_y: 0.0,
                                });
                            }
                        }
                        Err(e) => {
                            self.ui.load_error = Some(format!(
                                "Impossibile aprire «{path}» per il confronto:\n{e}"
                            ));
                        }
                    }
                }
            }
            Action::CompareCopyDev {
                off,
                par,
                dev,
                to_official,
            } => {
                let Some(cmp) = self.ui.compare.as_mut() else {
                    return;
                };
                if to_official {
                    // parallelo → ufficiale (import reale): copia il SingleDev dal
                    // progetto parallelo a quello ufficiale.
                    let src = cmp.other_app.projects.get_single_dev(par, dev).cloned();
                    copy_dev_into(&mut self.app, off, dev, src);
                    self.mark_changed();
                } else {
                    // ufficiale → parallelo (solo in memoria).
                    let src = self.app.projects.get_single_dev(off, dev).cloned();
                    copy_dev_into(&mut cmp.other_app, par, dev, src);
                    cmp.other_app.compute_sovra();
                }
            }
            Action::CompareCopyProject {
                off,
                par,
                to_official,
            } => {
                let Some(cmp) = self.ui.compare.as_mut() else {
                    return;
                };
                if to_official {
                    // parallelo → ufficiale (import reale dell'intero progetto).
                    let src = cmp.other_app.clone();
                    copy_project_into(&mut self.app, off, &src, par);
                    self.mark_changed();
                } else {
                    // ufficiale → parallelo (solo in memoria).
                    let src = self.app.clone();
                    copy_project_into(&mut cmp.other_app, par, &src, off);
                    cmp.other_app.compute_sovra();
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
            Action::SetDevDeclaredPct { proj, dev, pct } => {
                // Registra nello storico con la settimana corrente; marca come
                // modificato solo se lo storico è davvero cambiato.
                if self
                    .app
                    .projects
                    .set_dev_declared_pct(proj, dev, current_week_id(), pct)
                {
                    self.mark_changed();
                }
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
            Action::BulkFillEffort {
                proj,
                dev,
                start,
                workers,
                effort,
                weeks,
            } => {
                if bulk_fill_effort(&mut self.app, proj, dev, start, &workers, effort, weeks) {
                    self.mark_changed();
                }
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
            Action::SetProjectNotes { proj, notes } => {
                self.app.projects.set_notes(proj, notes);
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
            Action::SetProjectClosed { proj, closed } => {
                self.app.projects.set_closed(proj, closed);
                self.mark_changed();
            }
            Action::SetWorkerMaxHours { worker, hours } => {
                self.app.workers.set_max_hours(worker, hours);
                self.mark_changed();
            }
            Action::SetWorkerGhost { worker, ghost } => {
                self.app.workers.set_ghost(worker, ghost);
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
            Action::ExportPdf => {
                // Progetti "visibili" = quelli mostrati nel corpo centrale
                // (abilitati + modalità Vista corrente). Con un solo progetto apri
                // la dialog di selezione/ordinamento dei dev; con più di uno apri
                // la dialog di selezione dei progetti; con nessuno non c'è nulla da
                // esportare.
                let eligible: Vec<ProjectId> = body_projects(&self.app, self.ui.project_view);
                match eligible[..] {
                    [] => self
                        .ui
                        .toast_warn("Nessun progetto visibile: PDF non creato."),
                    [proj] => {
                        let entries = self
                            .app
                            .projects
                            .list_devs(proj)
                            .into_iter()
                            .map(|d| (d, true))
                            .collect();
                        self.ui.pdf_export = Some(PdfExport { proj, entries });
                    }
                    _ => {
                        let entries = eligible.into_iter().map(|id| (id, true)).collect();
                        self.ui.pdf_multi_export = Some(PdfMultiExport { entries });
                    }
                }
            }
            Action::ExportTrend => {
                // PDF andamento nel tempo: una pagina per ogni progetto visibile
                // (abilitato + modalità Vista corrente) con dati di avanzamento.
                let visible = body_projects(&self.app, self.ui.project_view);
                match crate::pdf_export::build_trend_pdf(&self.app, &visible) {
                    Some(bytes) => {
                        save_pdf_dialog(&mut self.ui, bytes, &dated_file_name("andamento", "pdf"))
                    }
                    None => self
                        .ui
                        .toast_warn("Nessun progetto con dati di avanzamento: PDF non creato."),
                }
            }
            Action::ExportPdfSelected { projects } => {
                crate::pdf_export::set_show_pct(self.ui.export_progress_pct);
                match crate::pdf_export::build_pdf_selected(
                    &self.app,
                    &projects,
                    self.ui.bar_format,
                ) {
                    None => self.ui.toast_warn(
                        "Nessun progetto selezionato con inizio e fine: PDF non creato.",
                    ),
                    Some(bytes) => {
                        save_pdf_dialog(&mut self.ui, bytes, &dated_file_name("progetti", "pdf"))
                    }
                }
            }
            Action::ExportPdfProject { proj, devs } => {
                crate::pdf_export::set_show_pct(self.ui.export_progress_pct);
                match crate::pdf_export::build_pdf_project(
                    &self.app,
                    proj,
                    &devs,
                    self.ui.bar_format,
                ) {
                    None => self
                        .ui
                        .toast_warn("Progetto senza inizio/fine: PDF non creato."),
                    Some(bytes) => {
                        save_pdf_dialog(&mut self.ui, bytes, &dated_file_name("progetto", "pdf"))
                    }
                }
            }
            Action::ExportSvgProject { proj, devs } => {
                crate::pdf_export::set_show_pct(self.ui.export_progress_pct);
                match crate::pdf_export::build_svg_project(
                    &self.app,
                    proj,
                    &devs,
                    self.ui.bar_format,
                ) {
                    None => self
                        .ui
                        .toast_warn("Progetto senza inizio/fine: SVG non creato."),
                    Some(svg) => {
                        save_svg_dialog(&mut self.ui, svg, &dated_file_name("grafico", "svg"))
                    }
                }
            }
            Action::GenerateMinuta {
                projects,
                only_current,
                only_with_notes,
                worker_notes,
            } => {
                let md = build_minuta(
                    &self.app,
                    &projects,
                    only_current,
                    only_with_notes,
                    worker_notes,
                );
                save_md_dialog(&mut self.ui, md, &dated_file_name("minuta", "md"));
            }
            Action::CreateMilestone(name, kind) => {
                self.app.milestones.add_with_kind(&name, kind);
                self.mark_changed();
            }
            Action::SetMilestoneColor { milestone, color } => {
                self.app.milestones.set_color(milestone, color);
                self.mark_changed();
            }
            Action::SetMilestoneKind { milestone, kind } => {
                self.app.milestones.set_kind(milestone, kind);
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
            Action::MoveEffort {
                proj,
                moves,
                delta,
                milestones,
                resolution,
            } => {
                self.app
                    .projects
                    .move_effort(proj, &moves, delta, &milestones, resolution);
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
#[derive(Clone, PartialEq)]
pub(crate) enum Col {
    /// Una o più settimane mergiate (len 1 al livello zoom 0). L'effort mostrato
    /// è la somma delle settimane del gruppo; le colonne con len > 1 sono sola lettura.
    Weeks(Vec<i32>),
    YearEnd(i32),
}

impl Col {
    /// True se il gruppo contiene la settimana `w`.
    fn contains_week(&self, w: i32) -> bool {
        matches!(self, Col::Weeks(ws) if ws.contains(&w))
    }
}

/// Numero di settimane mergiate per livello di zoom.
fn zoom_group(level: u8) -> usize {
    match level {
        1 => 2,
        2 => 4,
        _ => 1,
    }
}

/// Larghezza (px) della colonna di confine d'anno: più stretta di una settimana,
/// quel tanto che basta per il titolo "Effort residuo" e i valori.
const BOUNDARY_W: f32 = 64.0;

/// Larghezza di una colonna: le settimane usano `cw`, il confine è più stretto.
fn col_width(c: &Col, cw: f32) -> f32 {
    match c {
        Col::YearEnd(_) => BOUNDARY_W,
        Col::Weeks(_) => cw,
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
fn columns_vec(app: &App, level: u8) -> Vec<Col> {
    let g = zoom_group(level);
    let weeks = weeks_vec(app);
    let mut cols = Vec::with_capacity(weeks.len() + 2);
    let mut i = 0;
    while i < weeks.len() {
        // Raggruppa fino a `g` settimane consecutive dello stesso anno: un gruppo
        // non attraversa mai il confine d'anno (la colonna gialla resta separata).
        let year = days_to_local(weeks[i]).year();
        let mut group = Vec::new();
        while i < weeks.len() && group.len() < g && days_to_local(weeks[i]).year() == year {
            group.push(weeks[i]);
            i += 1;
        }
        cols.push(Col::Weeks(group));
        // Confine d'anno tra questo gruppo e il prossimo, se cambia anno.
        if i < weeks.len() {
            let next_year = days_to_local(weeks[i]).year();
            // di norma un solo confine; il ciclo copre eventuali salti d'anno.
            for y in year..next_year {
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

/// Offset X che **centra** la colonna `idx` in una vista larga `visible_width`
/// (mai negativo). Usato dallo scroll iniziale e dal salto a oggi (Ctrl+T).
pub(crate) fn centered_scroll_x(cols: &[Col], idx: usize, cw: f32, visible_width: f32) -> f32 {
    let col_center = col_x_offset(cols, idx, cw) + cw / 2.0;
    (col_center - visible_width / 2.0).max(0.0)
}

type Filter = Option<HashSet<String>>;

/// Filtro dev (Ctrl+D): `None` = nessun filtro, `Some(set)` = solo questi dev.
/// I dev sono identificati per `DevId` (il nome è solo l'etichetta a video).
pub(crate) type DevFilter = Option<HashSet<DevId>>;

/// Predicato unico del filtro dev, usato da `project_layout` (unica sorgente di
/// griglia + colonna sinistra): il dev è mostrato se è selezionato **e** ha
/// almeno un worker assegnato in questo progetto, **anche con effort 0**
/// (`has_any_worker`); resta nascosto solo il dev aggiunto e mai compilato.
/// Senza filtro attivo non nasconde nulla.
pub(crate) fn dev_shown(app: &App, proj: ProjectId, dev: DevId, filter: &DevFilter) -> bool {
    match filter {
        None => true,
        Some(set) => {
            set.contains(&dev)
                && app
                    .projects
                    .get_single_dev(proj, dev)
                    .is_some_and(|sd| sd.has_any_worker())
        }
    }
}

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

/// Altezza dell'area interna (celle) di un blocco dev.
/// - compatta: 1 riga (barra)
/// - mergiata (zoom): 2 righe (cumulativo + somma), i nomi worker spariscono
/// - normale: 1 riga cumulativo + `max_rows` righe worker
fn dev_inner_h(max_rows: usize, compact: bool, merged: bool) -> f32 {
    if compact {
        ROW_H
    } else if merged {
        2.0 * ROW_H
    } else {
        (max_rows as f32 + 1.0) * ROW_H
    }
}

fn dev_block_height(max_rows: usize, compact: bool, merged: bool) -> f32 {
    DEV_BORDER + dev_inner_h(max_rows, compact, merged) + DEV_BORDER
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

/// Checkbox "Select All" condivisa dai dialog con un elenco selezionabile.
/// `currently_all` deve essere `true` solo se ogni elemento attualmente elencato
/// è già selezionato. Ritorna `Some(nuovo_stato)` quando l'utente clicca la
/// checkbox — il chiamante applica `nuovo_stato` a tutti gli elementi — altrimenti
/// `None`.
fn select_all_checkbox(ui: &mut egui::Ui, currently_all: bool) -> Option<bool> {
    let mut all = currently_all;
    ui.checkbox(&mut all, "Select All").changed().then_some(all)
}

/// Voci del selettore del tipo milestone (traguardo / trigger di evento),
/// condivise dalla combo e dal sottomenù. Ritorna `true` se il tipo cambia.
fn milestone_kind_items(ui: &mut egui::Ui, kind: &mut MilestoneKind) -> bool {
    let mut changed = false;
    for k in [MilestoneKind::Goal, MilestoneKind::Trigger] {
        if ui
            .selectable_label(*kind == k, k.label())
            .on_hover_text(match k {
                MilestoneKind::Goal => "Traguardo: bandierina nell'export",
                MilestoneKind::Trigger => "Trigger di evento: fulmine nell'export",
            })
            .clicked()
            && *kind != k
        {
            *kind = k;
            changed = true;
        }
    }
    changed
}

/// Selettore del tipo milestone **a tendina**, per le finestre (gestore
/// «Filtri ▸ Milestone…»). Ritorna `true` quando l'utente cambia il tipo.
///
/// `id_salt` distingue le istanze: nel gestore c'è una combo per riga, quindi
/// l'id deve dipendere dalla milestone o le tendine si aprirebbero tutte insieme.
///
/// **Non usarlo dentro un menù della toolbar**: la tendina della combo vive in
/// un layer suo, il menù la considera un click "fuori" e si chiude prima ancora
/// che la voce venga selezionata (verificato su egui 0.34). Dentro un menù si
/// usa `milestone_kind_submenu`.
pub(crate) fn milestone_kind_combo(
    ui: &mut egui::Ui,
    label: &str,
    id_salt: impl std::hash::Hash,
    kind: &mut MilestoneKind,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if !label.is_empty() {
            ui.label(label);
        }
        egui::ComboBox::from_id_salt(("milestone_kind", id_salt))
            .selected_text(kind.label())
            .width(MILESTONE_KIND_COMBO_W)
            .show_ui(ui, |ui| {
                changed = milestone_kind_items(ui, kind);
            });
    });
    changed
}

/// Stesso selettore per i **menù** della toolbar: un sottomenù «Tipo: … ⏵»
/// (un controllo solo, come la combo) che al contrario della combo è gestito da
/// egui come parte del menù, quindi scegliere una voce **non chiude la tendina**
/// e si può proseguire scrivendo il nome.
pub(crate) fn milestone_kind_submenu(ui: &mut egui::Ui, kind: &mut MilestoneKind) -> bool {
    let mut changed = false;
    egui::containers::menu::SubMenuButton::new(format!("Tipo: {}", kind.label()))
        .config(
            egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
        )
        .ui(ui, |ui| {
            changed = milestone_kind_items(ui, kind);
        });
    changed
}

/// Larghezza della combo del tipo milestone: tiene "Traguardo"/"Trigger" senza
/// che la tendina cambi larghezza tra le due voci.
const MILESTONE_KIND_COMBO_W: f32 = 78.0;

/// "Select All" + elenco scrollabile di progetti con checkbox, condiviso dai
/// dialog che selezionano un sottoinsieme di progetti. `entries` sono coppie
/// `(progetto, selezionato)` mutate in place.
fn project_checklist(ui: &mut egui::Ui, app: &App, entries: &mut [(ProjectId, bool)]) {
    // Select All in cima.
    let currently_all = !entries.is_empty() && entries.iter().all(|(_, s)| *s);
    if let Some(v) = select_all_checkbox(ui, currently_all) {
        for e in entries.iter_mut() {
            e.1 = v;
        }
    }
    ui.separator();

    // La lista usa lo spazio effettivamente disponibile nella finestra, meno un
    // margine per i controlli che restano SOTTO (separatore + bottoni, e nella
    // minuta i radio delle note). Con pochi progetti si restringe al contenuto
    // (`auto_shrink` verticale) evitando un riquadro vuoto.
    const BOTTOM_RESERVE: f32 = 140.0;
    let max_h = (ui.available_height() - BOTTOM_RESERVE).max(120.0);
    egui::ScrollArea::vertical()
        .max_height(max_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (proj, sel) in entries.iter_mut() {
                ui.checkbox(sel, minuta_project_label(app, *proj));
            }
        });
}

/// Testo del manuale d'uso, incorporato a compile-time da `docs/MANUALE.md`.
const MANUAL_MD: &str = include_str!("../../docs/MANUALE.md");

// Righe minime occupate dal nome progetto (campo multiriga; può crescere).
const NAME_ROWS: usize = 2;

/// Layout condiviso da colonna sinistra e griglia: stessi progetti, stessi dev,
/// stesse altezze. Garantisce che le due colonne non possano divergere.
pub(crate) struct ProjLayout {
    proj: ProjectId,
    name: String,
    devs: Vec<(DevId, usize)>, // (dev, max_rows)
    proj_h: f32,
}

/// Scostamento dei triangoli di stato dagli angoli, verso il centro cella,
/// così ferie (alto-sx) e malattia (basso-sx) non si toccano.
const STATUS_TRI_INSET: f32 = 3.0;

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
        .unwrap_or(text())
}

/// Scrive `effort` per ognuno dei `workers` in `weeks` settimane consecutive a
/// partire da `start` (inclusa), andando in avanti. Sovrascrive il valore già
/// presente per quel worker e si ferma alla fine della griglia (`end_week`).
/// Ritorna `true` se ha scritto qualcosa.
fn bulk_fill_effort(
    app: &mut App,
    proj: ProjectId,
    dev: DevId,
    start: WeekId,
    workers: &[WorkerId],
    effort: Effort,
    weeks: usize,
) -> bool {
    if workers.is_empty() || weeks == 0 {
        return false;
    }
    let mut written = false;
    for i in 0..weeks {
        let w = WeekId(start.0 + i * WEEK_STEP);
        if w > app.end_week {
            break;
        }
        for wid in workers {
            app.projects.add_effort(proj, dev, w, *wid, effort);
            written = true;
        }
    }
    written
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Renderizza `project_checklist` con `n` progetti dentro una finestra grande
    /// come `screen_h` px e restituisce l'altezza verticale consumata dalla lista.
    fn checklist_height(n: usize, screen_h: f32) -> f32 {
        let app = App::new();
        let mut entries: Vec<(ProjectId, bool)> = (0..n).map(|i| (ProjectId(i), false)).collect();

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1000.0, screen_h),
        ));

        let consumed = Cell::new(0.0);
        // Due frame: la prima registra le aree, la seconda misura a layout stabile.
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| {
                egui::Window::new("test").show(ctx, |ui| {
                    let before = ui.cursor().top();
                    project_checklist(ui, &app, &mut entries);
                    consumed.set(ui.cursor().top() - before);
                });
            });
        }
        consumed.get()
    }

    #[test]
    fn project_checklist_shrinks_and_caps() {
        let screen_h = 400.0;
        let h_empty = checklist_height(0, screen_h);
        let h_one = checklist_height(1, screen_h);
        let h_many = checklist_height(50, screen_h);

        // Nessun panic (arrivare qui basta) e la lista non cresce illimitata:
        // con 50 progetti resta ampiamente sotto le ~50 righe che occuperebbe
        // senza cap, e comunque entro l'altezza dello schermo.
        assert!(
            h_many <= screen_h,
            "lista non limitata: {h_many} > schermo {screen_h}"
        );
        assert!(h_many < 500.0, "cap non applicato: {h_many}");

        // auto_shrink: con pochi progetti la lista è più bassa che con molti.
        assert!(
            h_one < h_many,
            "auto_shrink non attivo: 1 progetto ({h_one}) >= 50 ({h_many})"
        );
        // Con zero progetti la lista è comunque disegnata (Select All + area),
        // quindi non nulla ma piccola.
        assert!(h_empty > 0.0 && h_empty <= h_one + 40.0, "vuota: {h_empty}");
    }

    /// L'inserimento multiplo (tasto destro su cella vuota) scrive a tutti i
    /// worker scelti per N settimane consecutive, sovrascrive quello che c'era
    /// e non esce dalla griglia.
    #[test]
    fn bulk_fill_writes_all_workers_for_n_weeks() {
        let mut app = App::new();
        let w1 = app.workers.add("Alice");
        let w2 = app.workers.add("Bob");
        let dev = app.devs.add("Frontend");
        let proj = app.projects.add("Progetto", Some("A-B-C"), None);

        let start = app.start_week;
        // Valore preesistente che deve essere sovrascritto.
        app.projects.add_effort(proj, dev, start, w1, Effort(3));

        assert!(bulk_fill_effort(
            &mut app,
            proj,
            dev,
            start,
            &[w1, w2],
            Effort(8),
            3
        ));

        for i in 0..3 {
            let w = WeekId(start.0 + i * WEEK_STEP);
            let sew = app.projects.get_single_dev(proj, dev).unwrap().get_all(w);
            let sew = sew.unwrap_or_else(|| panic!("settimana {i} non scritta"));
            for wid in [w1, w2] {
                assert_eq!(sew.worker_id.get(&wid).unwrap().get_effort(), Effort(8));
            }
        }
        // La quarta settimana resta intatta.
        assert!(
            app.projects
                .get_single_dev(proj, dev)
                .unwrap()
                .get_all(WeekId(start.0 + 3 * WEEK_STEP))
                .is_none()
        );

        // Oltre la fine della griglia non scrive nulla.
        let after_end = WeekId(app.end_week.0 + WEEK_STEP);
        assert!(!bulk_fill_effort(
            &mut app,
            proj,
            dev,
            after_end,
            &[w1],
            Effort(8),
            5
        ));
        // Selezione vuota o zero settimane: nessuna scrittura.
        assert!(!bulk_fill_effort(
            &mut app,
            proj,
            dev,
            start,
            &[],
            Effort(8),
            3
        ));
        assert!(!bulk_fill_effort(
            &mut app,
            proj,
            dev,
            start,
            &[w1],
            Effort(8),
            0
        ));
    }

    /// La dialog dell'inserimento multiplo si disegna (due frame, come le altre
    /// prove di UI) senza panic e resta aperta finché non si conferma/annulla.
    #[test]
    fn bulk_fill_window_renders() {
        let mut app = App::new();
        app.workers.add("Alice");
        let dev = app.devs.add("Frontend");
        let proj = app.projects.add("Progetto", Some("A-B-C"), None);

        let mut state = UiState::default();
        state.bulk_fill = Some(BulkFill {
            proj,
            dev,
            week: app.start_week.0 as i32,
            search: String::new(),
            selected: HashSet::new(),
            effort_text: "8".to_string(),
            weeks_text: "3".to_string(),
        });

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1000.0, 800.0),
        ));
        for _ in 0..2 {
            let mut actions: Vec<Action> = Vec::new();
            let _ = ctx.run(input.clone(), |ctx| {
                bulk_fill_window(ctx, &app, &mut state, &mut actions);
            });
            assert!(
                actions.is_empty(),
                "nessuna azione senza click su Inserisci"
            );
        }
        assert!(state.bulk_fill.is_some(), "la dialog deve restare aperta");
    }

    #[test]
    fn manual_version_is_dynamic_and_toc_parses() {
        // Il file usa il segnaposto, non un numero fisso, così non invecchia.
        assert!(
            MANUAL_MD.contains("{{VERSION}}"),
            "il manuale deve usare il segnaposto {{{{VERSION}}}}"
        );
        let manual = MANUAL_MD.replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
        assert!(!manual.contains("{{VERSION}}"));
        assert!(manual.contains(env!("CARGO_PKG_VERSION")));

        let sections = manual_sections(&manual);
        // Esattamente una sezione "## Indice", sostituita in-app dalla colonna.
        assert_eq!(sections.iter().filter(|s| is_index_section(s)).count(), 1);
        // I capitoli navigabili nell'indice laterale sono i 20 numerati.
        let chapters: Vec<&str> = sections.iter().filter_map(|s| chapter_title(s)).collect();
        assert_eq!(
            chapters.len(),
            20,
            "attesi 20 capitoli, trovati {}",
            chapters.len()
        );
        assert!(chapters[0].starts_with("1. "), "primo: {}", chapters[0]);
        assert!(chapters[19].starts_with("20. "), "ultimo: {}", chapters[19]);
        // "Indice" non deve comparire tra i capitoli navigabili.
        assert!(!chapters.iter().any(|c| *c == "Indice"));
    }

    #[test]
    fn body_projects_respects_view_mode_and_closed() {
        let mut app = App::new();
        // aperto e visibile
        let open = app.projects.add("Aperto", Some("AAA"), None);
        // aperto ma nascosto dal filtro «Progetti…» (enable = false)
        let hidden = app.projects.add("Nascosto", Some("BBB"), None);
        app.projects.set_enable(hidden, Enable(false));
        // chiuso (set_closed forza enable = false)
        let closed = app.projects.add("Chiuso", Some("CCC"), None);
        app.projects.set_closed(closed, true);

        // ProjectId non implementa Debug: confronto sugli id interni.
        let ids = |v: Vec<ProjectId>| v.into_iter().map(|p| p.0).collect::<Vec<_>>();

        // Solo aperti: solo l'aperto e visibile.
        assert_eq!(
            ids(body_projects(&app, ProjectViewMode::Open)),
            vec![open.0],
            "Open deve mostrare solo l'aperto visibile"
        );

        // Solo chiusi: solo il chiuso, malgrado enable = false.
        assert_eq!(
            ids(body_projects(&app, ProjectViewMode::Closed)),
            vec![closed.0],
            "Closed deve mostrare il progetto chiuso"
        );

        // Tutti: aperto visibile + chiuso; l'aperto nascosto resta escluso.
        assert_eq!(
            ids(body_projects(&app, ProjectViewMode::All)),
            vec![open.0, closed.0],
            "All: aperto visibile + chiuso"
        );
    }

    /// Centro del testo `needle` disegnato nel frame, cercato tra le shape in
    /// uscita: evita di indovinare le coordinate dei widget nei test di input.
    fn text_pos(out: &egui::FullOutput, needle: &str) -> Option<egui::Pos2> {
        out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == needle => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
    }

    /// Accoda gli eventi di un click "realistico" su `pos`: hover in un frame,
    /// pressione nel successivo, rilascio in quello dopo. Un click compresso in
    /// un frame solo non riproduce il comportamento di menù e tendine.
    fn queue_click(pending: &mut Vec<Vec<egui::Event>>, pos: egui::Pos2) {
        let btn = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        pending.push(vec![egui::Event::PointerMoved(pos)]);
        pending.push(vec![btn(true)]);
        pending.push(vec![btn(false)]);
    }

    /// Il selettore del tipo milestone nel menù «Aggiungi» **non deve chiudere
    /// il menù**: si sceglie il tipo e si continua a scrivere il nome nello
    /// stesso menù. Vale con il sottomenù «Tipo: … ⏵»; con una `ComboBox` no —
    /// la sua tendina sta in un altro layer, il menù la legge come click "fuori"
    /// e si chiude prima ancora che la voce venga selezionata (egui 0.34).
    #[test]
    fn milestone_kind_submenu_keeps_the_add_menu_open() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(900.0, 600.0),
        ));

        let menu_btn = Cell::new(egui::Rect::NOTHING);
        let sub_btn = Cell::new(egui::Rect::NOTHING);
        let menu_open = Cell::new(false);
        let mut kind = MilestoneKind::Goal;
        let mut pending: Vec<Vec<egui::Event>> = Vec::new();

        for step in 0..20 {
            let mut inp = input.clone();
            if !pending.is_empty() {
                inp.events = pending.remove(0);
            }
            menu_open.set(false);
            let out = ctx.run(inp, |ctx| {
                egui::TopBottomPanel::top("t").show(ctx, |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        // Stessa configurazione del menù «Aggiungi» in toolbar.rs.
                        let (btn, _) = egui::containers::menu::MenuButton::new("Aggiungi")
                            .config(
                                egui::containers::menu::MenuConfig::new()
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                            )
                            .ui(ui, |ui| {
                                menu_open.set(true);
                                milestone_kind_submenu(ui, &mut kind);
                                sub_btn.set(ui.min_rect());
                            });
                        menu_btn.set(btn.rect);
                    });
                });
            });

            // Posizione della voce "Trigger": si cerca il testo disegnato, così
            // il test non dipende da coordinate indovinate a mano.
            let trigger_item = text_pos(&out, "Trigger");

            match step {
                0 => queue_click(&mut pending, menu_btn.get().center()),
                // hover sul sottomenù (si apre al passaggio del mouse)
                5 => {
                    let p = sub_btn.get().center();
                    pending.push(vec![egui::Event::PointerMoved(p)]);
                    pending.push(vec![egui::Event::PointerMoved(p)]);
                }
                9 => {
                    let p = trigger_item.expect("il sottomenù aperto disegna la voce «Trigger»");
                    queue_click(&mut pending, p);
                }
                _ => {}
            }
        }

        assert_eq!(
            kind,
            MilestoneKind::Trigger,
            "la voce del sottomenù deve cambiare il tipo"
        );
        assert!(
            menu_open.get(),
            "dopo la scelta il menù «Aggiungi» deve restare aperto per digitare il nome"
        );
    }

    /// Le icone del tipo milestone (bandierina / fulmine) devono essere davvero
    /// disegnabili con i font caricati dall'app: `has_glyph` è false quando il
    /// carattere finirebbe come tofu. Guardia contro un cambio di font.
    #[test]
    fn milestone_kind_icons_are_renderable() {
        let ctx = egui::Context::default();
        install_symbol_fallback(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let font = egui::FontId::proportional(14.0);
        for kind in [MilestoneKind::Goal, MilestoneKind::Trigger] {
            assert!(
                ctx.fonts_mut(|f| f.has_glyphs(&font, kind.icon())),
                "l'icona di {} ({}) non è disegnabile coi font caricati",
                kind.label(),
                kind.icon()
            );
        }
        assert_ne!(
            MilestoneKind::Goal.icon(),
            MilestoneKind::Trigger.icon(),
            "traguardo e trigger devono avere icone diverse"
        );
    }

    #[test]
    fn milestone_bands_fit_column_width() {
        // Colonna normale: ci stanno tutte le milestone presenti.
        assert_eq!(milestone_bands_shown(COL_W, 1), 1);
        assert_eq!(milestone_bands_shown(COL_W, 3), 3);
        // Non se ne inventano più di quelle collocate nella settimana.
        assert_eq!(milestone_bands_shown(COL_W, 0), 0);
        // Vista compatta (25 px): al massimo 6 bande da 4 px.
        assert_eq!(milestone_bands_shown(COMPACT_W, 4), 4);
        assert_eq!(milestone_bands_shown(COMPACT_W, 10), 6);
        // Colonna più stretta della banda minima: almeno una banda, sempre.
        assert_eq!(milestone_bands_shown(3.0, 5), 1);
    }

    #[test]
    fn multiple_milestones_can_share_a_week() {
        // Il modello dati ammette più milestone nella stessa settimana: la
        // chiave è la milestone, non la settimana.
        let mut app = App::new();
        let pid = app.projects.add("P", Some("AAA"), None);
        let a = app.milestones.add("Alpha");
        let b = app.milestones.add("Beta");
        let w = WeekId(20000);
        app.projects.add_project_milestone(pid, a, w);
        app.projects.add_project_milestone(pid, b, w);

        // Ordinate per id → l'ordine delle bande è stabile tra un frame e l'altro.
        assert_eq!(app.projects.project_milestones_at_week(pid, w), vec![a, b]);
        // …e ciascuna ha il suo colore, quindi le bande sono distinguibili.
        assert_ne!(app.milestones.get_color(a), app.milestones.get_color(b));
    }

    // ── Asse delle colonne (header + griglia + footer condividono questo) ───

    /// Costruisce un'app la cui griglia va da `from` a `to` (date locali), così
    /// il confine d'anno cade in un punto noto.
    fn app_spanning(from: (i32, u32, u32), to: (i32, u32, u32)) -> App {
        let day = |(y, m, d): (i32, u32, u32)| {
            local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()) as usize
        };
        let mut app = App::new();
        app.start_week = WeekId(day(from));
        app.end_week = WeekId(day(to));
        app
    }

    /// Settimane di una colonna (vuoto per la colonna di confine).
    fn col_weeks(c: &Col) -> Vec<i32> {
        match c {
            Col::Weeks(ws) => ws.clone(),
            Col::YearEnd(_) => Vec::new(),
        }
    }

    /// Senza zoom ogni colonna è **una** settimana, nell'ordine di `weeks_vec`.
    #[test]
    fn columns_at_zoom_zero_are_single_weeks() {
        let app = app_spanning((2026, 1, 5), (2026, 2, 9)); // 6 lunedì
        let cols = columns_vec(&app, 0);
        assert_eq!(cols.len(), 6);
        assert!(cols.iter().all(|c| col_weeks(c).len() == 1));
        // Le settimane sono consecutive di 7 giorni.
        let first = col_weeks(&cols[0])[0];
        let last = col_weeks(&cols[5])[0];
        assert_eq!(last - first, 5 * WEEK_STEP as i32);
    }

    /// Lo zoom raggruppa 2 (livello 1) o 4 (livello 2) settimane per colonna.
    #[test]
    fn zoom_groups_weeks_per_column() {
        let app = app_spanning((2026, 1, 5), (2026, 2, 23)); // 8 settimane
        assert_eq!(columns_vec(&app, 0).len(), 8);

        let z1 = columns_vec(&app, 1);
        assert_eq!(z1.len(), 4);
        assert!(z1.iter().all(|c| col_weeks(c).len() == 2));

        let z2 = columns_vec(&app, 2);
        assert_eq!(z2.len(), 2);
        assert!(z2.iter().all(|c| col_weeks(c).len() == 4));

        // Qualunque sia lo zoom, nessuna settimana si perde o si duplica.
        for level in [0, 1, 2] {
            let all: Vec<i32> = columns_vec(&app, level)
                .iter()
                .flat_map(col_weeks)
                .collect();
            assert_eq!(all.len(), 8, "livello {level}");
        }
    }

    /// A cavallo di fine anno viene inserita la colonna gialla di confine, e
    /// **nessun gruppo dello zoom attraversa il confine**: dicembre e gennaio
    /// non finiscono mai nella stessa colonna.
    #[test]
    fn year_boundary_splits_groups_and_inserts_a_canary_column() {
        // Dal 30/11/2026 al 25/01/2027: il confine 2026→2027 cade in mezzo.
        let app = app_spanning((2026, 11, 30), (2027, 1, 25));
        let cols = columns_vec(&app, 2); // gruppi da 4

        let boundaries: Vec<&Col> = cols
            .iter()
            .filter(|c| matches!(c, Col::YearEnd(_)))
            .collect();
        assert_eq!(boundaries.len(), 1, "un solo confine d'anno");
        assert!(matches!(boundaries[0], Col::YearEnd(2026)));

        // Ogni gruppo sta tutto dentro un anno solo.
        for c in &cols {
            let years: HashSet<i32> = col_weeks(c)
                .iter()
                .map(|w| days_to_local(*w).year())
                .collect();
            assert!(
                years.len() <= 1,
                "un gruppo non può attraversare il confine d'anno: {years:?}"
            );
        }
    }

    /// La colonna di confine è più stretta di una settimana, e gli offset sono
    /// coerenti con la larghezza totale: è ciò che tiene allineati header,
    /// griglia e footer, che scorrono insieme.
    #[test]
    fn column_offsets_are_consistent_with_total_width() {
        let app = app_spanning((2026, 11, 30), (2027, 1, 25));
        let cols = columns_vec(&app, 0);
        let cw = COL_W;

        // L'offset dell'ultima colonna più la sua larghezza = larghezza totale.
        let last = cols.len() - 1;
        assert_eq!(
            col_x_offset(&cols, last, cw) + col_width(&cols[last], cw),
            cols_width(&cols, cw)
        );
        // Il primo offset è zero e gli offset crescono monotonamente.
        assert_eq!(col_x_offset(&cols, 0, cw), 0.0);
        for i in 1..cols.len() {
            assert!(col_x_offset(&cols, i, cw) > col_x_offset(&cols, i - 1, cw));
        }
        // La colonna di confine non è larga come una settimana.
        let boundary = cols.iter().find(|c| matches!(c, Col::YearEnd(_))).unwrap();
        assert_ne!(col_width(boundary, cw), cw);
    }

    /// Lo scroll che centra una colonna non va mai a sinistra dell'origine
    /// (le prime colonne restano semplicemente attaccate al bordo).
    #[test]
    fn centered_scroll_never_goes_negative() {
        let app = app_spanning((2026, 1, 5), (2026, 6, 29)); // ~26 settimane
        let cols = columns_vec(&app, 0);
        let visible = 800.0;

        // Prima colonna: vorrebbe un offset negativo → resta a 0.
        assert_eq!(centered_scroll_x(&cols, 0, COL_W, visible), 0.0);
        // Colonna lontana: centrata nella vista.
        let idx = cols.len() - 1;
        let x = centered_scroll_x(&cols, idx, COL_W, visible);
        assert_eq!(
            x,
            col_x_offset(&cols, idx, COL_W) + COL_W / 2.0 - visible / 2.0
        );
        // Una vista larghissima non lascia nulla da scorrere.
        assert_eq!(centered_scroll_x(&cols, idx, COL_W, 100_000.0), 0.0);
    }

    // ── Altezze delle righe (colonna sinistra e griglia usano la stessa) ────

    /// L'altezza del blocco di un dev dipende dalla modalità: compatta = 1 riga,
    /// mergiata = 2 (cumulativo + somma), normale = una riga per worker + il
    /// cumulativo. Il bordo è contato due volte (sopra e sotto).
    #[test]
    fn dev_block_height_per_mode() {
        // Normale: 3 worker → 3 righe + 1 cumulativo.
        assert_eq!(dev_inner_h(3, false, false), 4.0 * ROW_H);
        // Compatta: sempre una riga (la barra), quanti che siano i worker.
        assert_eq!(dev_inner_h(3, true, false), ROW_H);
        // Mergiata (zoom): cumulativo + somma.
        assert_eq!(dev_inner_h(3, false, true), 2.0 * ROW_H);
        // La compatta vince sulla mergiata.
        assert_eq!(dev_inner_h(3, true, true), ROW_H);
        // Il blocco aggiunge il bordo sopra e sotto.
        assert_eq!(
            dev_block_height(3, false, false),
            2.0 * DEV_BORDER + 4.0 * ROW_H
        );
    }

    /// `project_layout` è l'unica sorgente di verità delle altezze: quello che
    /// restituisce deve quadrare con `total_content_h`, altrimenti la colonna
    /// sinistra e la griglia scorrerebbero disallineate.
    fn layout_in_headless_ui(
        app: &App,
        filter: &Filter,
        dev_filter: &DevFilter,
        compact: bool,
    ) -> Vec<ProjLayout> {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1200.0, 800.0),
        ));
        let out = std::cell::RefCell::new(Vec::new());
        let _ = ctx.run(input, |ctx| {
            egui::Window::new("t").show(ctx, |ui| {
                *out.borrow_mut() = project_layout(
                    ui,
                    app,
                    filter,
                    false,
                    dev_filter,
                    ProjectViewMode::Open,
                    compact,
                    false,
                );
            });
        });
        out.into_inner()
    }

    /// App di prova: due progetti aperti, il primo con due dev.
    fn layout_app() -> (App, ProjectId, ProjectId, DevId, DevId) {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let front = app.devs.add("Frontend");
        let back = app.devs.add("Backend");
        let wk = app.start_week;

        let p1 = app.projects.add("Primo", Some("AAA"), Some(wk));
        app.projects.add_dev(p1, front);
        app.projects.add_dev(p1, back);
        app.projects.add_effort(p1, front, wk, alice, Effort(8));
        app.projects.add_effort(p1, front, wk, bob, Effort(4));
        app.projects.add_effort(p1, back, wk, bob, Effort(8));

        let p2 = app.projects.add("Secondo", Some("BBB"), Some(wk));
        app.projects.add_dev(p2, front);
        app.projects.add_effort(p2, front, wk, alice, Effort(8));

        (app, p1, p2, front, back)
    }

    #[test]
    fn project_layout_heights_add_up_to_the_scrollable_content() {
        let (app, ..) = layout_app();
        let layout = layout_in_headless_ui(&app, &None, &None, false);
        assert_eq!(layout.len(), 2);

        let expected: f32 = DEV_BORDER + layout.iter().map(|p| p.proj_h + DEV_BORDER).sum::<f32>();
        assert_eq!(total_content_h(&layout), expected);

        // L'altezza di un progetto copre sempre i suoi blocchi dev.
        for p in &layout {
            let devs: f32 = p
                .devs
                .iter()
                .map(|(_, m)| dev_block_height(*m, false, false))
                .sum();
            assert!(
                p.proj_h >= devs,
                "il progetto deve contenere i suoi dev: {} < {devs}",
                p.proj_h
            );
        }
    }

    /// Con un filtro attivo l'intestazione del progetto si riduce a una riga
    /// (solo tripletta), così le righe filtrate non trascinano spazio inutile.
    #[test]
    fn active_filter_shrinks_the_project_header() {
        let (app, ..) = layout_app();
        let unfiltered = layout_in_headless_ui(&app, &None, &None, false);

        let only_alice: Filter = Some(["Alice".to_string()].into_iter().collect());
        let filtered = layout_in_headless_ui(&app, &only_alice, &None, false);

        // Alice lavora solo su Frontend: nel primo progetto resta un dev solo.
        let p1_before = &unfiltered[0];
        let p1_after = &filtered[0];
        assert_eq!(p1_before.devs.len(), 2);
        assert_eq!(p1_after.devs.len(), 1);
        assert!(
            p1_after.proj_h < p1_before.proj_h,
            "l'intestazione compressa deve far calare l'altezza: {} !< {}",
            p1_after.proj_h,
            p1_before.proj_h
        );
    }

    /// Un progetto senza dev corrispondenti al filtro sparisce del tutto.
    #[test]
    fn projects_without_matching_workers_disappear() {
        let (app, ..) = layout_app();
        let only_bob: Filter = Some(["Bob".to_string()].into_iter().collect());
        let layout = layout_in_headless_ui(&app, &only_bob, &None, false);
        // Bob è solo nel primo progetto → il secondo non compare.
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].name, "Primo");
    }

    /// La vista compatta abbassa ogni progetto: un blocco dev è una riga sola.
    #[test]
    fn compact_view_is_shorter_than_the_normal_one() {
        let (app, ..) = layout_app();
        let normal = layout_in_headless_ui(&app, &None, &None, false);
        let compact = layout_in_headless_ui(&app, &None, &None, true);

        assert_eq!(normal.len(), compact.len());
        assert!(
            total_content_h(&compact) < total_content_h(&normal),
            "compatta {} !< normale {}",
            total_content_h(&compact),
            total_content_h(&normal)
        );
    }

    // ── Notifiche in-app (toast) ────────────────────────────────────────────

    #[test]
    fn toast_icons_are_renderable() {
        let ctx = egui::Context::default();
        install_symbol_fallback(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let font = egui::FontId::proportional(14.0);
        for level in [
            ToastLevel::Info,
            ToastLevel::Success,
            ToastLevel::Warning,
            ToastLevel::Error,
        ] {
            assert!(
                ctx.fonts_mut(|f| f.has_glyphs(&font, level.icon())),
                "l'icona di {level:?} ({}) non è disegnabile coi font caricati",
                level.icon()
            );
        }
    }

    #[test]
    fn repeated_toasts_refresh_instead_of_stacking() {
        let mut state = UiState::default();
        state.toast_error("git: 'push' non riuscito");
        state.toast_error("git: 'push' non riuscito");
        state.toast_error("git: 'push' non riuscito");
        // Un errore che si ripete a ogni autosave non impila copie.
        assert_eq!(state.toasts.len(), 1);

        // Un testo diverso è una notifica a sé.
        state.toast_ok("File salvato");
        assert_eq!(state.toasts.len(), 2);
    }

    #[test]
    fn toast_list_is_capped_keeping_the_newest() {
        let mut state = UiState::default();
        for i in 0..(MAX_TOASTS + 3) {
            state.toast_info(format!("messaggio {i}"));
        }
        assert_eq!(state.toasts.len(), MAX_TOASTS);
        // Le più vecchie escono dalla cima: resta in coda l'ultima arrivata.
        assert_eq!(
            state.toasts.last().map(|t| t.text.as_str()),
            Some(format!("messaggio {}", MAX_TOASTS + 2).as_str())
        );
        assert!(!state.toasts.iter().any(|t| t.text == "messaggio 0"));
    }

    #[test]
    fn only_errors_are_sticky() {
        // Gli errori restano finché non li si chiude; gli altri livelli scadono.
        assert_eq!(ToastLevel::Error.ttl(), None);
        for level in [ToastLevel::Info, ToastLevel::Success, ToastLevel::Warning] {
            assert!(level.ttl().is_some(), "{level:?} deve scadere da sola");
        }
    }

    /// Il layer si disegna davvero (layout annidato ✕ + testo a capo) e le
    /// notifiche scadute spariscono da sole, quelle di errore no.
    #[test]
    fn toasts_layer_draws_and_expires() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1000.0, 700.0),
        ));

        let mut state = UiState::default();
        state.toast_error(
            "Errore lungo che deve andare a capo dentro il riquadro \
                           della notifica senza far esplodere il layout",
        );
        state.toast_ok("PDF salvato: progetto_26-08-10.pdf");
        state.toast_warn("Nessun progetto visibile: PDF non creato.");

        // Due frame: la prima registra l'area, la seconda disegna a layout stabile.
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| toasts_layer(ctx, &mut state));
        }
        assert_eq!(state.toasts.len(), 3, "nessuna deve sparire subito");

        // Invecchiamo tutto oltre la ttl più lunga: resta solo l'errore.
        let old = std::time::Instant::now() - std::time::Duration::from_secs(3600);
        for t in state.toasts.iter_mut() {
            t.born = old;
        }
        let _ = ctx.run(input.clone(), |ctx| toasts_layer(ctx, &mut state));
        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].level, ToastLevel::Error);
    }

    /// Un errore di salvataggio deve arrivare all'utente come notifica: qui il
    /// percorso è una cartella inesistente, quindi la scrittura fallisce.
    #[test]
    fn failed_save_reports_an_error_toast() {
        let app = App::new();
        let bad = "/pjm_cartella_inesistente_per_test/workers.ron";
        let err = app.save(bad).expect_err("il salvataggio deve fallire");

        let mut state = UiState::default();
        state.toast_error(err);
        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].level, ToastLevel::Error);
        assert!(state.toasts[0].text.contains(bad));
    }
}
