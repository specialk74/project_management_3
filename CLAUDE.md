# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> Code review / refactoring backlog: **`REVISIONE_CODICE.md`** (Italian). Items have
> stable codes (A1, C1, …) and are done **only after the user approves them**. Keep it
> up to date on **every** change that touches an item: tick it, add a note of what was
> done, refresh the "Stato attuale" numbers and append to "Registro delle modifiche".
>
> Feature / UX roadmap: **`MIGLIORAMENTI.md`** (numbered #1…). Code-quality items go in
> `REVISIONE_CODICE.md`, not there.

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
    `project_checklist`, save dialogs, `weeks_vec`…).
  - `tests.rs` — the UI tests (`#[cfg(test)] mod tests;` in `mod.rs`, a child module,
    so `use super::*` sees everything). `test_util.rs` (test-only, re-exported) holds
    the shared helpers: `screen(w, h)`, `click_events(pos, button)` (move/press/
    release — send them in **three separate frames**), `drawn_texts(out)`,
    `text_pos(out, label)`. Use them in new tests instead of re-writing the event
    arrays or the `Shape::Text` extraction.
  - `toolbar.rs` (`toolbar`/`header`/`add_field`), `grid.rs` (`body`/`grid`/`left_column`/
    `draw_dev_cells`/`project_layout`/`draw_project_info`/paint helpers…), `footer.rs`
    (`footer`/`draw_left_footer`/`draw_right_footer`…), `dialogs.rs` (all the modeless
    `*_window` fns + `move`/`popup` helpers), `export.rs` (PDF/SVG/minuta dialogs +
    `bar_format_selector`/`draw_bar_format_preview`), `help.rs` (manual window + parsing),
    `saturation.rs` (worker-saturation dashboard), `filters.rs` (dialog unica dei
    filtri Workers/Progetti/Dev), `toasts.rs` (notifiche in-app), `matrix.rs` (matrice
    progetti × worker: `matrix_select_window` + `matrix_window`).
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
├── corner_pct: i32                  — PDF/SVG corner rounding 0..100 (default 40, file-only)
├── workers   : Workers              — named people (max hours, colors, hidden-in-footer, show_in_find, ghost)
├── devs      : Devs                 — roles (e.g. "Frontend"); each has bg+font color + hide_in_totals
├── categories: Categories
├── milestones: Milestones           — name + color + kind + categorie di stampa (Internal/External), shared across projects
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
  block heights. It is computed **once per frame** in `body` via
  `body_layout(ui, app, state, filter, dev_filter)` and passed as `&[ProjLayout]` to
  `left_column`, `grid` and the jump-to-project (tests that call `grid` directly use
  `body_layout` too).
- **Viewport culling in the grid**: `grid` skips `draw_dev_cells` for dev blocks
  outside `ui.clip_rect()` (+ `CULL_MARGIN` = one column) and `draw_dev_cells` skips
  week columns outside it — but still adds their `week_total` to `running`, because
  the cumulative row sums every week from the left. The block/column holding the
  open `Editing` cell is **always** drawn (an editor that stops being drawn loses
  focus and would commit by itself). The left column is not culled (it has
  editable fields and is per-dev, not per-week). Tests
  `grid_skips_offscreen_columns_but_keeps_the_cumulative`,
  `grid_keeps_drawing_the_cell_being_edited_when_scrolled_away`.

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

### egui API conventions

The build **and `cargo clippy --all-targets`** have **zero warnings**: keep it that
way, and don't reintroduce deprecated egui 0.37 APIs. Functions with many parameters
use `#[allow(clippy::too_many_arguments)]` (project convention). The repo is **not**
`rustfmt`-formatted: don't run a global `cargo fmt`; to format only touched code use
`rustfmt +nightly --unstable-features --file-lines` with absolute paths, covering the
enclosing block. To
close a menu from one of its items use `ui.close_kind(egui::UiKind::Menu)` (what the
old `close_menu()` did; `ui.close()` closes the *nearest* closable, which can be just
a submenu); panels are `egui::Panel::{left,right,top,bottom}` with
`exact_size`/`default_size` and `.show(ui, …)` (`show_inside` was renamed); the
toolbar uses `egui::MenuBar::new().ui(…)`; a non-scrollable synced `ScrollArea` uses
`.scroll_source(ScrollSource::NONE)`; `ctx.content_rect()`, `Button::selectable`,
`ui.scope_builder`.

**egui 0.37 specifics** (upgraded from 0.34, item A8 of `REVISIONE_CODICE.md`):
- Ids: `egui::Id::unique(…)` (what `Id::new` was) for absolute ids
  (`ui.interact`, `Window::id`, `Area::new`); `ui.scope_id()` (was `ui.id()`) as
  parent for `.with(…)`. A generic "salt" parameter must be `impl Hash + Debug`.
- Scroll offsets: `out.state.clamped_offset()` (the `offset` field is private).
  **Rubber-band overscroll is turned off** at startup
  (`style.spacing.scroll.kinetic.rubber_band = false` in `PjmApp::new`): header,
  footer and left column follow the grid's offset and would drift during a bounce.
- Fonts: egui 0.37 no longer loads the symbol/emoji fonts by default; `Cargo.toml`
  enables `egui`'s `monochrome_emoji_fonts` (direct `egui` dependency only for that
  feature). «✕» (U+2715) is in **no** egui font — use «✖». The test
  `every_ui_symbol_is_renderable` scans the string literals of `src/ui/*.rs` and
  fails on any non-drawable character.
