# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> User-facing manual: see **`docs/MANUALE.md`** (Italian, complete). It is embedded
> into the app via `include_str!` and shown by the in-app Help window
> (**Aiuto ▸ Manuale d'uso…**). Keep it in sync when you change user-visible behavior.
> The `{{VERSION}}` placeholder in the manual is substituted at runtime with
> `CARGO_PKG_VERSION` (don't hardcode a version). The in-app TOC is **not** the
> Markdown index (egui_commonmark can't follow `#anchor` links): `help_window`
> rebuilds a clickable chapter list from the `## N.` headings (`chapter_title`) and
> scrolls with `scroll_to_rect`, hiding the Markdown `## Indice` (`is_index_section`).

## Commands

```bash
# Build
cargo build

# Run (loads workers.ron from the working directory)
cargo run

# Run with a specific data file
cargo run -- myfile.ron

# Run all tests
cargo test

# Run a single test
cargo test <test_name>
# e.g.: cargo test single_project_with_effortless_dev_produces_valid_pdf
```

The default data file is `workers.ron`, loaded from the working directory on
startup; a different file can be passed as a CLI argument. **File ▸ Apri…** opens a
native file-picker (via the `rfd` crate) filtered to `.ron` files.

## Architecture

This is a **Rust + egui/eframe** project management effort tracker (immediate-mode
GUI). It was originally written in Slint and has since been **fully rewritten in
egui** — there are no `.slint` files and no `build.rs`. Ignore any lingering Slint
references; the UI lives entirely in the `src/ui/` module.

### Source layout

- `src/main.rs` — module list, `eframe::run_native`, loads the RON file, launches `PjmApp`.
- `src/app.rs` — `App`: the whole persisted state; RON (de)serialization (`to_ron_string` / `from_ron_str`).
- `src/ui/` — the UI, split into a module (was one big `ui.rs`; see improvement #16):
  - `mod.rs` — the **core**: `eframe::App` (`PjmApp`), `UiState`, the `Action` enum + apply
    loop, all shared **types** (`Editing`/`Popup`/`Col`/`ProjLayout`/…), and shared helpers
    (`columns_vec`, `col_*`, `dev_*`, `project_in_body`, `select_all_checkbox`,
    `project_checklist`, save dialogs, `weeks_vec`…). The `#[cfg(test)]` tests live here.
  - `toolbar.rs` (`toolbar`/`header`/`add_field`), `grid.rs` (`body`/`grid`/`left_column`/
    `draw_dev_cells`/`project_layout`/`draw_project_info`/paint helpers…), `footer.rs`
    (`footer`/`draw_left_footer`/`draw_right_footer`…), `dialogs.rs` (all the modeless
    `*_window` fns + `move`/`popup` helpers), `export.rs` (PDF/SVG/minuta dialogs +
    `bar_format_selector`/`draw_bar_format_preview`), `help.rs` (manual window + parsing),
    `saturation.rs` (worker-saturation dashboard), `filters.rs` (dialog unica dei
    filtri Workers/Progetti/Dev), `toasts.rs` (notifiche in-app).
  - **`draw_dev_cells` is only the column-loop orchestrator** (~165 lines): each case of
    a column lives in a **private** helper of `grid.rs` (not re-exported) —
    `draw_year_end_cell`, `milestones_in_group` + `draw_milestone_strip` (the milestone /
    «Sposta» context menu, tooltip-only when `merged`), `draw_compact_bar`,
    `draw_cumulative_row`, `draw_merged_cell`, and the two branches of a worker cell,
    `draw_cell_editor` (keyboard, autocomplete, copy/paste, caret) and `draw_cell_static`
    (colors, click → editing, right-click → note / bulk fill). The cell helpers take a
    `CellCtx { proj, dev, week, row }` instead of four loose parameters. All the rects are
    derived from the column's `col_rect`, so the geometry stays in one place.
  - **Module mechanics** (mechanical split, verified by the compiler): only free **functions**
    moved to submodules; every shared **type/enum/const stays in `mod.rs`**. Submodules do
    `use super::*` (so they see the core's items and, since the top-of-file crate imports were
    made `pub(crate) use`, the external types too); `mod.rs` re-exports each submodule with
    `pub(crate) use <m>::*` so the parent and siblings can call moved fns. Moved fns are
    `pub(crate)`, as are the types they expose (`Action`/`Col`/`ProjLayout`/… — otherwise
    `private_interfaces` warns). Note the `include_str!` for the manual is `../../docs/…` now.
- `src/ui_style.rs` — sizes, fonts, colors, theme (light/dark) and B/W mode, `cumulative_color`.
- `src/pdf_export.rs` — PDF Gantt export (`build_pdf`, `build_pdf_project`).
- Data modules: `workers_utils/`, `dev_utils/`, `project_utils/`, `single_dev_utils/`, `single_effort_utils/`, `categories.rs`, `milestones.rs`, `date_utils/`, `sync_merge.rs`.

### Data Model

```
App (workers.ron)
├── start_week / end_week : WeekId   — grid time range
├── week_color: String               — "#RRGGBB" current-week highlight (file-only setting)
├── month_colors: Vec<String>        — 12 "#RRGGBB", Jan→Dec, date-row tint (file-only)
├── month_tint_pct: i32              — month tint strength 0..100 (default 60, file-only)
├── workers   : Workers              — named people (max hours, colors, hidden-in-footer, show_in_find, ghost)
├── devs      : Devs                 — roles (e.g. "Frontend"); each has bg+font color
├── categories: Categories
├── milestones: Milestones           — name + color + kind, shared across projects
├── sovra     : HashMap<(WeekId, WorkerId), Effort>  — per-week per-worker allocation
└── projects  : Projects
    └── Project (tripletta, name, start, end, category, enable, closed, milestones)
        └── SingleDev (per DevId)
            ├── effort : Effort              — planned hours
            ├── note   : Option<String>      — dev-level note
            ├── hide_effort : bool
            ├── declared_history : Vec<DeclaredPoint{week,pct}>  — dev-declared progress %
            │                       over time (one entry per week, latest = current value)
            └── weeks  : HashMap<WeekId, SingleEffortWeek>
                └── worker_id : HashMap<WorkerId, SingleEffort>  { effort, note }
```

- **`WeekId(usize)` is a day number, not a week index.** Adjacent weeks are 7 apart (`WEEK_STEP = 7`). The displayed label is the first day of the week (`%y-%m-%d`).
- `Effort(usize)` stores hours directly (not a percentage).
- Cell display format: `"WorkerName|effort"` (the `|` separator is parsed in Rust).

### Immediate-mode data flow

There is no retained widget tree and no binding layer. Each frame:

1. `PjmApp::ui()` runs. It creates a fresh `let mut actions: Vec<Action> = …`.
2. All panels/widgets are drawn from the current `&App` (read-only) and push
   `Action`s into `actions` in response to input.
3. After drawing, the actions are applied: each `Action` mutates `self.app` and,
   for anything user-visible, calls `mark_changed()`.

So the pattern is **draw from state → collect `Action`s → apply `Action`s**. When
adding a feature that mutates data, add an `Action` variant + a handler; don't
mutate `App` directly inside drawing code.

### Coordinate-based rendering

The grid, left column and footer are **not** built from nested egui widgets. Each
does a single `ui.allocate_exact_size(...)` and then paints everything at absolute
coordinates via `ui.painter()`. Interactions use `ui.interact(rect, id, sense)`.
This is why layout math is explicit and shared (see below).

### Shared column axis and layout

To keep header, grid and footer perfectly aligned (they share horizontal scroll):

- **`columns_vec(app, level) -> Vec<Col>`** is the single source of truth for the
  column axis. `Col::Weeks(Vec<i32>)` is one grid column (one or more weeks merged
  by the zoom level; length 1 at zoom 0), `Col::YearEnd(i32)` is a narrow canary
  year-boundary column. Helpers: `col_width`, `cols_width`, `col_x_offset`.
- **`project_layout(ui, app, filter, compact, merged) -> Vec<ProjLayout>`** is the
  single source of truth for row heights, shared by `left_column` and `grid` so the
  two columns can never diverge. `dev_inner_h` / `dev_block_height` compute per-dev
  block heights.

### Theme and colors (`ui_style.rs`)

- Colors that depend on the theme are **functions**, not consts: `bg()`, `text()`,
  `text_dim()`, `text_faint()`, `row_even()`, `row_alt()`, `strip_bg()`,
  `strip_border()`, plus footer semantics `ok_green()`, `zero_yellow()`,
  `override_brown()`. They read a `DARK_THEME` thread-local set once per frame by
  `set_dark_theme(dark)`.
- Accent constants (e.g. `EFFORT_ORANGE`, `START_BG`, `DEADLINE_BG`, `CANARY`) stay
  theme-independent. **Text drawn on the always-yellow canary column must be
  `Color32::BLACK`**, not `bg()`.
- `g(color)` applies the B/W (grayscale) transform when B/W mode is on (`set_bw_mode`
  per frame). Wrap accent colors in `g(...)`; theme functions already return final
  colors.
- **Current-week highlight** — the only **file-configured** color: `App.week_color`
  (`"#RRGGBB"`, `#[serde(default = "default_week_color")]`, **always serialized**, no
  UI to change it). `App::week_color_rgb()` parses it (`ui_style::parse_hex_rgb`,
  falling back to `THIS_WEEK_DEFAULT` = `0xCCFF00` neon yellow; an unparsable value is
  reported by `App::validate` in the "Problema nel file" window). `PjmApp::ui()` calls
  `set_this_week_color(app.week_color_rgb())` once per frame next to `set_bw_mode`/
  `set_dark_theme`; drawing code reads the **function** `this_week()` (there is no
  `THIS_WEEK` const anymore) — grid column tint, footer column tint **and each footer
  worker cell** (the alternating `row_even()/row_alt()` fill is opaque and would cover
  the column tint, so the cell repaints it), plus the week header in `toolbar.rs`.
- **Month tint on the date rows** — same file-only mechanism: `App.month_colors`
  (`Vec<String>` of 12 `"#RRGGBB"`, Jan→Dec, always serialized) →
  `App::month_colors_rgb() -> [u32; 12]` (**per-month** fallback to
  `MONTH_COLORS_DEFAULT`, so a short or partly broken list keeps the other months;
  `validate` reports the bad entries) → `set_month_colors(...)` once per frame →
  `month_tint(month)` = `g(month_color(m))` at the strength of `App.month_tint_pct`
  (0–100, default `MONTH_TINT_PCT_DEFAULT` = 60, clamped by `set_month_tint_pct`, also
  set once per frame; out-of-range is reported by `validate`). Both the header cell
  (`toolbar.rs`) and the footer's date row (`footer.rs`) paint it with the shared
  `paint_month_tint(ui, cell, ws)` (`mod.rs`), **under** the current-week tint.
  A week is **5 parts** (Mon–Fri, `MONTH_BAND_DAYS`): `month_bands(ws)` groups those
  days by month and returns `(month, from, to)` fractions, so a week straddling two
  months is drawn as two bands sized by the days each month owns (a month change on
  Sat/Sun leaves one band); on merged (zoom) columns the group's weeks are concatenated
  and the fractions span the whole cell. Because the tint can be strong,
  `paint_month_tint` returns the background of the band **at the cell centre** (under
  the label), rebuilt with `blend(base, over)` (`Color32` is **premultiplied**, so
  `over`'s components are added to the attenuated `base`); the date is then drawn with
  `contrast_text(...)` instead of `text()`.
- Theme is resolved at the top of `PjmApp::ui()` from `UiState.theme_pref`
  (`Auto`/`Light`/`Dark`); `Auto` reads `ctx.system_theme()`. egui's own `Visuals`
  are set to match so menus/popups/text-edits follow the theme too.

### Toolbar menus (in `toolbar`)

- **File**: Salva (`Cmd/Ctrl+S`), Apri…, Esporta… (PDF Gantt), Andamento… (PDF trend % nel tempo), Minuta… (esporta note progetti in Markdown), Esci.
  - **Minuta** (`minuta_window` + `build_minuta` in `export.rs`, stato `MinutaState`):
    note settimanali di progetto, più recenti prima. La spunta **«Includi le note dei
    worker»** (`MinutaState.worker_notes` → `Action::GenerateMinuta.worker_notes`,
    default off) aggiunge sotto ogni settimana le note delle **celle effort**
    (`SingleEffort.note`) come `- **Worker** (Dev): testo`, raccolte da
    `project_worker_notes` (ordine: dev del progetto, poi nome worker; righe extra
    rientrate di 2 spazi da `worker_note_bullet`). Con l'opzione attiva una settimana
    con **solo** note di cella compare lo stesso, e quelle note contano anche per il
    filtro «Solo progetti con note». Le note **Dev** (`SingleDev.note`) e quelle
    **worker/settimana del footer** (`Worker.week_notes`) restano fuori dalla minuta.
- **Aggiungi**: + Progetto, and `add_field` inputs for Worker / Dev / Categoria / Milestone.
- **Filtri**: le prime quattro voci (Progetti… `Cmd/Ctrl+P`, Workers… `Cmd/Ctrl+F`,
  Workers (settimana corrente)… `Cmd/Ctrl+G`, Dev… `Cmd/Ctrl+D`) aprono **un'unica
  dialog** `filters_window` (`src/ui/filters.rs`, `UiState.show_filters`) a **tre
  colonne** — Workers | Progetti | Dev — ognuna con campo di ricerca, Select All ed
  elenco con checkbox. La scorciatoia/voce di menù decide solo quale colonna riceve il
  focus (`UiState.filters_focus: Option<FilterPane>` + `filters_focus_dirty` one-shot;
  helper `open_filters`/`focus_pane`/`toggle_filters`). Ripremuta a dialog aperta, la
  stessa scorciatoia fa il **toggle di Select All** della sua colonna
  (`*_filter_toggle_all`), applicato agli elementi **attualmente elencati** (rispetta la
  ricerca); `Shift+…` deseleziona tutto senza aprire. **`Cmd/Ctrl+J`** →
  `reset_all_filters(app, state, actions)`: rimette `worker_filter`/`dev_filter` a
  `None`, spegne `worker_filter_current_week`, svuota le tre ricerche e riabilita tutti
  i progetti **non chiusi** (`Action::SetProjectEnabled`, che non marca il file come
  modificato). È **idempotente**: ripremuto non deseleziona nulla. Dettagli per colonna:
  - **Workers** — elenca solo i worker con `Worker.show_in_find` true (default true; la
    visibilità nel footer non conta); accanto a ogni nome il conteggio progetti
    `N/Aperti - M/Chiuso` via `worker_project_counts(app, current_week_only)` = progetti
    distinti in cui il worker è **assegnato** (anche a effort 0 — conta l'assegnazione,
    non le ore), split per `is_closed`, ignora vista/filtri.
    La spunta **«Solo settimana corrente»** è `UiState.worker_filter_current_week`
    (Ctrl+G): mostra solo i progetti in cui un worker selezionato è **assegnato nella
    settimana corrente** (`current_week_id()`, anche con effort 0); dentro il progetto la resa resta identica
    a Ctrl+F (tutte le settimane). Ctrl+F disattiva la modalità, Ctrl+G la riattiva; con
    la dialog aperta nella *altra* modalità la scorciatoia commuta soltanto, nella stessa
    fa il toggle di Select All. Il gate è il predicato `project_worker_in_current_week`
    applicato in `project_layout`, unica sorgente di grid+left_column. Non persistito.
  - **Progetti** — visibilità enable/disable + ricerca/salto per tripletta (click sulla
    tripletta o Invio → `jump_to_project`, chiude la dialog e azzera la ricerca).
  - **Dev** — `UiState.dev_filter: Option<HashSet<DevId>>`: mostra solo i progetti in cui
    un dev selezionato ha **almeno un worker assegnato** (`SingleDev::has_any_worker()`,
    anche a effort 0; resta escluso il dev mai compilato) e, dentro il progetto,
    **solo quei dev**.
    Predicato unico `dev_shown(app, proj, dev, &dev_filter)` applicato in
    `project_layout`; si combina in **AND** col filtro worker e, come quello, comprime
    l'header progetto alla sola tripletta (`filter_active`). Non persistito.

  Restano finestre a sé: Milestone… (manager), Ghost worker… (`ghost_manager_window`:
  elenca **tutti** i worker con spunta `Worker.ghost`, indipendente da
  hide_in_footer/show_in_find/filtro — unico punto sempre raggiungibile per il ghost),
  Closed…
- **Vista**: Vista compatta, Bianco/Nero, **Progetti** (Solo aperti `Cmd/Ctrl+1` / Solo chiusi `Cmd/Ctrl+2` / Tutti `Cmd/Ctrl+3` — `UiState.project_view: ProjectViewMode`, non persistito), **Tema** (Auto/Chiaro/Scuro), **Zoom settimane** (Normale/2/4), Saturazione worker… (dashboard read-only: `saturation_window`; mostra i worker con `show_in_find` true **o** non nascosti nel footer, ignorando il filtro Ctrl+F).
- **Aiuto**: Manuale d'uso… (opens `help_window`).

### Dialogs / windows

Each modeless window is a `fn xxx_window(ctx, …)` guarded by a `UiState` field and
called from the windows block in `ui()`. Examples: `dev_manage_window`
(`state.dev_manage`), `pdf_export_window` (`state.pdf_export`), `help_window`
(`state.show_help`), `filters_window` (dialog unica dei filtri), `milestone_manager_window`,
`move_dialog_window`, `popup_window` (single `Popup` enum for tripletta/start/end/
category/worker-max/etc.).

### Notifiche in-app (toast) — `src/ui/toasts.rs`

L'**unico** canale per riportare all'utente esiti ed errori; ha sostituito sia gli
`eprintln!` (invisibili) sia il vecchio `UiState.external_notice` in toolbar.

- Tipi in `mod.rs`: `ToastLevel { Info, Success, Warning, Error }` (con `icon()`,
  `color()`, `ttl()`) e `Toast { level, text, born: Instant }`; stato
  `UiState.toasts: Vec<Toast>`, cap `MAX_TOASTS` (5, escono le più vecchie).
- API: `state.toast_ok / toast_info / toast_warn / toast_error` (e `toast(level, …)`).
  Un testo **già presente** non crea un duplicato: rinfresca `born` (un `git push`
  che fallisce a ogni autosave non impila copie).
- Solo `Error` è **sticky** (`ttl() == None`, si chiude con ✕); gli altri livelli
  scadono da soli. `toasts_layer(ctx, state)` (ultima chiamata del blocco finestre in
  `ui()`) fa scadere, disegna in `Align2::RIGHT_BOTTOM` su `Order::Foreground` e
  chiama `request_repaint_after` sulla scadenza più vicina.
- Sorgenti collegate: `App::save` ora ritorna `Result<(), String>` e
  `PjmApp::save_to_disk` ritorna `bool` (**solo un salvataggio riuscito** azzera
  `changed`/`sync_baseline`; in uscita un errore **annulla la chiusura**);
  `save_export_dialog` (usata da `save_pdf_dialog`/`save_svg_dialog`/`save_md_dialog`)
  notifica successo col nome file o errore; gli export senza risultato usano
  `toast_warn`. Gli errori del **thread git** passano da
  `git_autosync::report` → `static LAST_ERROR: Mutex<Option<String>>` →
  `git_autosync::take_error()`, prelevato una volta per frame da
  `PjmApp::poll_background_errors`.
- Restano **modali** (non toast) gli errori bloccanti di caricamento:
  `UiState.load_error` + `load_error_window` (Apri…, Confronta…, validazione schema).
- Colori in `ui_style.rs`: `error_red()`, `warn_amber()`, `info_blue()`, `toast_bg()`
  (+ `ok_green()` già esistente), tutti theme-aware e passati per `g(...)`.

### Key UI mechanics

- **Ghost worker** (`Worker.ghost`, `#[serde(default, skip_serializing_if)]`): a
  worker flagged as an anomaly — its effective max is treated as 0. Toggle via
  **Filtri ▸ Ghost worker…** (all workers) or right-click the footer worker name
  (`Action::SetWorkerGhost`). Detection helpers on `SingleDev`:
  `has_worker_matching(pred)` / `weeks_with_worker(pred)` (effort>0 only). Effects
  when a ghost has effort in a dev: (1) the dev **name blinks** normal↔purple in
  `draw_left_devs` (time-based, requests repaint); (2) the grid cell and the
  footer week cell are **always purple** (highest priority, beats hidden/saturation).
  The GUI uses `GHOST_PURPLE` (in `ui_style.rs`, wrapped in `g(...)`) to distinguish
  ghosts from over-allocated workers (which stay red); **exports keep ghosts red**:
  (3) project PDF/SVG bars overlay **red** on `Row.ghost_weeks` + red dev-name label;
  (4) the trend PDF draws the incoming presunta segment + dot **red and thicker** for
  a ghost week.
- **Vai a oggi** (`Cmd/Ctrl+T`): ricalcola `columns_vec` con lo zoom/compact correnti,
  trova la colonna che contiene `current_week_id()` e imposta `UiState.pending_scroll_x`
  con `centered_scroll_x(cols, idx, cw, ui.available_width() - LEFT_W)` — lo stesso
  helper usato per lo scroll iniziale in `PjmApp::new`.
- **Cell editing**: left-click a grid cell to edit; typing triggers worker-name
  autocomplete; `Enter`/`Tab` commit, `Esc` cancels. Cell value is `"Worker|effort"`.
  `Cmd/Ctrl+C/X/V` copy/cut/paste (carrying the cell note for internal paste).
- **Bulk fill** (`UiState.bulk_fill: Option<BulkFill>` + `bulk_fill_window` in
  `dialogs.rs`): right-click an **empty** grid cell → dialog with the `show_in_find`
  workers (search + Select All), «Ore a settimana» and «Per quante settimane». OK →
  `Action::BulkFillEffort` → `bulk_fill_effort(app, proj, dev, start, workers, effort,
  weeks)` (free fn in `mod.rs`, unit-tested): writes the effort to every selected
  worker for N consecutive weeks starting at the clicked week (**inclusive**, going
  forward), **overwriting** any existing value for that worker (`add_effort` keeps the
  note), and stops at `app.end_week` — the dialog shows the effective week count. Only
  on non-merged columns (merged/zoom cells `continue` before the slot loop).
- **Notes** (yellow triangle indicator): right-click a non-empty cell → effort note;
  right-click a dev name → **Nota Dev…**; right-click a footer worker cell → **Note**.
- **Project progress %** — two figures, both planned-effort-weighted and both `None`
  (⇒ `—`) when there's no planned effort:
  - **actual/declared** (`Project::progress_pct` → `Projects::project_progress_pct`):
    `Σ(planned_i · declared_i) / Σ(planned_i)` (earned value).
  - **presumed** (`Project::presumed_progress_pct(today)` →
    `Projects::project_presumed_progress_pct`): budget consumed so far =
    `Σ(effort_up_to(today)_i) / Σ(planned_i)` — can exceed 100 (over-budget), so it's
    `Option<u32>`. `today` is `current_week_id()` in the grid, `WeekId(today)` in export.
  Shown together as **`presunta%/attuale%`**: an **«Avanz.: PP%/AA%»** row at the
  bottom of the project info header in `draw_project_info` (non-compact,
  non-worker-filter). Adds one info row, so `project_layout`'s `extra_rows` is **5** in
  normal mode. Both percentages **and** the value shown by the grid come from a single
  source, `Project::progress_breakdown(today) -> Option<(used, planned, weighted_declared)>`
  (`progress_pct`/`presumed_progress_pct` are thin wrappers over it); the grid's hover
  tooltip prints the **actual numbers** behind each figure (`usato/pianificato`,
  `dichiarate≈…/pianificato`).
- **Dev progress %** (under the dev name, non-compact only, in `draw_left_devs`): two
  small figures — a **presumed** % (read-only) = effort used up to today ÷ planned
  (`SingleDev::effort_up_to(current_week_id())` / `planned_effort()`; red when >100%,
  hidden when planned=0), and next to it an **editable declared** % typed by the dev
  (`Action::SetDevDeclaredPct` → `Projects::set_dev_declared_pct`, buffer in
  `UiState.declared_buffers`). The declared field's background is **red when declared <
  presumed, green when ≥**; when the **planned effort is 0 the declared field is hidden
  and not editable** (gated on `used_pct.is_some()`). Both are optionally drawn in
  PDF/SVG (see export toggle below). The declared value is stored as a **dated history**
  `SingleDev.declared_history: Vec<DeclaredPoint{week, pct}>`: `set_declared_pct(week,
  pct)` records it for the **current week** (`current_week_id()`) — one entry per week
  (same week overwrites), no-ops when unchanged (returns `bool`, so the handler only
  `mark_changed()`s on a real change); `declared_pct()` returns the latest (or 0).
  **Not backward compatible**: the old `declared_pct: u8` RON field was removed — old
  files still load (unknown field ignored) but their previous declared value is
  discarded (history starts empty). `declared_history()` exposes the series for the
  planned dev/project progress **trend PDF** (to be built next).
- **Milestone / move**: right-click the top strip of a dev's column → **Aggiungi
  milestone qui** and **Sposta** (blocco / devs). Several **different** milestones
  can share a week (`Project.milestones` is keyed by `MilestoneId`, so it's the
  *same* milestone that can't repeat within a project); the grid paints the week
  column as **N equal vertical bands**, one per milestone, ordered by id
  (`paint_milestone_bands` in `grid.rs`, `MS_BAND_MIN_W` = 4 px minimum band —
  beyond that only the first ones are drawn, the hover tooltip still lists all).
- **Milestone kind** (`MilestoneKind { Goal, Trigger }`, `#[serde(default)]` =
  `Goal`, so old RON files load as goals): a milestone is not only an arrival
  flag, it can be the **trigger of an event**. The kind belongs to the
  **milestone** (next to name/color), not to the placement, so it holds in every
  project. Picked at creation (**Aggiungi ▸ Milestone**, `UiState.new_milestone_kind`
  → `Action::CreateMilestone(name, kind)`) and changeable anytime in
  `milestone_manager_window` (`Action::SetMilestoneKind`). Two widgets, same items
  (`milestone_kind_items`): `milestone_kind_combo` (a `ComboBox`) **only inside
  windows**, and `milestone_kind_submenu` (a `SubMenuButton` «Tipo: … ⏵») **inside
  toolbar menus** — a `ComboBox` there breaks, because its dropdown lives in its
  own layer and the menu (`CloseOnClickOutside`) reads the click as "outside" and
  closes before the item is even selected (egui 0.34; guarded by the test
  `milestone_kind_submenu_keeps_the_add_menu_open`). On screen the kind only shows
  as an **icon before the name** in the grid's «Aggiungi milestone qui» menu
  (`MilestoneKind::icon()` — `⚑` / `⚡`, glyphs already covered by egui's bundled
  fonts, checked by `milestone_kind_icons_are_renderable`); the week column itself
  is unchanged. In **PDF/SVG** `pole_tip` draws the usual triangular pennant for
  `Goal` and a **mini lightning bolt** (6-point polygon) for `Trigger`, same color,
  same pole/label/arch layout.
- **Worker filter active** → `draw_project_info` shows **only the tripletta** (other
  info hidden so it adds no height); projects with no matching workers disappear.
- **Project view mode** (`UiState.project_view: ProjectViewMode` — `Open`/`Closed`/
  `All`, default `Open`, **not persisted**; menu Vista + `Cmd/Ctrl+1`/`2`/`3`) filters
  which projects the central body shows, on top of the worker filter. The predicate
  `project_in_body(app, view, proj)` is the single rule (used by `project_layout` and
  `body_projects`). **Gotcha**: closing a project forces `enable = false`
  (`Project::set_closed` / `reset_enable_from_closed`), so `enable` only distinguishes
  *open* projects hidden via «Filtri ▸ Progetti…». Hence: `Open` → `!closed && enable`,
  `Closed` → `closed` (enable ignored), `All` → `closed || enable`.
  `body_projects(app, view)` returns that set (ignoring the worker filter) and drives
  the PDF/SVG/minuta project lists so they match what's on screen. **Closed projects
  always render grayscale**: `grid`/`left_column` call
  `set_bw_mode(state.bw_mode || is_closed(proj))` per project and restore after.
- **Zoom (merge weeks)** → merged columns are **read-only**, sum effort, hide worker
  names (per dev: cumulative row + one summed cell); milestones/start/end stay
  visible; a group never crosses the year boundary. Only in non-compact view.
- **Year boundary**: `Col::YearEnd` canary column shows per-dev missing hours and a
  project total (`T:…`) for projects straddling the year.

### PDF / SVG export (`pdf_export.rs`)

Drawing is backend-agnostic: `page_shapes(...)` builds a `Vec<Shape>` (Rect/Poly/
Line/Text in mm, origin bottom-left), then `render_pdf(fonts, &shapes)` emits
printpdf `Op`s and `render_svg(&shapes)` emits an SVG string (Y-flipped, cropped to
content). `project_shapes(app, proj, name, dev_info, today, created, order, chart_only, fmt)`
gathers rows/flags for a project; `chart_only` drops the tripletta/description and
the footer date.

- **Progress % in export** (`set_show_pct(bool)` / `show_pct()` — a thread-local flag
  in `pdf_export.rs`, mirroring `ui_style`'s theme/B-W flags; default off): when on,
  each dev row appends `presunta%/dichiarata%` **after the date label (far right,
  same 7.0 date font)** — `Row.presumed_pct` / `Row.declared_pct`, computed in
  `project_shapes` from `today` (`-` when planned=0). **No-effort rows omit it** (they
  `continue` before the date label), so only devs with effort show percentages.
  The same flag also prints the **overall project progress under the "Today" marker**:
  **"Today" in bold** with `(presunta%/attuale%)` on the line below it, at the flag-date
  font (7.0); `progress_pct`+`presumed_pct` are passed into `page_shapes`. Shown only
  when today is within the chart axis.
  (In the on-screen grid the two under-name percentages instead use `cell_font`, the
  effort/residuo size — a separate choice from the PDF's date-font.)
  The export dialogs (`pdf_export_window`, `pdf_multi_export_window`) expose a
  **«Includi percentuali di avanzamento»** checkbox bound to `UiState.export_progress_pct`
  (remembered across exports, not persisted); the `Action::ExportPdf*/ExportSvgProject`
  handlers call `set_show_pct(self.ui.export_progress_pct)` before `build_*`.

- **Bar format** (`BarFormat`, passed through every `build_*`): how a dev's bar is
  drawn. `Continuous` (default, historical) = one rect first→last effort week;
  `Segmented` = one rect per run of consecutive effort weeks (gaps show), via
  `contiguous_runs`; `Proportional` = one rect per week, half-height ∝ week-sum /
  `proportional_ref(max_week)` — the dev's max weekly sum but never below
  `PROPORTIONAL_REF_MIN` (40h), so a 40h week is only full-height if the dev never
  works more elsewhere (busiest week reaches full `bar_hh`, same as the other two).
  `Row` carries `weeks: Vec<(day, hours)>` + `max_week` for the last two. The choice
  lives in `UiState.bar_format` (remembered across exports, **not** persisted) and is
  picked in the export dialogs via `bar_format_selector` (live `draw_bar_format_preview`
  thumbnails, no image assets).
- `build_pdf(app, fmt)` — one Gantt page per eligible project (enabled, not closed, has
  start AND end). Dev rows are only those **with** effort, sorted by start date (`order=None`).
- `build_pdf_selected(app, &[ProjectId], fmt)` — same as `build_pdf` but only the given
  projects (still enabled + has start AND end; **closed are allowed** — the caller's
  selection already reflects the view mode), in display order.
- `build_pdf_project(app, proj, ordered_devs, fmt)` — single project, user dev order.
  Devs **with** effort → colored bar; **without** → thin full-width line. Exports even
  with an empty dev list.
- `build_svg_project(app, proj, ordered_devs, fmt)` — same chart as the single-project PDF
  but chart-only (no tripletta/description/date) as an SVG string.
- **Trend PDF** (`build_trend_pdf(app, &[ProjectId])`, **File ▸ Andamento…** →
  `Action::ExportTrend` on `body_projects`): a **line chart of % over time**, **one page
  per project** (with a plottable dev). `trend_page_shapes` draws, for each dev with
  planned > 0 (color = GUI dev bg color): **presunta** as a solid polyline (weekly
  cumulative `effort_up_to(w)/planned`, **from that dev's own first effort week** — no
  0-anchor before it — to the last effort week, **can go past today**) and **dichiarata**
  as a dashed polyline (from `declared_history`, **held to the current week and no
  further**), plus a **black aggregate project pair** (planned-weighted; presunta starts
  at the **first effort in absolute**). X axis = months (only data weeks + today, snapped;
  project start **and** end excluded, so the chart starts at the first data point), Y axis
  = **auto 0..max** (rounded to a multiple of 20 with headroom, so over-budget >100% is
  visible). Includes title (tripletta+name), a solid/dashed style key, a dev-colour
  legend, a red **"oggi"** line, a **red horizontal 100% reference line** (the rest of
  the Y grid is grey, every 20%), and the same grey footer bar; **no milestone flags**.
  Every polyline **vertex is marked with a dot** (`dot`/`dots` helpers) so the exact
  data points that build each line are visible. Reusable drawing helpers:
  `dashed_seg`/`polyline`/`polyline_dashed`/`dot`/`dots`, `declared_at`.
- Triggering (in `Action::ExportPdf`): the visible set is `body_projects(app,
  view)` (enabled + current Vista mode). If exactly **one** project is visible
  the UI opens `pdf_export_window` (Select All, drag-to-
  reorder via egui `dnd_drag_source`/`dnd_release_payload`, per-dev checkbox); if
  **more than one**, it opens `pdf_multi_export_window` (Select All + per-project
  checkbox) → `Action::ExportPdfSelected` → `build_pdf_selected`; with **none**,
  nothing is exported.

### RON persistence & external changes

`App` (de)serializes to RON. New persisted fields must be `#[serde(default)]` for
backward compatibility. The app watches the file's mtime and, if it changes on disk
while open, either notifies + auto-applies or asks (Mantieni le mie / Ricarica);
see `sync_merge.rs`.

### Git auto-sync (`git_autosync.rs`)

Every save goes through `PjmApp::save_to_disk` (manual `Action::Save` + autosave) or,
on exit-save, an explicit blocking call. If the data file's folder is inside a git
work tree **and the `.ron` is already tracked** (`git ls-files` non-empty), the module
runs `git add -- <file>` → `git commit` (only the `.ron`; skips if nothing changed) →
`git push`. An **untracked** `.ron` is left alone — the program never `git add`s a file
the user hasn't chosen to track. It is **best-effort**: not a repo / untracked file /
no remote / offline never blocks or breaks the on-disk save. A **real failure**
(`add` non eseguibile, `push` fallito) goes through `report`, which logs to stderr
**and** parks the message in `static LAST_ERROR: Mutex<Option<String>>`; the UI drains
it with `take_error()` once per frame (`PjmApp::poll_background_errors`) and turns it
into an error toast — the thread can't touch `UiState` itself.
`commit_and_push` runs on a background thread (single-flight via an `AtomicBool`, so
autosaves can't pile up); `commit_and_push_blocking` runs inline and is used on exit
so the push finishes before the process ends. `GIT_TERMINAL_PROMPT=0` prevents git
from hanging on a credentials prompt. Only the `.ron` is staged — the `.bakN` files
are never committed.
