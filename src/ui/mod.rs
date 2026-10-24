//! Interfaccia egui (immediate-mode): stato UI, azioni, ciclo `ui()` e helper condivisi.
//! Tappa 1: griglia effort fedele, editing celle, scroll sincronizzato,
//! salva/apri, aggiunta worker/dev/progetto/categoria.

pub(crate) use std::collections::{HashMap, HashSet};

pub(crate) use chrono::Datelike;
pub(crate) use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Vec2};
pub(crate) use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

pub(crate) use crate::app::App;
pub(crate) use crate::categories::CategoryId;
pub(crate) use crate::date_utils::dates::{
    current_monday_days, days_to_local, local_to_days, parse_date_str,
    primo_giorno_settimana_corrente, today, today_days, week_label,
};
pub(crate) use crate::dev_utils::dev::DevId;
pub(crate) use crate::milestones::{MilestoneCategory, MilestoneId, MilestoneKind, MilestoneScope};
pub(crate) use crate::project_utils::project::{
    Enable, OverflowResolution, ProjectId, ProjectStatus,
};
pub(crate) use crate::single_dev_utils::single_dev::{WEEK_STEP, WeekId};
pub(crate) use crate::single_effort_utils::single_effort::Effort;
pub(crate) use crate::ui_style::*;
pub(crate) use crate::workers_utils::worker::{WORKER_ID_ZERO, WeekStatus, WorkerId};

mod compare;
mod dialogs;
mod export;
mod filters;
mod footer;
mod grid;
mod help;
mod matrix;
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
pub(crate) use matrix::*;
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
        /// Spunta «Metti tutti i worker in ferie».
        holiday: bool,
        /// Valore della spunta all'apertura (tutti già in ferie): toglierla
        /// rimuove le ferie, lasciarla spenta non tocca gli stati.
        holiday_initial: bool,
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

/// Colonna (prima settimana del gruppo, come `Col::Weeks(ws)[0]`) e worker
/// sotto il cursore nella griglia — o della cella in modifica, che ha la
/// precedenza. Header e footer evidenziano la data della colonna e, nel footer,
/// il nome del worker e la sua cella di quella settimana.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct GridHighlight {
    pub week: Option<i32>,
    pub worker: Option<WorkerId>,
}

/// Fondo e testo delle evidenziazioni (data/worker puntati, matrice): giallo
/// in grassetto su fondo scuro, uguali nei due temi.
pub(crate) fn hi_bg() -> Color32 {
    g(Color32::from_gray(40))
}

pub(crate) fn hi_text() -> Color32 {
    g(Color32::from_rgb(255, 221, 0))
}

/// Scarto della seconda passata del grassetto simulato (un solo peso di font).
pub(crate) const HI_BOLD_DX: f32 = 0.7;

/// Cella evidenziata: fondo `hi_bg`, testo `hi_text` in grassetto simulato,
/// centrato.
pub(crate) fn paint_highlight(ui: &egui::Ui, cell: Rect, text: &str) {
    ui.painter().rect_filled(cell, 0.0, hi_bg());
    for dx in [0.0, HI_BOLD_DX] {
        ui.painter().text(
            cell.center() + Vec2::new(dx, 0.0),
            Align2::CENTER_CENTER,
            text,
            cell_font(),
            hi_text(),
        );
    }
}

/// Finestra «Limite settimana (tutti)» per la settimana `week` (click sulla
/// data in alto o nel footer): la spunta ferie parte attiva se tutti i worker
/// (esclusi quelli in malattia) sono già in ferie.
pub(crate) fn bulk_week_popup(app: &App, week: usize, date: String) -> Popup {
    let holiday = app.all_on_holiday(week);
    Popup::BulkWeekMax {
        week,
        date,
        text: DEFAULT_MAX_HOURS.to_string(),
        holiday,
        holiday_initial: holiday,
    }
}

/// Stato della finestra «Clona progetto»: progetto scelto e ricerca
/// sull'elenco.
#[derive(Default)]
pub(crate) struct CloneProject {
    pub src: Option<ProjectId>,
    pub search: String,
}

/// Tripletta della copia: quella dell'originale più «Copia» (solo «Copia» se
/// l'originale non ne ha una).
pub(crate) fn clone_tripletta(original: &str) -> String {
    let t = original.trim();
    if t.is_empty() {
        "Copia".to_string()
    } else {
        format!("{t} Copia")
    }
}

/// Clona completamente `src` (vedi `Projects::clone_project`) e duplica le
/// sue milestone **personalizzate** come milestone della copia (stesso nome,
/// colore, tipo e categorie, stesse settimane), così modificarle in un
/// progetto non tocca l'altro. Quelle di sistema restano condivise. La copia
/// ha lo stesso nome e la tripletta di `clone_tripletta`.
pub(crate) fn clone_project(app: &mut App, src: ProjectId) -> Option<ProjectId> {
    let tripletta = clone_tripletta(&app.projects.get_tripletta(src));
    let name = app.projects.get_info(src);
    let new = app.projects.clone_project(src, &name, &tripletta)?;
    for (mid, week) in app.projects.list_project_milestones(new) {
        if app.milestones.owner(mid) != Some(src) {
            continue;
        }
        let copy = app.milestones.add_owned(
            app.milestones.get_name(mid).unwrap_or("?").to_string().as_str(),
            app.milestones.get_kind(mid),
            app.milestones.get_categories(mid),
            Some(new),
        );
        if let Some(c) = app.milestones.get_color(mid) {
            app.milestones.set_color(copy, c);
        }
        app.projects.remove_project_milestone(new, mid);
        app.projects.add_project_milestone(new, copy, week);
    }
    Some(new)
}

/// Estremo del progetto spostato dal menù della griglia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProjectBound {
    Start,
    End,
}

/// Cosa si perde spostando inizio/fine: le celle (anche a effort 0) e le
/// milestone che finirebbero fuori dal nuovo periodo del progetto.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct BoundLoss {
    pub efforts: Vec<(DevId, WeekId)>,
    pub milestones: Vec<(MilestoneId, WeekId)>,
}

impl BoundLoss {
    pub fn is_empty(&self) -> bool {
        self.efforts.is_empty() && self.milestones.is_empty()
    }
}

/// Spostamento di inizio/fine in attesa di conferma (`bound_move_window`):
/// c'è qualcosa da cancellare, l'utente deve dire sì o annullare.
pub(crate) struct BoundMove {
    pub proj: ProjectId,
    pub bound: ProjectBound,
    pub week: WeekId,
    pub loss: BoundLoss,
}

/// «Cancella gli effort» in attesa di conferma (`clear_efforts_window`):
/// `dev = Some(d)` svuota le celle di quel dev, `None` di tutti i dev del progetto.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ClearEfforts {
    pub proj: ProjectId,
    pub dev: Option<DevId>,
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
/// Ctrl+D → Dev, Ctrl+K → Categorie, Ctrl+Y → Anni).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterPane {
    Workers,
    Projects,
    Devs,
    Categories,
    Years,
}

/// Criterio del filtro anno (Ctrl+Y): quale data del progetto deve cadere in
/// uno degli anni selezionati. Scelto nella colonna «Anni», non persistito.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum YearCriterion {
    /// La data di **inizio** cade nell'anno.
    #[default]
    Starts,
    /// La data di **fine** cade nell'anno.
    Ends,
    /// Inizio **e** fine cadono nello **stesso** anno.
    Within,
}

impl YearCriterion {
    pub(crate) const ALL: [YearCriterion; 3] = [Self::Starts, Self::Ends, Self::Within];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Starts => "Inizia nell'anno",
            Self::Ends => "Finisce nell'anno",
            Self::Within => "Tutto nell'anno",
        }
    }
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
    state.category_filter = None;
    state.year_filter = None;
    state.worker_filter_current_week = false;
    state.worker_search.clear();
    state.project_search.clear();
    state.dev_search.clear();
    state.category_search.clear();
    state.year_search.clear();
    // Eventuali toggle richiesti nello stesso frame non devono ri-deselezionare.
    state.worker_filter_toggle_all = false;
    state.dev_filter_toggle_all = false;
    state.category_filter_toggle_all = false;
    state.year_filter_toggle_all = false;
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
/// Vista (aperti / chiusi / tutti), con il filtro di visibilità e con i filtri
/// categoria e anno (`ProjectFilter`, che come la visibilità agiscono su
/// progetti interi). Non tiene
/// conto del filtro worker (che nasconde solo dev, non progetti interi ai fini
/// dell'export). Usato per allineare gli elenchi di PDF/SVG/minuta a ciò che si
/// vede a schermo, in ordine di visualizzazione.
fn body_projects(
    app: &App,
    view: ProjectViewMode,
    pf: &ProjectFilter,
) -> Vec<ProjectId> {
    app.projects
        .list_full()
        .into_iter()
        .filter(|(id, _, _)| project_in_body(app, view, *id))
        .filter(|(id, _, _)| pf.shows(app, *id))
        .map(|(id, _, _)| id)
        .collect()
}