- **Drag-to-scroll everywhere**: the user scrolls every view by pressing a mouse
  button (usually the right one) on the content and dragging, horizontally and
  vertically. egui 0.37 made content dragging touch-screen-only by default
  (`DragScroll::OnTouch`), so every user-facing `ScrollArea` uses
  `.scroll_source(drag_scroll())` (`mod.rs`: bar + wheel + `DragScroll::Always`).
  Only the synced followers (header, footer, grid's left column, saturation's
  fixed column/header) don't. **Every new `ScrollArea` must use `drag_scroll()`.**
  The compare window pans with its own background drag (`compare.rs`). Test
  `right_drag_scrolls_the_grid_both_ways` (fails with egui's default).
- **GPU on Windows**: `main.rs` calls `prefer_low_power_gpu(&mut native_options,
  env_set)` only under `#[cfg(windows)]`: `wgpu_options.wgpu_setup` →
  `PowerPreference::LowPower` unless `WGPU_POWER_PREF` is set. On a hybrid laptop
  (NVIDIA RTX 2000 Ada + iGPU) the default `HighPerformance` made drag-scrolling
  stutter with egui 0.37 (cross-adapter presentation); the user confirmed
  `WGPU_POWER_PREF=low` fixes it. The function compiles and is tested on every OS
  (`low_power_gpu_unless_the_env_var_decides`); only the call is Windows-only, and
  there is no Windows target installed here, so that one line is not compiled locally.
- `TextEdit::align(Align2)` replaces `horizontal_align`.
- Tests run frames with `ctx.run_ui_test(input, |ui| …)` (`test_util::RunUiTest`):
  it clears `textures_delta`, otherwise a dropped `FullOutput` panics in debug
  («Dropped TexturesDelta with N unapplied deltas»).

### Toolbar menus (in `toolbar`)

- **File**: Salva (`Cmd/Ctrl+S`), Apri…, Esporta… (PDF Gantt), Andamento… (PDF trend % nel tempo), Minuta… (esporta note progetti in Markdown), Report… (PDF riepilogo progetti), Esci.
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
- **Aggiungi**: + Progetto, **Clona progetto…**, and `add_field` inputs for Worker / Dev / Categoria / Milestone.
  - **Clona progetto…** → `UiState.clone_project: Option<CloneProject>` →
    `clone_project_window` (`dialogs.rs`: all projects incl. closed, listed and
    searched by tripletta only — `project_label`, name as fallback —, no fields)
    → `Action::CloneProject { src }` → free fn `clone_project(app, src)` (`mod.rs`;
    same name, tripletta = `clone_tripletta(orig)` = «ABC Copia», or «Copia»):
    `Projects::clone_project` deep-clones the `Project` (everything), forces
    `Active` + enabled and re-indexes the order to put it **right after** the
    source; then each placement of a milestone **owned by the source** is swapped
    for a new owned milestone of the copy (same name/color/kind/categories/week),
    globals stay shared — so `App::validate` stays clean. The handler jumps to the
    copy and toasts. Test
    `clone_project_copies_everything_and_duplicates_own_milestones`.
- **Filtri**: le prime sei voci (Progetti… `Cmd/Ctrl+P`, Workers… `Cmd/Ctrl+F`,
  Workers (settimana corrente)… `Cmd/Ctrl+G`, Dev… `Cmd/Ctrl+D`, Categorie…
  `Cmd/Ctrl+K`, Anni… `Cmd/Ctrl+Y`) aprono **un'unica dialog** `filters_window`
  (`src/ui/filters.rs`, `UiState.show_filters`) a **cinque colonne** — Workers |
  Progetti | Dev | Categorie | Anni — ognuna con campo di ricerca, Select All ed
  elenco con checkbox. La scorciatoia/voce di menù decide solo quale colonna riceve il
  focus (`UiState.filters_focus: Option<FilterPane>` + `filters_focus_dirty` one-shot;
  helper `open_filters`/`focus_pane`/`toggle_filters`). Ripremuta a dialog aperta, la
  stessa scorciatoia fa il **toggle di Select All** della sua colonna
  (`*_filter_toggle_all`), applicato agli elementi **attualmente elencati** (rispetta la
  ricerca); `Shift+…` deseleziona tutto senza aprire. **`Cmd/Ctrl+J`** →
  `reset_all_filters(app, state, actions)`: rimette `worker_filter`/`dev_filter`/
  `category_filter`/`year_filter` a `None` (il criterio anno resta), spegne `worker_filter_current_week`, svuota le ricerche e riabilita tutti
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
  - **Categorie** — `UiState.category_filter: CategoryFilter` =
    `Option<HashSet<Option<CategoryId>>>` (la voce `None` del set è «Senza
    categoria», prima in elenco). Predicato `category_shown(app, proj, &f)`,
    raccolto in `ProjectFilter` (vedi Anni), quindi vale anche per gli elenchi di
    export. Agisce sul **progetto intero**: non
    entra in `filter_active` (header completo) e non nasconde progetti senza dev.
    Indipendente dal selettore categoria del footer (`selected_category`, totali-anno).
    Non persistito.
  - **Anni** — `UiState.year_filter: YearFilter` = `Option<HashSet<i32>>` (anni da
    `available_years`: date di inizio/fine dei progetti) + `UiState.year_criterion:
    YearCriterion { Starts (default), Ends, Within }` scelto con radio nella colonna.
    `year_shown(app, proj, &f, crit)`: passa se il criterio vale per **almeno un**
    anno del set; `Within` = inizio e fine nello **stesso** anno; manca la data
    richiesta ⇒ nascosto. Tutti gli anni spuntati ⇒ `None` (filtro spento, qualunque
    criterio). Categoria + anno formano **`ProjectFilter { category, years,
    criterion }`** (`ProjectFilter::from_state(state)`, `.shows(app, proj)`), l'unica
    regola a livello progetto passata a `project_layout` **e** a
    `body_projects(app, view, &pf)`: agisce sul progetto intero (header completo,
    fuori da `filter_active`). Non persistiti.

  Restano finestre a sé: Milestone… (manager),
  Worker (visibilità e ghost)… (`worker_visibility_window`, titolo «Worker»,
  `UiState.show_worker_visibility` + `worker_visibility_search`; ha assorbito la
  vecchia «Ghost worker…»: **tutti** i worker, indipendente da
  hide_in_footer/show_in_find/filtro — unico punto sempre raggiungibile per il
  ghost —, ricerca, tre colonne di spunte; Select All in intestazione sui worker
  elencati solo per Footer ed Elenchi; **Ghost** → `Action::SetWorkerGhost`; **Footer** →
  `Action::SetWorkerHiddenInFooter` → `Workers::set_hidden_in_footer` (scrive
  `hide_in_footer: None` quando visibile) ed **Elenchi** →
  `Action::SetWorkerShownInFind` → `Workers::set_shown_in_find`; entrambi
  ritornano `bool` e marcano il file solo se cambiano),
  Dev nei totali… (`dev_totals_window`, `UiState.show_dev_totals`: **tutti** i dev con
  spunta + Select All → `Action::SetDevHiddenInTotals` → `Devs::set_hidden_in_totals`;
  `Dev.hide_in_totals`, `#[serde(default, skip_serializing_if = "is_false")]`,
  persistito: il dev sparisce **solo** dal riquadro in basso a sinistra del footer —
  riga e somma «Totale», via `Devs::list_in_totals` usato sia per l'altezza in
  `footer` sia in `draw_left_footer` — e resta ovunque altrove; anche tasto destro
  sul nome dev nel footer ▸ «Nascondi dai totali»; unico punto per rimetterlo è la
  finestra),
  Stato progetti… (`closed_filter_window`, voce rinominata da «Closed…»): una combo
  per progetto **Attivo / On-Hold / Closed** → `Action::SetProjectStatus` →
  `Projects::set_status`. **On-hold** = `ProjectStatus::OnHold` in
  `project.rs`: salvato come `closed: Some(true)` **più** `on_hold: true`
  (`#[serde(default, skip_serializing_if = "is_false")]`), quindi `is_closed()` è
  vero e si comporta in tutto da chiuso (le versioni vecchie lo leggono chiuso);
  `set_closed` azzera sempre `on_hold`, `status()`/`set_status()` lo gestiscono.
  Cambia solo l'etichetta nella matrice («on-hold» invece di «chiuso»/«closed»).
  **Gotcha**: la finestra si chiude col click fuori, ma il menu di una combo è un
  livello a sé e la scelta di una voce chiude il menu **nello stesso frame**: il
  controllo usa `Popup::is_any_open` **o** il flag del frame precedente
  `closed_filter_popup_was_open` (test
  `status_combo_sets_on_hold_and_keeps_the_window_open`, che con il solo controllo
  sul frame corrente fallisce).
  - **Report** (`report_window` in `export.rs`, stato `ReportState { entries }`
    preselezionato con `body_projects`, → `Action::GenerateReport { projects }` →
    `pdf_export::build_report_pdf`): PDF di riepilogo, **una pagina (o più) per
    progetto** via `report_pages` — tripletta, categoria, info, inizio/fine
    (`mm/dd/yyyy`), avanzamento presunto/effettivo, poi la tabella di **tutti** i dev
    (`list_devs`) da `report_devs` → `ReportDev { planned, used, allocated, missing }`:
    usato = `effort_up_to(oggi)`, allocato = tutte le ore in griglia, mancante =
    stimato − usato (negativo = sforamento, in **rosso**); le % sono sullo stimato
    del dev (`ReportDev::pct`, `None` ⇒ «—» con stimato 0). La tabella continua su
    pagine successive (intestazione ripetuta) se i dev non entrano.
    **Fine anno**: se `report_year_end(app, proj, today)` è `Some((anno, 31/12))`
    (inizio ≤ anno di oggi < anno di fine, entrambe le date presenti) la tabella ha
    **due gruppi in più** e la colonna nome si stringe (`REPORT_NAME_W_YEAR`):
    `ReportDev.year_split = (fino_31_12, dal_1_1)` con `fino_31_12 =
    effort_up_to(31/12) − usato` (settimane keyed per lunedì: quella che parte
    entro il 31/12 conta nell'anno) e `dal_1_1 = mancante − fino_31_12` (negativo
    ⇒ rosso). Titoli dei gruppi rimpiccioliti se non entrano.
- **Vista**: Vista compatta, Bianco/Nero, **Progetti** (Solo aperti `Cmd/Ctrl+1` / Solo chiusi `Cmd/Ctrl+2` / Tutti `Cmd/Ctrl+3` — `UiState.project_view: ProjectViewMode`, non persistito), **Tema** (Auto/Chiaro/Scuro), **Zoom settimane** (Normale/2/4), Saturazione worker… (dashboard read-only: `saturation_window`; mostra i worker con `show_in_find` true **o** non nascosti nel footer, ignorando il filtro Ctrl+F).
  Finestra ridimensionabile in entrambe le direzioni (gli scroll riempiono
  `available_height`). Colonne = `SatCol { year, label, full, weeks }`
  (`saturation_columns`: label `mm-dd` / `mm` senza anno, `full` col year per il
  tooltip). L'intestazione (riga anno da `year_spans` + riga date) è disegnata da
  `draw_saturation_header` in uno `ScrollArea::horizontal` a parte, **fuori** dallo
  scroll verticale, che segue `UiState.sat_scroll_x` del corpo; la colonna fissa
  segue `sat_scroll_y`. Allineamento a passo fisso `SAT_CW + SAT_GAP` (griglia con
  `min_col_width(SAT_CW)`), `SAT_HEAD_PAD` compensa la barra verticale del corpo.
  Test `header_stays_visible_and_aligned_when_scrolled_down`.
  The menu item re-ticks `saturation_future_only` on every opening.
  Hover: the heat cell under the pointer writes `(worker, col index)` into a local
  `hover_next`, published to `UiState.sat_hover` at the end (repaint on change);
  header and fixed column, drawn first, read `sat_hover` and paint date / name with
  `hi_bg()`/`hi_text()`/`HI_BOLD_DX`. Test `hovering_a_cell_highlights_its_worker_and_week`.
  **Matrice progetti/worker…**: prima `matrix_select_window` (worker `show_in_find`,
  ricerca + Select All → `UiState.matrix_workers`, **solo sessione**), poi
  `matrix_window` (`UiState.matrix_from`/`matrix_to: Option<i32>` in giorni, `None` =
  oggi; `DatePickerButton`). In mezzo alla barra la combo anni (`years_combo`,
  anni da `available_years`, stato `UiState.matrix_years: BTreeSet<i32>`, solo
  sessione) imposta Da/A con `years_range` (1/1 del più vecchio → 31/12 del più
  recente); un cambio manuale di Da/A (`range_end_picker` → `bool`) la svuota.
  Accanto, `categories_combo` → `UiState.matrix_categories:
  BTreeSet<Option<CategoryId>>` (vuoto = tutte, `None` = «Senza categoria»,
  solo sessione), applicato da `worker_matrix::build_filtered` (righe, riepilogo e
  totali; `build` = senza filtro) e stampato nel sottotitolo del PDF. Idem
  `statuses_combo` → `UiState.matrix_statuses: BTreeSet<ProjectStatus>` (vuoto o
  tutti e tre = nessun filtro, solo sessione), ultimo argomento di
  `build_filtered`: filtra righe/riepilogo/totali e riempie `MatrixData.statuses`
  (indici in `ProjectStatus::ALL`), l'elenco delle sotto-righe per stato da
  disegnare in GUI e PDF; nel PDF «Status: …» nel sottotitolo se non sono tutti
  (`MATRIX_STATUS_EN`). Test `status_filter_limits_rows_summary_and_sub_rows`,
  `matrix_pdf_follows_the_status_filter`.
  I dati vengono da `src/worker_matrix.rs`
  (`build(app, &workers, from, to) -> MatrixData`, condiviso con l'export):
  colonne = worker, righe = **tutti** i progetti (aperti e chiusi, ignora Vista e
  filtri) in cui un worker è **assegnato, anche a effort 0**, in una settimana che
  si **sovrappone** a `[from, to]` (`week_overlaps`: lunedì..domenica); cella =
  nomi dei dev; righe raggruppate per categoria (per id, «Senza categoria» in
  fondo) colorate con `category_color` (palette fissa per id, le categorie non
  hanno un colore nel file); i progetti chiusi (`MatrixRow.closed`) restano nel
  gruppo, riga con la tinta della categoria spenta verso il grigio
  (`closed_colors`) + strisce (`hatch_segments`), condivisi GUI/PDF, e
  «chiuso»/«closed»; `summary[cat][worker]` = progetti per categoria (righe subito
  sotto la matrice, senza intestazione propria),
  `totals[worker]` = progetti totali (riga «Totale progetti» / «Total projects»),
  seguiti da `status_totals[worker]: [attivi, on-hold, closed]` (ordine
  `ProjectStatus::ALL`); ogni categoria ha la stessa suddivisione in
  `summary_status[cat][worker]`. Sotto-righe «Attivi/On-Hold/Closed» (GUI,
  `STATUS_LABELS`, `MX_SUB_H`) / «Active/On-Hold/Closed» (PDF, `MATRIX_SUB_H`)
  dopo ogni categoria e dopo il totale. Nel PDF il riepilogo è un elenco di
  `SumRow` disegnate una per una con salto pagina (`new_page`, intestazione worker
  ripetuta) — test `matrix_summary_breaks_across_pages`.
  GUI: `draw_matrix` ridisegna la riga dei worker **in fondo**, a
  `clip_rect().top().max(rect.top())`, così resta fissa in cima scorrendo
  (test `worker_header_stays_visible_when_scrolled`). Hover: `mx_hit(pos, …,
  rows)` → `MxHit { row, col }` (righe progetto + riepilogo raccolte in `rows`,
  le intestazioni di categoria escluse); velo `mx_veil()` (nero nel tema chiaro,
  bianco nel tema scuro) su tutta la colonna (non la riga intera); etichetta
  della riga e contenuto della cella puntata (dev, o valore nel riepilogo) **gialli
  in grassetto su fondo scuro** (`mx_hi_text()`/`mx_hi_bg()`; grassetto simulato
  con una seconda passata a `MX_BOLD_DX`, l'app ha un solo peso di font;
  uguali nei due temi: le righe
  progetto sono pastello anche col tema scuro). Tutto va **sotto/al posto dei
  testi** senza conoscere l'hover in anticipo: per ogni fascia (`Band { Group,
  Project, Summary }`) si riserva uno slot `p.add(Shape::Noop)` dopo lo sfondo,
  e ogni testo evidenziabile è a sua volta uno slot (`HiText`, via `put`);
  a fine disegno, calcolato `mx_hit`, si riempiono con `p.set` (test
  `hover_highlight_is_yellow_on_dark_in_dark_theme`).
  Nome del worker puntato a colori invertiti (fondo `text()`, nome `bg()`).
  **Worker header split by dev hours** (GUI and PDF): `MatrixData.dev_hours[w]` =
  hours per dev (DevId order, >0 only) summed over the matrix's rows (category/
  status filters apply) in the weeks overlapping `[from, to]`;
  `worker_matrix::dev_bands(&hours) -> Vec<(DevId, from, to)>` gives the width
  fractions, shared by `draw_matrix`'s `header` and `matrix_pages`' `header`, so
  screen and PDF match. No hours (only effort-0 assignments) → grey
  (`mx_no_hours_bg` / `MATRIX_NO_HOURS_BG`). The name uses `contrast_text` /
  `text_on` against the band under its centre. Tests
  `dev_hours_sum_projects_in_the_period_and_filters`,
  `worker_header_is_split_by_dev_hours`, `matrix_pdf_worker_header_is_split_by_dev_hours`.
  **Worker separators**: the vertical border between label column/workers and
  between workers is thick (`MX_SEP_W` 2.5 px, `mx_sep_color()` = `text()`; PDF
  `MATRIX_SEP_W` 0.6 mm black), the left edge stays thin; in the GUI the header's
  `vlines` are drawn **after** the inverted hovered name, which would cover half of
  it. **Hours by dev** (bottom of the summary, GUI «Ore per dev» / PDF «Hours by
  dev»): a header row with `worker_hours(&dev_hours[w])` then one row per dev in
  `MatrixData.devs` (= **all** `app.devs.list()`, even with no hours), label in the
  dev's colors, cell = `dev_share_label(h, pct)` (`30h · 75%`) from
  `dev_share(&dev_hours[w], dev)` (`None` ⇒ empty; pct rounded). In the PDF these
  are the `SumRow::DevHead`/`Dev(i)` rows, so they page-break like the rest. Tests
  `dev_share_is_hours_and_rounded_percent_of_the_worker_total`,
  `dev_rows_show_hours_and_percent_and_workers_are_split_by_thick_lines`,
  `matrix_pdf_has_dev_hour_rows_and_thick_worker_borders`. `worker_matrix::build`
  esiste solo nei test (`#[cfg(test)]`); il codice usa `build_filtered`.
  **Esporta PDF…** → `Action::ExportWorkerMatrix { from, to, years }` (anni della
  combo, stampati nel sottotitolo da `matrix_subtitle`) →
  `pdf_export::build_matrix_pdf` (A4 orizzontale, inglese; colonne in blocchi da
  `MATRIX_COL_MIN`, in ogni blocco solo le righe dei suoi worker, righe paginate con
  intestazione ripetuta, riepilogo in coda al blocco).
  **Torte dev/categorie per worker…** (`src/ui/pies.rs`, data/geometry in
  `src/worker_pies.rs`): same flow as the matrix, with its **own** session state
  (`pies_select_open`/`show_pies`/`pies_workers`/`pies_worker_search`/`pies_from`/
  `pies_to`/`pies_years`/`pies_categories`). The worker dialog is the shared
  `worker_pick_window(…) -> WorkerPick { Open, Show, Closed }` (`matrix.rs`, also
  behind `matrix_select_window`); `years_combo` and `categories_combo` take an
  id salt, and
  `range_end_picker`/`years_range` are shared too. `worker_pies::build(app,
  &workers, from, to, &cats) -> Vec<WorkerPie>` (`cats` empty = all, else only
  projects of those categories — totals and % follow): **hours** (effort 0 ignored) of all
  projects in weeks overlapping `[from, to]` (`week_overlaps`), per dev → per
  category (by id, «Senza» last) plus per category; `total` 0 ⇒ «Nessuna ora nel
  periodo.» and skipped by exports. Rings as `Seg { key, hours, from, to }`
  (fractions of the worker total): `dev_ring`, `dev_category_ring` (each dev's
  categories inside its slice), `category_ring`. Geometry shared GUI/PDF with a
  `y_up` flag: angle 0 at 12 o'clock, clockwise (`arc_point`, `arc`, `sector`),
  `label_pos` = label centre only if the text fits (chord × ring thickness),
  `hit` for hover tooltips. GUI slices are **`egui::Mesh`** triangle strips
  (`paint_sector`) — a ring slice isn't convex, so no filled polygon; PDF uses
  `Shape::Poly`. Percentages via `pct_label` (`<1%` for hours that round to 0).
  **Grid gotcha**: the tables' numeric cells (`num`) are a fixed `NUM_W` rect
  with the text painted `RIGHT_CENTER` — a `right_to_left` layout inside an
  `egui::Grid` cell doesn't right-align, and in the **last** column (no width
  bound) it stretched to the window edge, pushing «% del dev» out of the table
  (test `table_values_stay_under_their_headers`).
  Export: `Action::ExportWorkerPies { from, to, years, svg }` →
  **Done / planned**: every value is a `worker_pies::Hours { total, done }`
  (`planned()` = rest; `WorkerPie` keeps `total`/`done` fields); `build` takes
  `now` = current Monday (`current_monday_days()`), weeks starting `<= now` are
  done. Pies use `total` only; the tables get Fatte/Pianificate/Totale (PDF
  Done/Planned/Total, `pie_hour_cells`/`pie_hour_heads`, GUI `hour_cells`/
  `hour_heads`) only when `split_shown(to, now)` (period past the current week).
  GUI layout = PDF layout: each block is title, pie, table **below**
  (`draw_worker_pies`, test `tables_are_drawn_below_the_pies`), built by
  `pie_block`: block width = max(`PIE_D`, table width), pie centred in it (the
  pie fns take that `width`), a narrower table padded to the centre. The table
  is drawn after the pie, so its width comes from the **previous frame** (egui
  temp data, the table fns return `Grid` width; `request_repaint` on change —
  test `pie_is_centered_over_its_table`); the two blocks
  sit side by side (`PIE_SPACING` 72 px apart), stacked when narrower than
  `side_by_side_min_w(split)`; the «Categoria» cells are `category_cell` —
  swatch + name in an exact-size rect capped at `CAT_W` (110 px, `fit_text`
  ellipsis, full name in the tooltip; test
  `category_column_is_capped_and_tables_are_spaced`); the PDF tables go through `pie_table` (columns
  `PIE_NUM_W` apart from the right edge). Test
  `planned_hours_split_shows_done_and_planned_columns`.
  `pdf_export::build_pies_pdf` (one A4 page per worker, `pies_page`: two
  panels, each with a big pie centred under its title and the table **below**,
  scaled down by `pie_table_scale` when too long; subtitle from `pies_subtitle`
  with years + categories) or
  `build_pies_svgs` (one SVG per worker, no footer) → `save_multi_export_dialog`
  with the worker name made safe by `file_safe` as suffix. Tests
  `pies_sum_hours_by_dev_and_category`, `pie_geometry_labels_and_hit`,
  `worker_pies_draw_slices_labels_and_tables`,
  `pies_pdf_has_a_page_per_worker_with_hours`.
- **Aiuto**: Manuale d'uso… (opens `help_window`).

### Dialogs / windows

**Project lists show at most 15 rows** (`MAX_VISIBLE_PROJECTS`), then scroll: every
project list's `ScrollArea` uses `project_list_max_height(ui, key)` = 15 × the row
pitch measured on the previous frame by `remember_project_rows(ui, key, top, rows)`
(the pitch depends on the row widget — checkbox, combo — and the font). Applied to
`project_checklist` (Esporta multi, Minuta, Report), Filtri ▸ Progetti, Stato
progetti and Confronta. `project_checklist` also leaves room for the controls
**below** it, measured by `checklist_dialog_end(ui)` (call it last in the dialog's
content, same `ui`), so on a small screen «Esporta PDF…» stays on screen (tests
`project_checklist_shows_at_most_15_projects`,
`multi_export_button_stays_on_a_small_screen`). New project lists should follow it.
The same mechanism, via `rows_max_height(ui, key, rows)`, caps the dev and milestone
lists of the single-project `pdf_export_window` at `MAX_VISIBLE_EXPORT_ROWS` (10)
each (still also capped at 30% of the screen; test
`pdf_export_window_lists_show_at_most_10_rows`).

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
- Solo `Error` è **sticky** (`ttl() == None`, si chiude con ✖); gli altri livelli
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
  **Filtri ▸ Worker (visibilità e ghost)…** (all workers, «Ghost» column) or right-click the footer worker name
  (`Action::SetWorkerGhost`). Detection helpers on `SingleDev`:
  `has_worker_matching(pred)` / `weeks_with_worker(pred)` (effort>0 only). Effects
  when a ghost has effort in a dev: (1) the dev **name blinks** normal↔purple in
  `draw_left_devs` (time-based, requests repaint); (2) the grid cell and the
  footer week cell are **always purple** (highest priority, beats hidden/saturation).
  The GUI uses `GHOST_PURPLE` (in `ui_style.rs`, wrapped in `g(...)`) to distinguish
  ghosts from over-allocated workers (which stay red); **exports keep ghosts red**:
  (3) project PDF/SVG bars overlay **red** on `Row.ghost_weeks` + red dev-name
  label — gated by the export checkbox, see **Ghost in export** below;
  (4) the trend PDF draws the incoming presunta segment + dot **red and thicker** for
  a ghost week.