/// Stato della dialog "Esporta PDF" del singolo progetto: i dev del progetto con
/// il flag di selezione, nell'ordine (riordinabile con ▲▼) scelto dall'utente.
struct PdfExport {
    proj: ProjectId,
    entries: Vec<(DevId, bool)>,
    /// Milestone **collocate in questo progetto** con il flag di selezione, in
    /// ordine di settimana: solo quelle spuntate finiscono nel PDF/SVG.
    milestones: Vec<(MilestoneId, WeekId, bool)>,
    /// Formato della barra scelto per ogni dev (riga della dialog).
    formats: HashMap<DevId, crate::pdf_export::DevBar>,
    /// Periodo disegnato («Da», «A»), in giorni: alla prima apertura inizio e
    /// fine del progetto, poi quello ricordato per il progetto.
    range: (i32, i32),
}

/// Stato della dialog "Esporta PDF" con più progetti visibili: l'elenco dei
/// progetti esportabili con il flag di selezione (una pagina Gantt ciascuno).
struct PdfMultiExport {
    entries: Vec<(ProjectId, bool)>,
}

/// Ultime scelte della dialog "Esporta PDF" di **un** progetto, ricordate per la
/// sessione (non persistite): dev nell'ordine scelto con la spunta, e spunta
/// di ogni milestone.
#[derive(Default)]
struct ExportProjectMemory {
    devs: Vec<(DevId, bool)>,
    milestones: HashMap<MilestoneId, bool>,
    formats: HashMap<DevId, crate::pdf_export::DevBar>,
    /// Periodo «Da»/«A» scelto per il progetto (`None` = mai aperto).
    range: Option<(i32, i32)>,
}

/// Periodo proposto alla prima apertura della dialog di export di un progetto:
/// le sue date di inizio e fine. Se ne manca una, 12 settimane dall'altra (o da
/// oggi): l'export comunque richiede entrambe le date.
pub(crate) fn project_export_range(app: &App, proj: ProjectId) -> (i32, i32) {
    const SPAN: i32 = 12 * WEEK_STEP as i32;
    let start = app.projects.get_project_start_week(proj).map(|w| w.0 as i32);
    let end = app.projects.get_project_end_week(proj).map(|w| w.0 as i32);
    match (start, end) {
        (Some(s), Some(e)) => (s, e),
        (Some(s), None) => (s, s + SPAN),
        (None, Some(e)) => (e - SPAN, e),
        (None, None) => {
            let today = today_days();
            (today, today + SPAN)
        }
    }
}