- **Date picker conversions** live only in `date_utils::picker` (`days_to_jiff`,
  `jiff_to_days`, `date_str_to_jiff`, `jiff_to_date_str`, `today_jiff`):
  `egui_extras::DatePickerButton` uses `jiff`, everything else uses `chrono`. The
  `yy-mm-dd`/`yyyy-mm-dd` parsing is `dates::parse_ymd` (exact day), shared with
  `parse_date_str` (Monday of the week).
- **«Oggi» has a single source**: `date_utils::dates::today()` (local date — never
  `Utc::now`, which shows the previous week on Monday after midnight in Italy),
  `today_days()` and `current_monday_days()`. `current_week_id()` wraps the latter,
  and `UiState.this_week` is refreshed at the top of every `ui()` frame, so grid,
  footer and saturation agree even if the app stays open across a Monday. Exports
  use the same functions. Only wall-clock timestamps (git commit, file name, toast
  time) still call `chrono::Local::now()` directly.
- **Vai a oggi** (`Cmd/Ctrl+T`): ricalcola `columns_vec` con lo zoom/compact correnti,
  trova la colonna che contiene `current_week_id()` e imposta `UiState.pending_scroll_x`
  con `centered_scroll_x(cols, idx, cw, ui.available_width() - LEFT_W)` — lo stesso
  helper usato per lo scroll iniziale in `PjmApp::new`.