/// Dev della dialog di export di un progetto a partire dalle scelte ricordate:
/// quelli ancora presenti tengono ordine e spunta; i dev nuovi (o tutti, senza
/// memoria) vanno in fondo, spuntati, nell'ordine del progetto.
fn restore_dev_entries(current: &[DevId], remembered: &[(DevId, bool)]) -> Vec<(DevId, bool)> {
    let mut out: Vec<(DevId, bool)> = remembered
        .iter()
        .filter(|(d, _)| current.contains(d))
        .copied()
        .collect();
    for d in current {
        if !out.iter().any(|(x, _)| x == d) {
            out.push((*d, true));
        }
    }
    out
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

/// Stato della dialog "Report" (File ▸ Report…): progetti con flag di selezione.
struct ReportState {
    entries: Vec<(ProjectId, bool)>,
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

/// Ambiti milestone spuntati nelle dialog di export (vedi `MilestoneScope`).
/// Default: solo Internal, cioè un unico PDF con tutte le milestone — il
/// comportamento di prima delle categorie.
/// Spunta «Includi i worker ghost» delle dialog di export. Parte **attiva** (il
/// comportamento storico: i ghost sono un'anomalia da vedere), per questo è un
/// tipo a sé — `UiState` deriva `Default` e un `bool` partirebbe da `false`.
#[derive(Clone, Copy)]
pub(crate) struct ExportGhost {
    /// «Includi i worker ghost (anche a effort 0)».
    pub show: bool,
    /// Sotto-opzione: marcatura in rosso (`true`) o nel colore del dev.
    pub red: bool,
}

impl Default for ExportGhost {
    fn default() -> Self {
        Self {
            show: true,
            red: true,
        }
    }
}

/// Spunta «Includi l'anno nelle date dei dev» delle dialog di export. Parte
/// **attiva**: tipo a sé per lo stesso motivo di `ExportGhost` (`UiState`
/// deriva `Default` e un `bool` partirebbe da `false`).
#[derive(Clone, Copy)]
pub(crate) struct ExportYear(pub bool);

impl Default for ExportYear {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ExportScopes {
    pub internal: bool,
    pub external: bool,
    pub mix: bool,
}

impl Default for ExportScopes {
    /// Tutte e tre spuntate: di default escono tre file.
    fn default() -> Self {
        Self {
            internal: true,
            external: true,
            mix: true,
        }
    }
}

impl ExportScopes {
    /// Ambiti scelti, nell'ordine in cui vanno esportati.
    pub fn list(&self) -> Vec<MilestoneScope> {
        let mut v = Vec::new();
        if self.internal {
            v.push(MilestoneScope::Internal);
        }
        if self.external {
            v.push(MilestoneScope::External);
        }
        if self.mix {
            v.push(MilestoneScope::Mix);
        }
        v
    }

    /// Nessun ambito spuntato: non c'è niente da esportare.
    pub fn is_empty(&self) -> bool {
        !self.internal && !self.external && !self.mix
    }
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
    category_search: String,
    year_search: String,
    // progetto verso cui scrollare al prossimo frame (risolto in `body`)
    jump_to_project: Option<ProjectId>,
    editing: Option<Editing>,
    note_editor: Option<NoteEditing>,
    popup: Option<Popup>,
    // inserimento multiplo (tasto destro su cella vuota): worker + ore + settimane
    bulk_fill: Option<BulkFill>,
    bulk_fill_was_open: bool,
    // conferma «Sposta inizio/fine progetto qua» quando si perdono effort/milestone
    bound_move: Option<BoundMove>,
    dev_manage: Option<ProjectId>,
    confirm_del_dev: Option<(ProjectId, DevId)>,
    // conferma «Cancella gli effort» (menù del nome dev)
    confirm_clear_efforts: Option<ClearEfforts>,
    // conferma «Cancella tutte le milestone» (menù della fascia milestone)
    confirm_clear_milestones: Option<ProjectId>,
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
    /// Worker scelto col click sinistro sul suo nome nel footer: le sue righe
    /// nelle celle dei dev restano evidenziate (giallo su scuro) finché non si
    /// riclicca lo stesso nome (o un altro, che prende il suo posto). Solo sessione.
    footer_selected_worker: Option<crate::workers_utils::worker::WorkerId>,
    // filtro dev (Ctrl+D): None = nessun filtro; Some(set) = mostra solo i
    // progetti in cui almeno un dev selezionato ha effort > 0, e dentro il
    // progetto solo quei dev. Si combina in AND con il filtro worker. Non persistito.
    dev_filter: Option<HashSet<DevId>>,
    // filtro categoria (Ctrl+K): None = nessun filtro; Some(set) = mostra solo i
    // progetti la cui categoria è nel set (`None` nel set = "Senza categoria").
    // Agisce su progetti interi, come la visibilità. Non persistito.
    category_filter: CategoryFilter,
    // filtro anno (Ctrl+Y): None = nessun filtro; Some(set) = mostra solo i
    // progetti che soddisfano `year_criterion` per almeno un anno del set. Agisce
    // su progetti interi. Non persistito (come il criterio).
    year_filter: YearFilter,
    year_criterion: YearCriterion,
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
    // un menu (combo dello stato) era aperto nel frame precedente della
    // finestra «Stato progetti»: il click che sceglie una voce chiude il menu
    // nello stesso frame, e non deve contare come «click fuori».
    closed_filter_popup_was_open: bool,
    // solo test: rettangoli delle combo di stato e delle voci del menu aperto,
    // per simulare click veri.
    #[cfg(test)]
    status_combo_rects: Vec<Rect>,
    #[cfg(test)]
    status_item_rects: Vec<Rect>,
    // richiesta (da Ctrl+F / Ctrl+G / Ctrl+P / Ctrl+D a dialog già aperta) di fare
    // il toggle del "Select All" della rispettiva colonna; consumata dalla dialog
    // "Filtri" nello stesso frame.
    worker_filter_toggle_all: bool,
    dev_filter_toggle_all: bool,
    category_filter_toggle_all: bool,
    year_filter_toggle_all: bool,
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
    // export multi-progetto: date sotto ogni tratto per i formati segmentati
    // (nel singolo progetto si sceglie per dev). Ricordato, non persistito.
    export_dates_below: bool,
    // tema chiaro/scuro: Auto = segue il sistema, altrimenti forzato
    theme_pref: ThemePref,
    // dialog "Esporta PDF" del singolo progetto (aperta quando resta visibile
    // un solo progetto e si lancia l'export)
    pdf_export: Option<PdfExport>,
    // dialog "Esporta PDF" con più progetti visibili: selezione dei progetti
    pdf_multi_export: Option<PdfMultiExport>,
    // dialog "Minuta" (File ▸ Minuta…): selezione progetti + scelta note
    minuta: Option<MinutaState>,
    // dialog "Report" (File ▸ Report…): selezione dei progetti del PDF
    report: Option<ReportState>,
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
    // offset orizzontale della heatmap, seguito dall'intestazione (anno + date)
    // che sta fuori dallo scroll verticale e quindi resta sempre visibile
    sat_scroll_x: f32,
    // cella della heatmap sotto il cursore nel frame precedente (worker, indice
    // di colonna): intestazione e colonna fissa sono disegnate prima delle celle,
    // quindi leggono questo valore per evidenziare data e nome
    sat_hover: Option<(WorkerId, usize)>,
    // Matrice progetti × worker (Vista ▸ Matrice progetti/worker…): dialog di
    // scelta dei worker, finestra della matrice, worker scelti (ricordati per
    // la sessione, non persistiti) e intervallo di date (`None` = oggi).
    matrix_select_open: bool,
    show_matrix: bool,
    matrix_workers: HashSet<WorkerId>,
    matrix_worker_search: String,
    matrix_from: Option<i32>,
    matrix_to: Option<i32>,
    // anni scelti nella combo in alto: impostano Da/A (1/1 del più vecchio →
    // 31/12 del più recente); svuotata se Da/A vengono cambiati a mano.
    matrix_years: std::collections::BTreeSet<i32>,
    // categorie scelte nella combo in alto (`None` = «Senza categoria»); vuoto =
    // tutte. Solo sessione.
    matrix_categories: std::collections::BTreeSet<Option<CategoryId>>,
    // matrice: stati mostrati (Attivi / On-Hold / Closed; vuoto = tutti),
    // solo sessione
    matrix_statuses: std::collections::BTreeSet<ProjectStatus>,
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
    // nome della milestone "di progetto" in corso di creazione dal menù della
    // griglia (tasto destro sulla riga alta ▸ Aggiungi milestone qui)
    new_project_milestone: String,
    // categorie di stampa scelte per la PROSSIMA milestone di progetto
    // (vuoto = solo Internal); restano per l'inserimento successivo
    new_project_milestone_categories: Vec<MilestoneCategory>,
    // tipo scelto per la PROSSIMA milestone creata dal menù Aggiungi
    new_milestone_kind: MilestoneKind,
    // categorie scelte per la PROSSIMA milestone creata dal menù Aggiungi
    // (vuoto = solo Internal, come i file scritti prima delle categorie)
    new_milestone_categories: Vec<MilestoneCategory>,
    // finestra di gestione milestone (elenco, colore, tipo, elimina)
    show_milestone_manager: bool,
    milestone_manager_just_opened: bool,
    // finestra «Clona progetto» (Aggiungi ▸ Clona progetto…)
    clone_project: Option<CloneProject>,
    // finestra «Worker» (Filtri ▸ Worker (visibilità e ghost)…): ricerca
    show_worker_visibility: bool,
    worker_visibility_just_opened: bool,
    worker_visibility_search: String,
    // Data e worker da evidenziare in intestazione e footer: `grid_hl` è quello
    // del frame precedente (letto da header/footer, disegnati PRIMA della
    // griglia), `grid_hl_next` quello raccolto dalla griglia nel frame corrente.
    grid_hl: GridHighlight,
    grid_hl_next: GridHighlight,
    // finestra «Dev nei totali»: TUTTI i dev con la spunta di inclusione nei
    // totali del footer (unico punto da cui rimettere un dev nascosto).
    show_dev_totals: bool,
    dev_totals_just_opened: bool,
    // buffer di editing per nomi progetto ed effort dev
    name_buffers: HashMap<usize, String>,
    effort_buffers: HashMap<(usize, usize), String>,
    // buffer di editing per la % dichiarata dal dev (chiave: proj, dev)
    declared_buffers: HashMap<(usize, usize), String>,
    // includere le percentuali (presunta/dichiarata) nel PDF/SVG esportato;
    // scelta chiesta nelle dialog di export, ricordata tra un export e l'altro.
    export_progress_pct: bool,
    // aggiungere l'anno abbreviato ('26) alle date dei dev nel PDF/SVG;
    // ricordata tra un export e l'altro, non persistita.
    export_show_year: ExportYear,
    // stampare o no i worker "ghost" (marcatura rossa, comprese le settimane a
    // effort 0); scelta chiesta nelle dialog di export, ricordata tra un export
    // e l'altro, non persistita.
    export_ghost: ExportGhost,
    // ultime scelte delle dialog di export PDF/SVG, solo per la sessione:
    // per progetto (dev, ordine, milestone) e, nell'export multi-progetto, la
    // spunta di ogni progetto (quelli mai visti partono spuntati).
    export_project_memory: HashMap<ProjectId, ExportProjectMemory>,
    export_multi_memory: HashMap<ProjectId, bool>,
    // ambiti milestone scelti nelle dialog di export (Solo internal / Solo
    // external / Mix): ogni ambito spuntato = un file (default: tutti e tre).
    // Ricordato tra un export e l'altro, non persistito.
    export_scopes: ExportScopes,
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
    /// Clona completamente un progetto (Aggiungi ▸ Clona progetto…).
    CloneProject {
        src: ProjectId,
    },
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
    /// Sposta inizio o fine del progetto alla settimana `week` (menù della
    /// griglia), cancellando effort e milestone che finiscono fuori dal periodo.
    MoveProjectBound {
        proj: ProjectId,
        bound: ProjectBound,
        week: WeekId,
    },
    SetProjectCategory {
        proj: ProjectId,
        cat: Option<CategoryId>,
    },
    DelRow {
        proj: ProjectId,
        dev: DevId,
    },
    /// Svuota le celle della griglia (non l'effort pianificato) di un dev, o di
    /// tutti i dev del progetto con `dev: None`.
    ClearEfforts {
        proj: ProjectId,
        dev: Option<DevId>,
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
    /// Stato del progetto (Attivo / On-Hold / Closed) dalla finestra «Stato
    /// progetti».
    SetProjectStatus {
        proj: ProjectId,
        status: ProjectStatus,
    },
    SetWorkerMaxHours {
        worker: WorkerId,
        hours: u32,
    },
    SetWorkerGhost {
        worker: WorkerId,
        ghost: bool,
    },
    /// Nasconde (o mostra) la riga del worker nel footer (`hide_in_footer`).
    SetWorkerHiddenInFooter {
        worker: WorkerId,
        hidden: bool,
    },
    /// Fa comparire (o no) il worker negli elenchi di scelta (`show_in_find`).
    SetWorkerShownInFind {
        worker: WorkerId,
        shown: bool,
    },
    /// Esclude (o rimette) un dev dai totali in basso a sinistra del footer.
    SetDevHiddenInTotals {
        dev: DevId,
        hidden: bool,
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
    /// Limite ore della settimana per tutti i worker; `holiday` vedi
    /// `App::set_bulk_week` (`Some(true)` ferie, `Some(false)` togli, `None` invariato).
    SetBulkWeekLimit {
        week: usize,
        hours: u32,
        holiday: Option<bool>,
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
        /// Ambiti milestone da stampare: uno per file.
        scopes: Vec<MilestoneScope>,
    },
    ExportPdfProject {
        proj: ProjectId,
        /// Dev nell'ordine scelto, ciascuno col proprio formato barra.
        devs: Vec<(DevId, crate::pdf_export::DevBar)>,
        scopes: Vec<MilestoneScope>,
        /// Milestone spuntate nella dialog (le altre non vengono disegnate).
        milestones: Vec<MilestoneId>,
        /// Periodo disegnato («Da», «A»), in giorni.
        range: (i32, i32),
    },
    ExportSvgProject {
        proj: ProjectId,
        devs: Vec<(DevId, crate::pdf_export::DevBar)>,
        scopes: Vec<MilestoneScope>,
        milestones: Vec<MilestoneId>,
        range: (i32, i32),
    },
    GenerateReport {
        projects: Vec<ProjectId>,
    },
    /// PDF della matrice progetti × worker (worker scelti in `UiState`)
    /// sull'intervallo di giorni `[from, to]`.
    ExportWorkerMatrix {
        from: i32,
        to: i32,
        /// Anni scelti nella combo (vuoto = nessuno), stampati nel sottotitolo.
        years: Vec<i32>,
    },
    GenerateMinuta {
        projects: Vec<ProjectId>,
        only_current: bool,
        only_with_notes: bool,
        worker_notes: bool,
    },
    CreateMilestone(String, MilestoneKind, Vec<MilestoneCategory>),
    /// Milestone **personalizzata** di un progetto, creata dal menù della
    /// griglia e collocata subito nella settimana su cui si è cliccato.
    CreateProjectMilestone {
        proj: ProjectId,
        week: WeekId,
        name: String,
        /// Categorie di stampa scelte nel menù (vuoto = solo Internal).
        categories: Vec<MilestoneCategory>,
    },
    SetMilestoneColor {
        milestone: MilestoneId,
        color: u32,
    },
    SetMilestoneKind {
        milestone: MilestoneId,
        kind: MilestoneKind,
    },
    SetMilestoneCategory {
        milestone: MilestoneId,
        category: MilestoneCategory,
        on: bool,
    },
    DeleteMilestone {
        milestone: MilestoneId,
    },
    AddProjectMilestone {
        proj: ProjectId,
        milestone: MilestoneId,
        week: WeekId,
    },
    /// Toglie dal progetto tutte le milestone collocate (le definizioni restano).
    ClearProjectMilestones {
        proj: ProjectId,
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
        missing_settings: Vec<&'static str>,
        startup_error: Option<String>,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.global_style_mut(|s| s.interaction.tooltip_delay = 0.0);
        install_symbol_fallback(&cc.egui_ctx);

        let today = today();
        let this_week = current_monday_days();

        // Scroll iniziale per centrare la settimana corrente.
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

        let mut me = Self {
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
        };
        me.write_missing_settings(&missing_settings);
        me
    }

    /// Se al file appena caricato mancavano parametri "solo file" (es.
    /// `corner_pct` in un file scritto prima che esistesse), lo risalva subito
    /// così i valori compaiono nel `.ron`, pronti da modificare a mano.
    fn write_missing_settings(&mut self, missing: &[&'static str]) {
        if missing.is_empty() {
            return;
        }
        if self.save_to_disk() {
            self.sync_baseline();
            self.ui
                .toast_info(format!("Aggiunti al file i parametri: {}", missing.join(", ")));
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

/// Etichetta breve di un progetto: la tripletta, o la descrizione se la
/// tripletta è vuota, o infine l'id. Condivisa da minuta, gestore milestone e
/// ovunque serva nominare un progetto in una riga.
pub(crate) fn project_label(app: &App, proj: ProjectId) -> String {
    app.projects
        .get(proj)
        .map(|p| p.label(proj))
        .unwrap_or_else(|| format!("Progetto {}", proj.0))
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
    write_export_file(state, kind, &path, data);
}

/// Scrive un file di export e riporta l'esito come notifica in-app.
fn write_export_file(
    state: &mut UiState,
    kind: &str,
    path: &std::path::Path,
    data: impl AsRef<[u8]>,
) {
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

/// Dialog di salvataggio unico per un export che può produrre **più file** (uno
/// per ambito milestone): il nome si sceglie una volta sola e, se i file sono
/// più d'uno, a ognuno viene aggiunto il suffisso dell'ambito
/// (`progetti_2026_07_13_internal.pdf` / `…_external.pdf`). Con un solo file il
/// nome scelto è usato tale e quale.
fn save_multi_export_dialog(
    state: &mut UiState,
    kind: &str,
    ext: &str,
    default_name: &str,
    items: Vec<(String, Vec<u8>)>,
) {
    if items.is_empty() {
        return;
    }
    let Some(path) = rfd::FileDialog::new()
        .add_filter(kind, &[ext])
        .set_file_name(default_name)
        .save_file()
    else {
        return;
    };
    let single = items.len() == 1;
    for (suffix, data) in items {
        let target = if single || suffix.is_empty() {
            path.clone()
        } else {
            path_with_suffix(&path, &suffix, ext)
        };
        write_export_file(state, kind, &target, data);
    }
}

/// Costruisce un export per ogni ambito milestone richiesto: imposta l'ambito
/// (`set_milestone_scope`), chiama `build`, e restituisce le coppie
/// `(suffisso file, byte)` degli export riusciti. Alla fine rimette ambito,
/// filtro milestone e periodo ai valori di default, così i `build_*`
/// successivi partono da Internal, senza restrizioni e con l'asse automatico.
fn scoped_exports(
    scopes: &[MilestoneScope],
    mut build: impl FnMut(MilestoneScope) -> Option<Vec<u8>>,
) -> Vec<(String, Vec<u8>)> {
    let mut items = Vec::new();
    for sc in scopes {
        crate::pdf_export::set_milestone_scope(*sc);
        if let Some(bytes) = build(*sc) {
            items.push((sc.file_suffix().to_string(), bytes));
        }
    }
    crate::pdf_export::set_milestone_scope(MilestoneScope::default());
    crate::pdf_export::set_milestone_allow(None);
    crate::pdf_export::set_export_range(None);
    items
}

/// `cartella/nome_suffisso.est` a partire dal percorso scelto nel dialog
/// (l'estensione del percorso, o `ext` se manca).
fn path_with_suffix(path: &std::path::Path, suffix: &str, ext: &str) -> std::path::PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let e = path
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| ext.to_string());
    path.with_file_name(format!("{stem}_{suffix}.{e}"))
}

/// Mostra il dialog di salvataggio PDF e scrive i byte nel file scelto.
fn save_pdf_dialog(state: &mut UiState, bytes: Vec<u8>, default_name: &str) {
    save_export_dialog(state, "PDF", "pdf", default_name, bytes);
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

            // Settimana corrente ricalcolata a ogni frame (non solo all'avvio): se
            // l'app resta aperta oltre il lunedì, footer e saturazione seguono la
            // griglia (`current_week_id()`), che legge la stessa data.
            state.this_week = current_monday_days();

            // Resa in scala di grigi: impostata una volta per frame, prima di disegnare.
            set_bw_mode(state.bw_mode);

            // Colore della settimana corrente e palette dei mesi: vengono dal file
            // (`week_color`, `month_colors`), non dall'interfaccia; come il tema,
            // si impostano una volta per frame.
            set_this_week_color(app.week_color_rgb());
            set_month_colors(app.month_colors_rgb());
            set_month_tint_pct(app.month_tint_pct);

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
            let (
                key_s,
                key_f,
                key_g,
                key_d,
                key_k,
                key_y,
                key_p,
                key_t,
                key_j,
                shift,
                key_1,
                key_2,
                key_3,
            ) = ui.ctx().input(|i| {
                    let cmd = i.modifiers.command || i.modifiers.ctrl;
                    (
                        cmd && i.key_pressed(egui::Key::S),
                        cmd && i.key_pressed(egui::Key::F),
                        cmd && i.key_pressed(egui::Key::G),
                        cmd && i.key_pressed(egui::Key::D),
                        cmd && i.key_pressed(egui::Key::K),
                        cmd && i.key_pressed(egui::Key::Y),
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
            if key_k {
                if shift {
                    state.category_filter = Some(HashSet::new()); // deseleziona tutte
                } else if state.show_filters {
                    state.category_filter_toggle_all = true;
                } else {
                    open_filters(state, FilterPane::Categories);
                }
                focus_pane(state, FilterPane::Categories);
            }
            if key_y {
                if shift {
                    state.year_filter = Some(HashSet::new()); // deseleziona tutti
                } else if state.show_filters {
                    state.year_filter_toggle_all = true;
                } else {
                    open_filters(state, FilterPane::Years);
                }
                focus_pane(state, FilterPane::Years);
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

            // Evidenziazione data/worker: la griglia la raccoglie da zero.
            state.grid_hl_next = GridHighlight::default();

            egui::Panel::top("toolbar")
                .show_inside(ui, |ui| toolbar(ui, app, state, &mut actions));

            egui::Panel::top("header")
                .frame(egui::Frame::NONE.fill(bg()))
                .show_inside(ui, |ui| header(ui, app, state));

            // Footer (worker / sovra) — nascosto in vista compatta.
            // La maniglia col triangolino permette di collassarlo/riaprirlo.
            if !state.compact_mode {
                if state.footer_hidden {
                    // Solo la maniglia: triangolino verso l'alto per riaprire.
                    egui::Panel::bottom("footer_handle")
                        .frame(egui::Frame::NONE.fill(bg()))
                        .show_inside(ui, |ui| footer_handle(ui, state));
                } else {
                    egui::Panel::bottom("footer")
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
            bound_move_window(ui.ctx(), app, state, &mut actions);
            dev_manage_window(ui.ctx(), app, state, &mut actions);
            pdf_export_window(ui.ctx(), app, state, &mut actions);
            pdf_multi_export_window(ui.ctx(), app, state, &mut actions);
            minuta_window(ui.ctx(), app, state, &mut actions);
            report_window(ui.ctx(), app, state, &mut actions);
            help_window(ui.ctx(), state);
            confirm_del_dev_window(ui.ctx(), state, &mut actions);
            clear_efforts_window(ui.ctx(), app, state, &mut actions);
            clear_milestones_window(ui.ctx(), app, state, &mut actions);
            filters_window(ui.ctx(), app, state, &mut actions);
            milestone_manager_window(ui.ctx(), app, state, &mut actions);
            dev_totals_window(ui.ctx(), app, state, &mut actions);
            worker_visibility_window(ui.ctx(), app, state, &mut actions);
            clone_project_window(ui.ctx(), app, state, &mut actions);
            closed_filter_window(ui.ctx(), app, state, &mut actions);
            move_dialog_window(ui.ctx(), app, state, &mut actions);
            saturation_window(ui.ctx(), app, state);
            matrix_select_window(ui.ctx(), app, state);
            matrix_window(ui.ctx(), app, state, &mut actions);
            compare_window(ui.ctx(), app, state, &mut actions);
            // Sopra a tutto: le notifiche in-app.
            toasts_layer(ui.ctx(), state);

            // Header e footer sono già stati disegnati con l'evidenziazione del
            // frame prima: se la griglia ne ha trovata un'altra, la si pubblica
            // e si ridisegna subito, così il ritardo non si vede.
            if state.grid_hl_next != state.grid_hl {
                state.grid_hl = state.grid_hl_next;
                ui.ctx().request_repaint();
            }
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
    let t = today();
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
                    match App::load_reporting_missing(&path) {
                        Ok((loaded, missing)) => {
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
                            self.write_missing_settings(&missing);
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
                let start = Some(WeekId(current_monday_days() as usize));
                self.app.projects.add("Nuovo Progetto", None, start);
                self.mark_changed();
            }
            Action::CloneProject { src } => {
                if let Some(new) = clone_project(&mut self.app, src) {
                    self.mark_changed();
                    self.ui.jump_to_project = Some(new);
                    let label = project_label(&self.app, new);
                    self.ui.toast_ok(format!("Progetto clonato: {label}"));
                }
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
                    let Some((name, e)) = parse_cell(text) else {
                        continue;
                    };
                    if let Some(wid) = self.app.workers.get_id_by_name(name) {
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
            Action::MoveProjectBound { proj, bound, week } => {
                if move_project_bound(&mut self.app, proj, bound, week) {
                    self.mark_changed();
                }
            }
            Action::SetProjectCategory { proj, cat } => {
                self.app.projects.set_category(proj, cat);
                self.mark_changed();
            }
            Action::DelRow { proj, dev } => {
                self.app.projects.del_row(proj, dev);
                self.mark_changed();
            }
            Action::ClearEfforts { proj, dev } => {
                if self.app.projects.clear_efforts(proj, dev) {
                    self.mark_changed();
                }
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
            Action::SetProjectStatus { proj, status } => {
                self.app.projects.set_status(proj, status);
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
            Action::SetWorkerHiddenInFooter { worker, hidden } => {
                if self.app.workers.set_hidden_in_footer(worker, hidden) {
                    self.mark_changed();
                }
            }
            Action::SetWorkerShownInFind { worker, shown } => {
                if self.app.workers.set_shown_in_find(worker, shown) {
                    self.mark_changed();
                }
            }
            Action::SetDevHiddenInTotals { dev, hidden } => {
                if self.app.devs.set_hidden_in_totals(dev, hidden) {
                    self.mark_changed();
                }
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
            Action::SetBulkWeekLimit {
                week,
                hours,
                holiday,
            } => {
                self.app.set_bulk_week(week, hours, holiday);
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
                let eligible: Vec<ProjectId> = body_projects(
                    &self.app,
                    self.ui.project_view,
                    &ProjectFilter::from_state(&self.ui),
                );
                match eligible[..] {
                    [] => self
                        .ui
                        .toast_warn("Nessun progetto visibile: PDF non creato."),
                    [proj] => {
                        // Ripropone le ultime scelte fatte per questo progetto
                        // (in memoria); dev e milestone nuovi partono spuntati.
                        let mem = self.ui.export_project_memory.get(&proj);
                        let entries = restore_dev_entries(
                            &self.app.projects.list_devs(proj),
                            mem.map_or(&[][..], |m| &m.devs[..]),
                        );
                        // Milestone del progetto, in ordine di settimana (a parità
                        // di settimana, per id).
                        let mut milestones: Vec<(MilestoneId, WeekId, bool)> = self
                            .app
                            .projects
                            .list_project_milestones(proj)
                            .into_iter()
                            .map(|(id, w)| {
                                let on = mem
                                    .and_then(|m| m.milestones.get(&id).copied())
                                    .unwrap_or(true);
                                (id, w, on)
                            })
                            .collect();
                        milestones.sort_by_key(|(id, w, _)| (w.0, id.0));
                        // Formato per dev: quello ricordato, altrimenti l'ultimo
                        // formato scelto con «applica a tutti».
                        let formats = entries
                            .iter()
                            .map(|(d, _)| {
                                let f = mem
                                    .and_then(|m| m.formats.get(d).copied())
                                    .unwrap_or(self.ui.bar_format.into());
                                (*d, f)
                            })
                            .collect();
                        // Periodo: quello ricordato, altrimenti inizio/fine
                        // del progetto (poi non viene più toccato).
                        let range = mem
                            .and_then(|m| m.range)
                            .unwrap_or_else(|| project_export_range(&self.app, proj));
                        self.ui.pdf_export = Some(PdfExport {
                            proj,
                            entries,
                            milestones,
                            formats,
                            range,
                        });
                    }
                    _ => {
                        let mem = &self.ui.export_multi_memory;
                        let entries = eligible
                            .into_iter()
                            .map(|id| (id, mem.get(&id).copied().unwrap_or(true)))
                            .collect();
                        self.ui.pdf_multi_export = Some(PdfMultiExport { entries });
                    }
                }
            }
            Action::ExportTrend => {
                // PDF andamento nel tempo: una pagina per ogni progetto visibile
                // (abilitato + modalità Vista corrente) con dati di avanzamento.
                let visible = body_projects(
                    &self.app,
                    self.ui.project_view,
                    &ProjectFilter::from_state(&self.ui),
                );
                match crate::pdf_export::build_trend_pdf(&self.app, &visible) {
                    Some(bytes) => {
                        save_pdf_dialog(&mut self.ui, bytes, &dated_file_name("andamento", "pdf"))
                    }
                    None => self
                        .ui
                        .toast_warn("Nessun progetto con dati di avanzamento: PDF non creato."),
                }
            }
            Action::ExportPdfSelected { projects, scopes } => {
                crate::pdf_export::set_show_pct(self.ui.export_progress_pct);
                crate::pdf_export::set_show_year(self.ui.export_show_year.0);
                crate::pdf_export::set_show_ghost(self.ui.export_ghost.show);
                crate::pdf_export::set_ghost_red(self.ui.export_ghost.red);
                // Export multi-progetto: nessuna scelta milestone per progetto.
                crate::pdf_export::set_milestone_allow(None);
                // Un file per ambito milestone scelto (Internal e/o External).
                let items = scoped_exports(&scopes, |_| {
                    crate::pdf_export::build_pdf_selected(
                        &self.app,
                        &projects,
                        crate::pdf_export::DevBar::new(
                            self.ui.bar_format,
                            self.ui.export_dates_below,
                        ),
                    )
                });
                if items.is_empty() {
                    self.ui.toast_warn(
                        "Nessun progetto selezionato con inizio e fine: PDF non creato.",
                    );
                } else {
                    save_multi_export_dialog(
                        &mut self.ui,
                        "PDF",
                        "pdf",
                        &dated_file_name("progetti", "pdf"),
                        items,
                    );
                }
            }
            Action::ExportPdfProject {
                proj,
                devs,
                scopes,
                milestones,
                range,
            } => {
                crate::pdf_export::set_export_range(Some(range));
                crate::pdf_export::set_show_pct(self.ui.export_progress_pct);
                crate::pdf_export::set_show_year(self.ui.export_show_year.0);
                crate::pdf_export::set_show_ghost(self.ui.export_ghost.show);
                crate::pdf_export::set_ghost_red(self.ui.export_ghost.red);
                crate::pdf_export::set_milestone_allow(Some(milestones));
                let items = scoped_exports(&scopes, |_| {
                    crate::pdf_export::build_pdf_project_fmt(&self.app, proj, &devs)
                });
                if items.is_empty() {
                    self.ui
                        .toast_warn("Progetto senza inizio/fine: PDF non creato.");
                } else {
                    save_multi_export_dialog(
                        &mut self.ui,
                        "PDF",
                        "pdf",
                        &dated_file_name("progetto", "pdf"),
                        items,
                    );
                }
            }
            Action::ExportSvgProject {
                proj,
                devs,
                scopes,
                milestones,
                range,
            } => {
                crate::pdf_export::set_export_range(Some(range));
                crate::pdf_export::set_show_pct(self.ui.export_progress_pct);
                crate::pdf_export::set_show_year(self.ui.export_show_year.0);
                crate::pdf_export::set_show_ghost(self.ui.export_ghost.show);
                crate::pdf_export::set_ghost_red(self.ui.export_ghost.red);
                crate::pdf_export::set_milestone_allow(Some(milestones));
                let items = scoped_exports(&scopes, |_| {
                    crate::pdf_export::build_svg_project_fmt(&self.app, proj, &devs)
                    .map(String::into_bytes)
                });
                if items.is_empty() {
                    self.ui
                        .toast_warn("Progetto senza inizio/fine: SVG non creato.");
                } else {
                    save_multi_export_dialog(
                        &mut self.ui,
                        "SVG",
                        "svg",
                        &dated_file_name("grafico", "svg"),
                        items,
                    );
                }
            }
            Action::ExportWorkerMatrix { from, to, years } => {
                let m = crate::worker_matrix::build_filtered(
                    &self.app,
                    &self.ui.matrix_workers,
                    from,
                    to,
                    &self.ui.matrix_categories,
                    &self.ui.matrix_statuses,
                );
                let categories: Vec<Option<CategoryId>> =
                    self.ui.matrix_categories.iter().copied().collect();
                match crate::pdf_export::build_matrix_pdf(
                    &self.app,
                    &m,
                    from,
                    to,
                    &years,
                    &categories,
                ) {
                    Some(bytes) => {
                        save_pdf_dialog(&mut self.ui, bytes, &dated_file_name("matrice", "pdf"))
                    }
                    None => self
                        .ui
                        .toast_warn("Nessun progetto da esportare: matrice non creata."),
                }
            }
            Action::GenerateReport { projects } => {
                match crate::pdf_export::build_report_pdf(&self.app, &projects) {
                    Some(bytes) => {
                        save_pdf_dialog(&mut self.ui, bytes, &dated_file_name("report", "pdf"))
                    }
                    None => self
                        .ui
                        .toast_warn("Nessun progetto selezionato: report non creato."),
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
            Action::CreateMilestone(name, kind, categories) => {
                self.app.milestones.add_with(&name, kind, categories);
                self.mark_changed();
            }
            Action::CreateProjectMilestone {
                proj,
                week,
                name,
                categories,
            } => {
                // Tipo di default (traguardo) e colore automatico: si cambiano
                // poi dal gestore. Le categorie si scelgono qui nel menù.
                let id = self.app.milestones.add_owned(
                    &name,
                    MilestoneKind::default(),
                    categories,
                    Some(proj),
                );
                self.app.projects.add_project_milestone(proj, id, week);
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
            Action::SetMilestoneCategory {
                milestone,
                category,
                on,
            } => {
                self.app.milestones.set_category(milestone, category, on);
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
            Action::ClearProjectMilestones { proj } => {
                if self.app.projects.clear_project_milestones(proj) {
                    self.mark_changed();
                }
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

/// Autocompletamento: completa `typed`; se nessun match tieni il digitato, altrimenti
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

/// Giorni "colorati" di una settimana: lunedì–venerdì, cioè le 5 parti in cui
/// si divide la cella per la velatura dei mesi.
pub(crate) const MONTH_BAND_DAYS: usize = 5;

/// Suddivisione della cella di una colonna nelle bande dei mesi che la
/// attraversano. Ogni settimana vale `MONTH_BAND_DAYS` parti uguali (lun–ven) e
/// i giorni consecutivi dello stesso mese sono raggruppati: una settimana tutta
/// dentro un mese dà una sola banda `(mese, 0.0, 1.0)`, una a cavallo di due
/// mesi dà la parte sinistra col mese che sta finendo, larga quanti sono i suoi
/// giorni, e la restante col mese nuovo. Le frazioni sono relative alla
/// larghezza della cella, così vale anche per le colonne mergiate (zoom), dove
/// le settimane del gruppo si accodano.
pub(crate) fn month_bands(ws: &[i32]) -> Vec<(u32, f32, f32)> {
    let mut days: Vec<u32> = Vec::with_capacity(ws.len() * MONTH_BAND_DAYS);
    for w in ws {
        let first = primo_giorno_settimana_corrente(&days_to_local(*w));
        for i in 0..MONTH_BAND_DAYS as u64 {
            days.push((first + chrono::Days::new(i)).month());
        }
    }
    let n = days.len() as f32;
    let mut out: Vec<(u32, f32, f32)> = Vec::new();
    for (i, m) in days.into_iter().enumerate() {
        let to = (i + 1) as f32 / n;
        match out.last_mut() {
            Some((last, _, end)) if *last == m => *end = to,
            _ => out.push((m, i as f32 / n, to)),
        }
    }
    out
}

/// Disegna la velatura dei mesi su una cella di intestazione (in alto o nel
/// footer) e ritorna lo sfondo che ne risulta **al centro** della cella, cioè
/// quello sotto l'etichetta della data: serve a scegliere il colore del testo.
pub(crate) fn paint_month_tint(ui: &egui::Ui, cell: Rect, ws: &[i32]) -> Color32 {
    let mut center = bg();
    for (month, from, to) in month_bands(ws) {
        let tint = month_tint(month);
        // Bordi arrotondati al pixel: bande adiacenti combaciano senza filetti.
        let x0 = (cell.left() + cell.width() * from).round();
        let x1 = (cell.left() + cell.width() * to).round();
        let band = Rect::from_min_max(
            egui::pos2(x0, cell.top()),
            egui::pos2(x1.max(x0), cell.bottom()),
        );
        ui.painter().rect_filled(band, 0.0, tint);
        if from <= 0.5 && 0.5 < to {
            center = blend(bg(), tint);
        }
    }
    center
}

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
    if !(start_y <= year_ending && end_y > year_ending) {
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

/// Filtro categoria (Ctrl+K): `None` = nessun filtro, `Some(set)` = solo i
/// progetti con una di queste categorie; la voce `None` del set è «Senza
/// categoria» (progetto a cui non è stata assegnata alcuna categoria).
pub(crate) type CategoryFilter = Option<HashSet<Option<CategoryId>>>;

/// Predicato unico del filtro categoria, usato da `project_layout` (griglia +
/// colonna sinistra) e da `body_projects` (elenchi di export): agisce sul
/// progetto intero. Senza filtro attivo non nasconde nulla.
pub(crate) fn category_shown(app: &App, proj: ProjectId, filter: &CategoryFilter) -> bool {
    match filter {
        None => true,
        Some(set) => set.contains(&app.projects.get_category(proj)),
    }
}

/// Filtro anno (Ctrl+Y): `None` = nessun filtro, `Some(set)` = solo gli anni del set.
pub(crate) type YearFilter = Option<HashSet<i32>>;

/// Predicato del filtro anno: il progetto passa se soddisfa `crit` per **almeno
/// uno** degli anni selezionati. Un progetto senza la data richiesta dal criterio
/// (inizio per `Starts`, fine per `Ends`, entrambe per `Within`) è nascosto
/// quando il filtro è attivo. Senza filtro attivo non nasconde nulla.
pub(crate) fn year_shown(app: &App, proj: ProjectId, filter: &YearFilter, crit: YearCriterion) -> bool {
    let Some(set) = filter else {
        return true;
    };
    let year = |w: Option<WeekId>| w.map(|w| days_to_local(w.0 as i32).year());
    let start = year(app.projects.get_project_start_week(proj));
    let end = year(app.projects.get_project_end_week(proj));
    match crit {
        YearCriterion::Starts => start.is_some_and(|y| set.contains(&y)),
        YearCriterion::Ends => end.is_some_and(|y| set.contains(&y)),
        YearCriterion::Within => matches!((start, end), (Some(s), Some(e)) if s == e && set.contains(&s)),
    }
}

/// Filtri che agiscono su **progetti interi** (categoria + anno): l'unica regola
/// condivisa da `project_layout` (griglia + colonna sinistra) e `body_projects`
/// (elenchi di export). Non comprimono l'header, a differenza di worker/dev.
#[derive(Clone, Default)]
pub(crate) struct ProjectFilter {
    pub(crate) category: CategoryFilter,
    pub(crate) years: YearFilter,
    pub(crate) criterion: YearCriterion,
}

impl ProjectFilter {
    pub(crate) fn from_state(state: &UiState) -> Self {
        Self {
            category: state.category_filter.clone(),
            years: state.year_filter.clone(),
            criterion: state.year_criterion,
        }
    }

    pub(crate) fn shows(&self, app: &App, proj: ProjectId) -> bool {
        category_shown(app, proj, &self.category)
            && year_shown(app, proj, &self.years, self.criterion)
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
/// Righe di una cella-settimana del dev: `(testo "Worker|ore", nota, worker)`,
/// ordinate per testo e completate con righe vuote fino a `max_rows`. Il worker
/// viaggia con il testo, così chi disegna non deve ricavarlo di nuovo dal nome.
fn gather_slots(
    app: &App,
    proj: ProjectId,
    dev: DevId,
    week: i32,
    max_rows: usize,
    filter: &Filter,
) -> Vec<(String, String, Option<WorkerId>)> {
    let mut slots: Vec<(String, String, Option<WorkerId>)> = app
        .projects
        .get_single_dev(proj, dev)
        .and_then(|sd| sd.get_all(WeekId(week as usize)))
        .map(|sew| {
            let mut v: Vec<(String, String, Option<WorkerId>)> = sew
                .worker_id
                .iter()
                .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                .filter(|(wid, _)| worker_shown(filter, app.workers.get_name_by_id(**wid)))
                .map(|(wid, se)| {
                    let name = app.workers.get_name_by_id(*wid);
                    (
                        format!("{}|{}", name, se.get_effort().0),
                        se.get_note(),
                        Some(*wid),
                    )
                })
                .collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        })
        .unwrap_or_default();
    while slots.len() < max_rows {
        slots.push((String::new(), String::new(), None));
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

/// Categorie effettive di una milestone in fase di **creazione** (l'elenco in
/// `UiState.new_milestone_categories`): elenco vuoto = solo Internal, come per
/// le milestone già salvate.
pub(crate) fn effective_categories(cats: &[MilestoneCategory]) -> Vec<MilestoneCategory> {
    let out: Vec<MilestoneCategory> = MilestoneCategory::ALL
        .into_iter()
        .filter(|c| cats.contains(c))
        .collect();
    if out.is_empty() {
        vec![MilestoneCategory::Internal]
    } else {
        out
    }
}

/// Voci del selettore delle **categorie di stampa** (Internal / External),
/// condivise dal sottomenù di creazione e dal gestore milestone. `cats` è
/// l'elenco effettivo (mai vuoto); `compact` usa le etichette brevi Int/Ext.
/// Ritorna `Some((categoria, nuovo_stato))` quando l'utente cambia una spunta.
///
/// L'ultima categoria rimasta non si può togliere: una milestone senza
/// categorie varrebbe comunque Internal, quindi la spunta non risponderebbe.
fn milestone_category_items(
    ui: &mut egui::Ui,
    cats: &[MilestoneCategory],
    compact: bool,
) -> Option<(MilestoneCategory, bool)> {
    let mut changed = None;
    for c in MilestoneCategory::ALL {
        let on = cats.contains(&c);
        let last = on && cats.len() == 1;
        let text = if compact { c.short() } else { c.label() };
        let resp = ui
            .add_enabled(!last, egui::Button::selectable(on, text))
            .on_hover_text(match c {
                MilestoneCategory::Internal => "Stampata nel PDF Internal (e nel Mix)",
                MilestoneCategory::External => "Stampata nel PDF External (e nel Mix)",
            })
            .on_disabled_hover_text("Una milestone deve avere almeno una categoria.");
        if resp.clicked() {
            changed = Some((c, !on));
        }
    }
    changed
}

/// Aggiunge/toglie una categoria da un elenco in fase di creazione, applicando
/// la stessa regola del modello: l'elenco risultante è ordinato, senza doppioni
/// e mai vuoto (togliendo l'ultima si ricade su Internal).
pub(crate) fn toggle_category(
    cats: &[MilestoneCategory],
    cat: MilestoneCategory,
    on: bool,
) -> Vec<MilestoneCategory> {
    let mut next: Vec<MilestoneCategory> = cats.iter().copied().filter(|x| *x != cat).collect();
    if on {
        next.push(cat);
    }
    effective_categories(&next)
}

/// Selettore delle categorie per i **menù** (Aggiungi ▸ Milestone nella toolbar
/// e «Aggiungi milestone qui» nella griglia): un sottomenù «Categorie: … ⏵»,
/// come quello del tipo — dentro un menù serve un sottomenù e non una combo
/// (vedi `milestone_kind_combo`). Ritorna la `Response` del pulsante (serve ai
/// test per aprirlo).
pub(crate) fn milestone_categories_submenu(
    ui: &mut egui::Ui,
    cats: &mut Vec<MilestoneCategory>,
) -> egui::Response {
    let eff = effective_categories(cats);
    let label = eff
        .iter()
        .map(|c| c.label())
        .collect::<Vec<_>>()
        .join(" + ");
    egui::containers::menu::SubMenuButton::new(format!("Categorie: {label}"))
        .config(
            egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
        )
        .ui(ui, |ui| {
            if let Some((c, on)) = milestone_category_items(ui, &eff, false) {
                *cats = toggle_category(&eff, c, on);
            }
        })
        .0
}

/// Selettore compatto (Int / Ext) per una riga del gestore milestone. Ritorna
/// il cambio richiesto dall'utente, da tradurre in `Action::SetMilestoneCategory`.
pub(crate) fn milestone_categories_toggles(
    ui: &mut egui::Ui,
    cats: &[MilestoneCategory],
) -> Option<(MilestoneCategory, bool)> {
    let mut changed = None;
    ui.horizontal(|ui| {
        changed = milestone_category_items(ui, cats, true);
    });
    changed
}

/// Larghezza della combo del tipo milestone: tiene "Traguardo"/"Trigger" senza
/// che la tendina cambi larghezza tra le due voci.
const MILESTONE_KIND_COMBO_W: f32 = 78.0;

/// Regola generale degli elenchi di progetti: se ne vedono al massimo 15,
/// oltre si scorre (altrimenti su schermi piccoli la finestra cresce e i
/// bottoni in fondo, es. «Esporta…», escono dallo schermo).
pub(crate) const MAX_VISIBLE_PROJECTS: usize = 15;

/// Altezza massima di un elenco di progetti: `MAX_VISIBLE_PROJECTS` righe.
/// Il passo di riga è quello misurato al frame precedente da
/// `remember_project_rows` (stessa `key`), perché dipende dal widget della
/// riga (checkbox, combo…) e dal font; al primo frame una stima.
pub(crate) fn project_list_max_height(ui: &egui::Ui, key: egui::Id) -> f32 {
    rows_max_height(ui, key, MAX_VISIBLE_PROJECTS)
}

/// Come `project_list_max_height`, ma per `rows` righe: altezza massima di un
/// elenco che oltre quel numero di righe deve scorrere.
pub(crate) fn rows_max_height(ui: &egui::Ui, key: egui::Id, rows: usize) -> f32 {
    let pitch = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(key.with("row_pitch")))
        .unwrap_or_else(|| ui.spacing().interact_size.y + ui.spacing().item_spacing.y);
    rows as f32 * pitch
}

/// Righe visibili degli elenchi dev e milestone della dialog «Esporta PDF» di
/// un solo progetto: oltre si scorre.
pub(crate) const MAX_VISIBLE_EXPORT_ROWS: usize = 10;

/// Da chiamare dopo aver disegnato le `rows` righe di un elenco di progetti
/// iniziato all'altezza `top`: ne ricorda il passo per `project_list_max_height`.
pub(crate) fn remember_project_rows(ui: &egui::Ui, key: egui::Id, top: f32, rows: usize) {
    if rows > 0 {
        let pitch = (ui.cursor().top() - top) / rows as f32;
        ui.ctx()
            .data_mut(|d| d.insert_temp(key.with("row_pitch"), pitch));
    }
}

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

    // Al massimo `MAX_VISIBLE_PROJECTS` righe, poi si scorre; e comunque non
    // più dello spazio disponibile nella finestra, meno quello dei controlli
    // che restano SOTTO (opzioni + bottoni): misurato al frame precedente da
    // `checklist_dialog_end`, con 140 px di partenza. Con pochi progetti si
    // restringe al contenuto (`auto_shrink` verticale) evitando un riquadro vuoto.
    let key = ui.id().with("project_checklist");
    let reserve = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(key.with("reserve")))
        .unwrap_or(140.0);
    let max_h = (ui.available_height() - reserve)
        .max(120.0)
        .min(project_list_max_height(ui, key));
    egui::ScrollArea::vertical()
        .max_height(max_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let top = ui.cursor().top();
            for (proj, sel) in entries.iter_mut() {
                ui.checkbox(sel, minuta_project_label(app, *proj));
            }
            remember_project_rows(ui, key, top, entries.len());
        });
    let bottom = ui.cursor().top();
    ui.ctx()
        .data_mut(|d| d.insert_temp(key.with("bottom"), bottom));
}

/// Da chiamare in fondo al contenuto di un dialog che usa `project_checklist`
/// (stesso `ui`): misura quanto spazio occupano i controlli sotto l'elenco,
/// così al frame dopo l'elenco lo lascia libero e i bottoni restano a schermo.
pub(crate) fn checklist_dialog_end(ui: &egui::Ui) {
    let key = ui.id().with("project_checklist");
    let ctx = ui.ctx();
    if let Some(bottom) = ctx.data(|d| d.get_temp::<f32>(key.with("bottom"))) {
        let below = ui.min_rect().bottom() - bottom;
        let margin = ui.spacing().window_margin.bottom as f32 + ui.spacing().item_spacing.y;
        ctx.data_mut(|d| d.insert_temp(key.with("reserve"), (below + margin).max(0.0)));
    }
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
        .get(dev)
        .map(|d| from_hex(d.bg_color() as u32))
        .unwrap_or(g(Color32::from_rgb(0x00, 0x99, 0xFF)))
}

fn dev_text_color(app: &App, dev: DevId) -> Color32 {
    app.devs
        .get(dev)
        .map(|d| from_hex(d.font_color() as u32))
        .unwrap_or(text())
}

/// Voci «Sposta/Imposta inizio|fine progetto qua» del menù della griglia per la
/// settimana `week`: `(estremo, etichetta, abilitata)`. Dentro il periodo ci
/// sono entrambe, prima dell'inizio solo l'inizio, dopo la fine solo la fine;
/// una data mancante diventa «Imposta …». La voce che non cambierebbe nulla
/// (inizio già lì) resta visibile ma disabilitata.
pub(crate) fn bound_menu_items(
    app: &App,
    proj: ProjectId,
    week: WeekId,
) -> Vec<(ProjectBound, &'static str, bool)> {
    let start = app.projects.get_project_start_week(proj);
    let end = app.projects.get_project_end_week(proj);
    let mut v = Vec::new();
    if end.is_none_or(|e| week <= e) {
        let label = if start.is_some() {
            "Sposta inizio progetto qua"
        } else {
            "Imposta inizio progetto qua"
        };
        v.push((ProjectBound::Start, label, start != Some(week)));
    }
    if start.is_none_or(|s| week >= s) {
        let label = if end.is_some() {
            "Sposta fine progetto qua"
        } else {
            "Imposta fine progetto qua"
        };
        v.push((ProjectBound::End, label, end != Some(week)));
    }
    v
}

/// Effort (celle, anche a 0) e milestone del progetto che resterebbero fuori
/// dal periodo se `bound` andasse a `week`. Riguarda tutti i dev.
pub(crate) fn bound_move_loss(
    app: &App,
    proj: ProjectId,
    bound: ProjectBound,
    week: WeekId,
) -> BoundLoss {
    let (start, end) = match bound {
        ProjectBound::Start => (Some(week), app.projects.get_project_end_week(proj)),
        ProjectBound::End => (app.projects.get_project_start_week(proj), Some(week)),
    };
    let outside = |w: WeekId| start.is_some_and(|s| w < s) || end.is_some_and(|e| w > e);
    let mut loss = BoundLoss::default();
    for dev in app.projects.list_devs(proj) {
        if let Some(sd) = app.projects.get_single_dev(proj, dev) {
            loss.efforts.extend(
                sd.get_weeks()
                    .into_iter()
                    .filter(|w| outside(*w))
                    .map(|w| (dev, w)),
            );
        }
    }
    let mut ms: Vec<_> = app
        .projects
        .list_project_milestones(proj)
        .into_iter()
        .filter(|(_, w)| outside(*w))
        .collect();
    ms.sort_by_key(|(id, w)| (*w, id.0));
    loss.milestones = ms;
    loss
}

/// Sposta `bound` del progetto a `week`, cancellando prima quanto resterebbe
/// fuori (vedi `bound_move_loss`). Ritorna `true` se ha cambiato qualcosa.
fn move_project_bound(app: &mut App, proj: ProjectId, bound: ProjectBound, week: WeekId) -> bool {
    let loss = bound_move_loss(app, proj, bound, week);
    for (dev, w) in &loss.efforts {
        app.projects.reset_effort(proj, *dev, *w);
    }
    for (m, _) in &loss.milestones {
        app.projects.remove_project_milestone(proj, *m);
    }
    let old = match bound {
        ProjectBound::Start => app.projects.get_project_start_week(proj),
        ProjectBound::End => app.projects.get_project_end_week(proj),
    };
    match bound {
        ProjectBound::Start => app.projects.set_project_start_week(proj, Some(week)),
        ProjectBound::End => app.projects.set_project_end_week(proj, Some(week)),
    }
    old != Some(week) || !loss.is_empty()
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
    app.devs.get(dev).map(|d| d.get_name()).unwrap_or_default()
}

// ── Colonna sinistra ────────────────────────────────────────────────────────

#[cfg(test)]
mod test_util;
#[cfg(test)]
pub(crate) use test_util::*;
#[cfg(test)]
mod tests;