- **Date/worker highlight** (`GridHighlight { week, worker }` in `mod.rs`): the
  header and the footer are drawn **before** the grid, so the grid writes
  `UiState.grid_hl_next` (reset at the top of `ui()`) and they read
  `grid_hl` (previous frame); at the end of `ui()` a change is published and
  `request_repaint()`ed. `grid()` sets `week` = first week of the `Col::Weeks`
  under the pointer (`rect_contains_pointer`, YearEnd ignored);
  `draw_cell_static` adds the hovered cell's `worker`; the footer does the
  same (`draw_right_footer`: column under the pointer → `week`, worker row →
  `worker`; `draw_left_footer`: hovered worker name → `worker` only); an open
  `Editing` overrides all of them (week of the cell, worker parsed from `buf`). Painted with the
  shared `hi_bg()`/`hi_text()`/`HI_BOLD_DX`/`paint_highlight` (the matrix's
  `mx_hi_*` now delegate to them): header date (compact: background only),
  footer date row, footer worker name and the worker × week cell. Tests
  `grid_hover_records_column_and_worker_to_highlight`,
  `header_and_footer_paint_the_highlighted_date_and_worker`,
  `footer_hover_records_column_and_worker_to_highlight`.
- **Footer worker name** (`draw_left_footer`): **left-click** toggles
  `UiState.footer_selected_worker` (session only) via `toggle_selected_worker`
  (same worker ⇒ off, another ⇒ switches); `draw_cell_static` paints that worker's
  `Worker|ore` rows with `hi_bg()` + `hi_text()` (bold = second pass at `HI_BOLD_DX`),
  and the footer name stays highlighted. **Right-click** menu: «Ore max…» (opens
  `Popup::WorkerMax`, formerly on left-click) and «Ghost». Tests
  `footer_click_toggles_and_switches_the_selected_worker`,
  `selected_worker_is_highlighted_in_the_dev_cells`.
- **Footer date row** (`draw_right_footer`): left-click on a non-merged date opens
  the same `Popup::BulkWeekMax` as the header date (→ `Action::SetBulkWeekLimit`,
  all workers). Test `footer_date_click_opens_the_bulk_week_limit`.
  The popup (built by `bulk_week_popup(app, week, date)`, shared by header and
  footer) has the checkbox «Metti tutti i worker in ferie» (`Popup::BulkWeekMax {
  holiday, holiday_initial }`, initial = `App::all_on_holiday(week)`), drawn via
  `hours_popup_window_with(…, extra)`. Every confirm button sends
  `SetBulkWeekLimit { holiday: Option<bool> }` → `App::set_bulk_week`: `Some(true)`
  = limit (the typed hours, not forced to 0) then Ferie to everyone not in Malattia; `Some(false)` (unticked
  after starting ticked) = drop Ferie and restore the global max, then limit;
  `None` = limit only. Tests `bulk_week_holiday_keeps_the_typed_hours_and_spares_sick_workers`,
  `bulk_week_popup_has_the_holiday_checkbox_and_sends_the_choice`.
- **Grid cell data**: `gather_slots` returns `(text "Worker|ore", note,
  Option<WorkerId>)`; `draw_cell_static` receives the id, so never look a worker up
  by name again in the cell path. Parse a typed `"Worker|ore"` only with
  `parse_cell` / `cell_worker` (`grid.rs`). Search fields filter with
  `search_matches(q, text)` (`filters.rs`); confirmation dialogs go through
  `confirm_window(ctx, title, ok_label, body) -> Option<bool>` (`dialogs.rs`);
  week labels come from `date_utils::dates::week_label(days)`; the project's
  display name is `Project::label(id)` (via `ui::project_label`).
- **Cell editing**: left-click a grid cell to edit; typing triggers worker-name
  autocomplete (`find_completion`: **only workers with `show_in_find`** — the
  «Elenchi» tick —; a hidden worker typed in full is still accepted on commit;
  test `cell_completion_skips_workers_hidden_from_lists`); `Esc` cancels.
  **Keyboard navigation** (only while a cell is open): arrows, `Tab`/`Shift+Tab`
  (→/←) and `Enter`/`Shift+Enter` (↓/↑) commit and open the neighbour cell.
  `nav_target(app, &state.nav_order, from, NavDir)` (`grid.rs`, pure, unit-tested)
  skips non-editable weeks (`week_editable`) and, vertically, devs of projects
  where that week isn't editable; up/down cross devs and projects in grid order
  (`UiState.nav_order`, refreshed by `grid` while editing); `None` at the edge →
  commit only. The editor pushes `CommitCell` **then** `Action::OpenCellEditor`,
  whose handler builds `Editing::open(…)` from the already-updated data. Gotchas:
  egui moves focus on Tab/arrows, so the editor sets `set_focus_lock_filter`
  **and** also treats a nav key that arrives in the same frame the focus is lost
  as navigation (the filter only applies from the second focused frame); on its
  first frame the editor calls `scroll_into_view`, and `body` consumes
  `pending_scroll_x/y` with `take()` **before** drawing the grid so that a request
  made while drawing survives to the next frame. Cell value is `"Worker|effort"`.
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
- **Cancella gli effort** (dev-name context menu, below «Elimina riga»): «Cancella
  gli effort…» (that dev) and «Cancella gli effort di tutti i dev…» (whole project)
  set `UiState.confirm_clear_efforts: Option<ClearEfforts { proj, dev: Option<DevId> }>`
  → `clear_efforts_window` (`dialogs.rs`, «Sì, cancella» / «Annulla») →
  `Action::ClearEfforts` → `Projects::clear_efforts(proj, dev)` → `SingleDev::clear_efforts`:
  empties `weeks` (cells incl. effort 0 and cell notes); planned effort, dev note and
  declared history stay. Returns `bool`, so the handler marks changed only on a real
  change; the items are disabled when there are no cells (`SingleDev::has_cells`).
  Tests `clear_efforts_empties_one_dev_or_all_devs_keeping_planned`,
  `clear_efforts_window_asks_then_confirms`.
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
  milestone qui** and **Sposta** (blocco / devs). Below them, **Sposta/Imposta inizio|fine
  progetto qua** (`bound_menu_items(app, proj, week)` in `mod.rs`: inside the period
  both, before the start only the start, after the end only the end; «Imposta» when
  the date is missing; the no-op one disabled). `bound_move_loss` lists the cells
  (any dev, effort 0 included) and milestone placements that would fall outside the
  new period: empty ⇒ `Action::MoveProjectBound` at once, otherwise
  `UiState.bound_move: Option<BoundMove>` → `bound_move_window` (`dialogs.rs`,
  «Cancella e sposta» / «Annulla»). The handler's `move_project_bound` deletes only
  those and sets the date. **Outside** the period the context menu holds only that
  item (`bound_menu`, no milestone/Sposta submenus). No menu on merged (zoom) columns. Several **different** milestones
  can share a week (`Project.milestones` is keyed by `MilestoneId`, so it's the
  *same* milestone that can't repeat within a project); the grid paints the week
  column as **N equal vertical bands**, one per milestone, ordered by id
  (`paint_milestone_bands` in `grid.rs`, `MS_BAND_MIN_W` = 4 px minimum band —
  beyond that only the first ones are drawn, the hover tooltip still lists all).
- **Cancella tutte le milestone…** (bottom of the «Aggiungi milestone qui» submenu,
  disabled when nothing is placed) → `UiState.confirm_clear_milestones:
  Option<ProjectId>` → `clear_milestones_window` (`dialogs.rs`, lists the placements)
  → `Action::ClearProjectMilestones` → `Projects::clear_project_milestones` →
  `Project::clear_milestones`: removes **only the placements** of that project
  (globals and own); milestone definitions — owned ones included — stay. Returns
  `bool` (mark changed only on a real change). Tests
  `clear_project_milestones_removes_placements_but_keeps_definitions`,
  `clear_milestones_window_asks_then_confirms`.
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
  closes before the item is even selected (egui 0.34–0.37; guarded by the test
  `milestone_kind_submenu_keeps_the_add_menu_open`). On screen the kind only shows
  as an **icon before the name** in the grid's «Aggiungi milestone qui» menu
  (`MilestoneKind::icon()` — `⚑` / `⚡`, glyphs already covered by egui's bundled
  fonts, checked by `milestone_kind_icons_are_renderable`); the week column itself
  is unchanged. In **PDF/SVG** `pole_tip` draws the usual triangular pennant for
  `Goal` and a **mini lightning bolt** (6-point polygon) for `Trigger`, same color,
  same pole/label/arch layout.
- **Milestone categories** (`MilestoneCategory { Internal, External }`, field
  `Milestone.categories: Vec<MilestoneCategory>`, `#[serde(default,
  skip_serializing_if = "Vec::is_empty")]`): **print-only** metadata — the grid
  ignores them. **Empty = Internal**, so old RON files keep printing as before;
  `Milestone::has_category` / `effective_categories` apply that fallback and
  `Milestones::set_category(id, cat, on)` normalizes through
  `normalize_categories` (ordered, deduped, **never empty** — dropping the last
  one falls back to Internal, and the UI disables that last toggle). Picked at
  creation (**Aggiungi ▸ Milestone**, `milestone_categories_submenu` →
  `UiState.new_milestone_categories` → `Action::CreateMilestone(name, kind,
  categories)` → `Milestones::add_with`) and editable per row in
  `milestone_manager_window` (`milestone_categories_toggles` compact Int/Ext
  selectable labels → `Action::SetMilestoneCategory`). Same two widgets rule as
  the kind: a **submenu** inside toolbar menus, plain widgets inside windows.
- **Project-owned milestones** (`Milestone.owner: Option<ProjectId>`,
  `#[serde(default, skip_serializing_if = "Option::is_none")]`, so old files load
  as global): `None` = system-wide milestone, `Some(p)` = private to that project.
  `Milestones::list_for_project(p)` (globals + `p`'s own) drives the grid's
  «Aggiungi milestone qui» menu — `list()` stays "everything" and is what the
  manager shows, with the owner's `project_label` beside the name. Created only
  from that grid menu: a `milestone_categories_submenu` («Categorie: …», bound to
  `UiState.new_project_milestone_categories`, remembered across insertions) over
  the `add_field` «Nuova milestone solo qui…» →
  `Action::CreateProjectMilestone { proj, week, name, categories }` →
  `add_owned(name, Goal, categories, Some(proj))` **plus** `add_project_milestone`
  (created and placed in one go; kind/color are adjusted later in the manager,
  and so are the categories if needed). The submenu's toggle rule lives in the
  free fn `toggle_category` (ordered, deduped, never empty), shared with the
  toolbar path and unit-tested. The scope is
  fixed at creation — there is no global⇄project switch. `App::validate` reports
  a placement whose milestone belongs to another project, and
  `Milestones::purge_project` exists for dropping a project's own milestones.
  **Menu gotcha**: that submenu is built with `SubMenuButton` +
  `MenuConfig::close_behavior(CloseOnClickOutside)` — a context menu defaults to
  `CloseOnClick`, which closes the whole menu the moment you click **into the
  text field**, so the name could never be typed (caught by, and guarded with,
  `project_milestone_field_keeps_the_context_menu_open`, which drives real
  right-click/click/type/Enter events). `add_field` returns the text field's
  `Response` for that test.
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

**Exports are in English with US dates** (the GUI stays Italian): every string drawn
into PDF/SVG is English (`MONTHS_EN`, "Today"/"today", report headings…), full dates use
`US_DATE` (`%m/%d/%Y` — footer `created`, report start/end) and `short_date` gives
`"Sep 22"` (month first). Keep new export text in English.

Drawing is backend-agnostic: `page_shapes(...)` builds a `Vec<Shape>` (Rect/Poly/
Line/Text in mm, origin bottom-left), then `render_pdf(fonts, &shapes)` emits
printpdf `Op`s and `render_svg(&shapes)` emits an SVG string (Y-flipped, cropped to
content). `project_shapes(app, proj, name, dev_info, today, created, order, chart_only, fmt)`
gathers rows/flags for a project; `chart_only` drops the tripletta/description and
the footer date.

- **Export options are a parameter, not global state**: every Gantt builder
  (`build_pdf_selected`, `build_pdf_project_fmt`, `build_svg_project_fmt`, and the
  test-only wrappers) takes `opts: &ExportOptions` (`pdf_export.rs`): `show_pct`
  (default off), `show_year` (on), `scope` (`Mix`), `milestones:
  Option<HashSet<MilestoneId>>` (`None` = all), `show_ghost` (on), `ghost_red` (on),
  `range: Option<(i32, i32)>` (`None` = auto axis). It flows `project_shapes_fmt` →
  `page_shapes` → helpers (`dev_date`/`dev_range`/`segment_dates` get `show_year`,
  `pct_label` gets `show_pct`). The UI builds it with
  `UiState::export_options(milestones, range)` from the remembered dialog choices;
  `scoped_exports(&base, &scopes, |opts| …)` clones it once per scope. There is
  nothing to reset after an export. Tests pass `&ExportOptions::default()` or
  `ExportOptions { field: …, ..Default::default() }`.
- **Progress % in export** (`ExportOptions::show_pct`, default off): when on,
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
  (remembered across exports, not persisted), copied into `ExportOptions::show_pct`.
- **Year in dev dates** (`ExportOptions::show_year`, default **on**): the «Includi l'anno nelle date dei dev ('26)» checkbox
  (`year_checkbox` in `export.rs`, `UiState.export_show_year: ExportYear` — a
  newtype like `ExportGhost` so it starts **on**; remembered, not persisted) of both Gantt dialogs. `dev_date(days, show_year)` = `short_date` + ` 'YY`
  (`"Sep 22 '26"`) and is used **only** for the date label after each dev bar;
  milestone flag dates keep `short_date`.
  The label is `dev_range(start, end, show_year)` with end = **Friday** of the last effort week
  (`r.end_day + DEV_END_OFFSET`, 4 days after its Monday), so a one-week dev reads
  `Mon - Fri` of that week; with the year on, a same-year range shows it only on the
  end (`Sep 21 - Oct 9 '26`), a cross-year one on both. Only the label: the bar already spans to `end_day + 7`.
- **Remembered dialog choices** (in memory only, lost on exit): the single-project
  dialog stores, **per project**, dev ticks + order and milestone ticks in
  `UiState.export_project_memory: HashMap<ProjectId, ExportProjectMemory>`
  (written every frame by `pdf_export_window`, also on Annulla); `Action::ExportPdf`
  rebuilds the lists with `restore_dev_entries` (remembered devs keep order/tick,
  new ones appended ticked) and `milestones.get(id).unwrap_or(true)`. The
  multi-project dialog keeps `UiState.export_multi_memory: HashMap<ProjectId, bool>`
  (projects never seen start ticked). Global options (format, %, year, ghost,
  scopes) were already remembered.
- **Export period** (single-project dialog only): `period_row` (`export.rs`, top
  row, two `DatePickerButton`s + «Inizio/fine progetto») edits `PdfExport.range:
  (i32, i32)` (days). First opening = `project_export_range(app, proj)`
  (start/end of the project); then it is remembered **per project** in
  `ExportProjectMemory.range` and never touched again, even if the project's dates
  change. Rides in `Action::ExportPdfProject/ExportSvgProject { …, range }` →
  `ExportOptions::range`. `ExportOptions::window()` snaps it to whole weeks `[Monday(from),
  Monday(to)+7)`; `project_shapes_fmt` drops effort weeks, ghost weeks and
  milestones outside it (`in_window`; a dev with effort **only outside** gets
  `Row.off = Some(OffWindow::{Ended, NotStarted, Gap})` (`OffWindow::of`) and
  `page_shapes` draws no bar/line, just `dev_range` + `pct_label` at the chart's
  left edge / right edge / centre — test
  `devs_outside_the_export_range_show_their_dates_at_the_edges`; a dev with no
  effort at all stays a `no_effort` thin row; `max_week` stays the all-weeks max so `Proportional`
  heights don't change), and `page_shapes` uses `opts.window()` as the exact axis
  instead of the month-snapped auto range (+2 months). Bars are clipped by
  `x_of`'s clamp while the date label keeps the dev's real `start_day/end_day`;
  when the label fits neither right nor left of a bar clipped at both edges it
  goes above the bar, right-aligned. `from > to` disables the export buttons.
  Because the window snaps to whole weeks it can start with a few days of the
  previous month (e.g. «Da» 1/1/2026 → Mon 29/12/2025), too narrow for a month
  label: the **orange year labels** at the axis ends use the first/last month whose
  name is actually drawn (`labeled` in `page_shapes`), not the axis extremes —
  otherwise the chart reads «2025 · Jan Feb…». Test
  `year_labels_follow_the_first_and_last_readable_month`.
  Tests `export_range_limits_axis_milestones_and_devs`,
  `export_range_starts_from_project_dates_and_is_remembered_per_project`.

- **Milestone scope** (`MilestoneScope { Internal, External, Mix }` in
  `milestones.rs`; `ExportOptions::scope`, default `Mix`): which flags
  `project_shapes` draws.
  `Internal` («Solo internal») → those carrying `MilestoneCategory::Internal`
  (uncategorized = Internal); `External` («Solo external») → those carrying
  `External`; an Int/Ext milestone is in **both**; `Mix` → **all**
  (`Milestones::in_scope`). Chosen in **every** Gantt export dialog by
  `milestone_scope_selector` (`src/ui/export.rs`, three checkboxes) bound to
  `UiState.export_scopes: ExportScopes { internal, external, mix }` (default **all
  three** ⇒ three files, remembered across exports, not persisted; the export
  buttons are disabled when none is ticked). The list goes into
  `Action::ExportPdfSelected/ExportPdfProject/ExportSvgProject { …, scopes }`,
  whose handlers call `scoped_exports(&base, &scopes, build)` (`mod.rs`): one
  `build_*` per scope with `ExportOptions { scope, ..base }`, each tagged with
  `MilestoneScope::file_suffix()`. Saving goes through `save_multi_export_dialog`: **one**
  rfd dialog, one file with the chosen name when a single scope was picked, and
  `path_with_suffix` (`name_internal.pdf` / `name_external.pdf` / `name_mix.pdf`)
  when more than one was
  (`save_svg_dialog` is gone; SVG goes through the same path). The trend PDF has
  no milestones, so it is unaffected.
- **Per-milestone selection** (single-project dialog only): `PdfExport.milestones:
  Vec<(MilestoneId, WeekId, bool)>` is filled when `Action::ExportPdf` opens
  `pdf_export_window` (from `list_project_milestones`, sorted by week then id,
  ticked unless remembered otherwise — see **Remembered dialog choices**). The dialog draws a «Milestone da includere» checklist (Select All +
  one checkbox per milestone, labelled `icon nome — yy-mm-dd [Int/Ext]` in the
  milestone color) **beside** the dev list (two `ui.columns`, so the dialog stays short on 1920×1080 @125%); the ticked ids ride along in
  `Action::ExportPdfProject/ExportSvgProject { …, milestones }` and become
  `ExportOptions::milestones = Some(ids)` (`None` = no restriction, what the
  multi-project export uses). `ExportOptions::milestone_shown` applies
  **allow-list AND scope**, so a milestone must be ticked *and* in scope.
- **Ghost in export** (`ExportOptions::show_ghost`, default **true** = historical
  behavior): the «Includi i worker ghost (anche a
  effort 0)» checkbox of both Gantt dialogs (`ghost_checkbox` in `export.rs`,
  bound to `UiState.export_ghost: ExportGhost` — a newtype because `UiState`
  derives `Default` and this one starts **on**; remembered across exports, not
  persisted). `ExportGhost { show, red }`: `show` is the checkbox, `red` a
  sub-option (radio «— colore: Rosso / Colore del dev» on the same row as the checkbox, enabled only when
  `show`, default red), copied into `ExportOptions::show_ghost` / `ghost_red`. In
  `page_shapes` the overlays use `ghost_col` = `RED` or the
  row's `r.color`; with dev color the dev-name label stays **black**. Gantt
  PDF/SVG only — the trend PDF ignores it (always red). Effects inside `project_shapes`: `ghost_weeks(dev)` is
  empty when off, and when on it uses `SingleDev::weeks_with_worker_assigned`
  (**assignment, effort 0 included**) instead of the >0-only `weeks_with_worker`
  that the trend PDF still uses. With it on, the `order = None` path (multi-project
  export, normally devs **with** effort only) also emits a `no_effort` row for a dev
  whose sole assignment is a zero-effort ghost, and `page_shapes` paints red
  segments over the thin `no_effort` line (it `continue`s before the usual bar
  overlay), so a ghost at 0 is visible either way.
- **Rounded corners** (`App.corner_pct`, `#[serde(default = "default_corner_pct")]`,
  **always serialized**, no UI — same family as `week_color`/`month_tint_pct`;
  default `pdf_export::CORNER_PCT_DEFAULT` = 40, out-of-range reported by
  `App::validate`): percentage of the **maximum** radius (`pct/100 · min(w,h)/2`),
  so 0 = square and 100 = pill-shaped ends. It is a file setting, not a dialog
  choice, so it is **not** in `ExportOptions`: `project_shapes_fmt` reads
  `app.corner_pct` (clamped 0–100) and passes it as `corner` to `page_shapes` →
  `rect_round(…, corner)` / `round_rect_pts(…, corner)`. `rect_round(...)` replaces
  `rect_fill` for the **effort bars** (all three `BarFormat`s), the **ghost
  overlay** on them, the **month axis cells** and the red **"Today" progress bar**
  on the axis; it emits a `Shape::Poly` from
  `round_rect_pts` (6 segments per corner, no renderer change needed since Poly
  already renders to PDF and SVG) and falls back to a real `Shape::Rect` at 0.
  Today's vertical line/triangle, the thin no-effort row and the grey footer band stay square.
  **Missing file-only settings are written on load**: `App::load_reporting_missing`
  returns `missing_file_settings(raw)` (`week_color`/`month_colors`/`month_tint_pct`/
  `corner_pct` absent from the text); `PjmApp::write_missing_settings` (startup via
  `PjmApp::new`, and **File ▸ Apri…**) saves immediately + info toast, so an old file
  gains `corner_pct: 40` without waiting for another edit.
  The ghost overlay merges **consecutive** ghost weeks into one rect via
  `contiguous_runs` (both on the bar and on the thin no-effort row) — one rect per
  week would read as a row of detached tiles once the corners are rounded; the
  `Proportional` format keeps one rect per week because each has its own height.
  **Test gotcha**: bars are polygons now, so shape-counting tests must count
  `Rect` **and** `Poly` (see `segmented_and_proportional_formats_produce_valid_output`).
- **Bar format** (`BarFormat`, passed through every `build_*`): how a dev's bar is
  drawn. `Continuous` (default, historical) = one rect first→last effort week;
  `Segmented` = one rect per run of consecutive effort weeks (gaps show), via
  `contiguous_runs`; `Proportional` = one rect per week, half-height ∝ week-sum /
  `proportional_ref(max_week)` — the dev's max weekly sum but never below
  `PROPORTIONAL_REF_MIN` (40h), so a 40h week is only full-height if the dev never
  works more elsewhere (busiest week reaches full `bar_hh`, same as the other two).
  `Row` carries `weeks: Vec<(day, hours)>` + `max_week` for the last two, and its own
  **`fmt`**: the format is **per dev** (`page_shapes` reads `r.fmt`, it no longer takes
  a format). `project_shapes(…, fmt)` is a wrapper giving every dev the same format
  over `project_shapes_fmt(…, fmt_of: &dyn Fn(DevId) -> BarFormat)`; likewise
  `build_pdf_project`/`build_svg_project` wrap `build_*_project_fmt(app, proj,
  &[(DevId, BarFormat)])`, which `Action::ExportPdfProject/ExportSvgProject { devs:
  Vec<(DevId, BarFormat)>, … }` call. Single-project dialog: a `dev_bar_format_combo`
  per dev row (`PdfExport.formats`, remembered per project in
  `ExportProjectMemory.formats`; unseen devs start from `UiState.bar_format`), and
  `bar_format_selector(ui, title, &mut fmt) -> bool` (live
  `draw_bar_format_preview` thumbnails) acts as **«applica a tutti»** — it returns
  `true` on any click, even on the already-selected option. Multi-project dialog: the
  same selector, one format for all (`UiState.bar_format`, remembered, **not**
  persisted).
  **Dates below the runs**: the per-dev choice is a `DevBar { fmt, dates_below }`
  (`From<BarFormat>` = no dates; `dates_below()` is false for `Continuous`), so the
  `*_fmt` builders take `&[(DevId, DevBar)]` and `fmt_of` returns `DevBar`;
  `build_pdf`/`build_pdf_selected` take `impl Into<DevBar>`. The dev combo lists
  `DEV_BAR_CHOICES` (5 entries); the multi dialog adds the checkbox
  `UiState.export_dates_below`. `Row.dates_below` → in `page_shapes` each
  `contiguous_runs` run gets start (Monday) / end (Friday, `+DEV_END_OFFSET`) on two
  lines centred under it (`segment_dates`, `SEG_DATE_FONT`), and the right label
  keeps only the percentages (nothing if they are off). Rows are **variable-height**:
  dates rows add `SEG_DATES_H`, and everything scales down together when the page
  would overflow.
- **Test-only wrappers** (`#[cfg(test)]`, the app calls the `*_fmt` / `_selected`
  variants): `build_pdf`, `build_pdf_project`, `build_svg_project`, `project_shapes`.
  The model modules no longer carry `#![allow(unused)]`/`#![allow(dead_code)]`:
  helpers used only by tests are marked `#[cfg(test)]` so dead code shows up as a warning.
- `build_pdf(app, fmt)` — one Gantt page per eligible project (enabled, not closed, has
  start AND end). Dev rows are only those **with** effort, sorted by start date (`order=None`).
- **One PDF writer**: every `build_*_pdf` turns its pages into `Vec<Shape>` and calls
  `pdf_document(title, &pages)` (fonts, `PdfPage`s, save; `None` with no pages).
  `build_pdf`/`build_pdf_selected` share `gantt_pdf(app, bar, keep)`.
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
  legend, a red **"today"** line, a **red horizontal 100% reference line** (the rest of
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
