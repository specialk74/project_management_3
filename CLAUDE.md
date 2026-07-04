# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> User-facing manual: see **`docs/MANUALE.md`** (Italian, complete). It is embedded
> into the app via `include_str!` and shown by the in-app Help window
> (**Aiuto ▸ Manuale d'uso…**). Keep it in sync when you change user-visible behavior.

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
references; the UI lives entirely in `src/ui.rs`.

### Source layout

- `src/main.rs` — module list, `eframe::run_native`, loads the RON file, launches `PjmApp`.
- `src/app.rs` — `App`: the whole persisted state; RON (de)serialization (`to_ron_string` / `from_ron_str`).
- `src/ui.rs` — the `eframe::App` (`PjmApp`), `UiState`, the `Action` enum, and **all** drawing (toolbar, header, grid, footer, dialogs). This is the big file.
- `src/ui_style.rs` — sizes, fonts, colors, theme (light/dark) and B/W mode, `cumulative_color`.
- `src/pdf_export.rs` — PDF Gantt export (`build_pdf`, `build_pdf_project`).
- Data modules: `workers_utils/`, `dev_utils/`, `project_utils/`, `single_dev_utils/`, `single_effort_utils/`, `categories.rs`, `milestones.rs`, `date_utils/`, `sync_merge.rs`.

### Data Model

```
App (workers.ron)
├── start_week / end_week : WeekId   — grid time range
├── workers   : Workers              — named people (max hours, colors, hidden-in-footer)
├── devs      : Devs                 — roles (e.g. "Frontend"); each has bg+font color
├── categories: Categories
├── milestones: Milestones           — name + color, shared across projects
├── sovra     : HashMap<(WeekId, WorkerId), Effort>  — per-week per-worker allocation
└── projects  : Projects
    └── Project (tripletta, name, start, end, category, enable, closed, milestones)
        └── SingleDev (per DevId)
            ├── effort : Effort              — planned hours
            ├── note   : Option<String>      — dev-level note
            ├── hide_effort : bool
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
- Theme is resolved at the top of `PjmApp::ui()` from `UiState.theme_pref`
  (`Auto`/`Light`/`Dark`); `Auto` reads `ctx.system_theme()`. egui's own `Visuals`
  are set to match so menus/popups/text-edits follow the theme too.

### Toolbar menus (in `toolbar`)

- **File**: Salva (`Cmd/Ctrl+S`), Apri…, Esporta PDF…, Esci.
- **Aggiungi**: + Progetto, and `add_field` inputs for Worker / Dev / Categoria / Milestone.
- **Filtri**: Progetti… (enable/disable visibility), Workers… (`Cmd/Ctrl+F`), Milestone… (manager), Closed…
- **Vista**: Vista compatta, Bianco/Nero, **Tema** (Auto/Chiaro/Scuro), **Zoom settimane** (Normale/2/4).
- **Aiuto**: Manuale d'uso… (opens `help_window`).

### Dialogs / windows

Each modeless window is a `fn xxx_window(ctx, …)` guarded by a `UiState` field and
called from the windows block in `ui()`. Examples: `dev_manage_window`
(`state.dev_manage`), `pdf_export_window` (`state.pdf_export`), `help_window`
(`state.show_help`), `project_filter_window`, `milestone_manager_window`,
`move_dialog_window`, `popup_window` (single `Popup` enum for tripletta/start/end/
category/worker-max/etc.).

### Key UI mechanics

- **Cell editing**: left-click a grid cell to edit; typing triggers worker-name
  autocomplete; `Enter`/`Tab` commit, `Esc` cancels. Cell value is `"Worker|effort"`.
  `Cmd/Ctrl+C/X/V` copy/cut/paste (carrying the cell note for internal paste).
- **Notes** (yellow triangle indicator): right-click a non-empty cell → effort note;
  right-click a dev name → **Nota Dev…**; right-click a footer worker cell → **Note**.
- **Milestone / move**: right-click the top strip of a dev's column → **Aggiungi
  milestone qui** and **Sposta** (blocco / devs).
- **Worker filter active** → `draw_project_info` shows **only the tripletta** (other
  info hidden so it adds no height); projects with no matching workers disappear.
- **Zoom (merge weeks)** → merged columns are **read-only**, sum effort, hide worker
  names (per dev: cumulative row + one summed cell); milestones/start/end stay
  visible; a group never crosses the year boundary. Only in non-compact view.
- **Year boundary**: `Col::YearEnd` canary column shows per-dev missing hours and a
  project total (`T:…`) for projects straddling the year.

### PDF export (`pdf_export.rs`)

- `build_pdf(app)` — one Gantt page per eligible project (enabled, not closed, has
  start AND end). Dev rows are only those **with** effort, sorted by start date.
- `build_pdf_project(app, proj, ordered_devs)` — single project, dev order chosen by
  the user. Devs **with** effort → normal colored bar; devs **without** effort → a
  thin line spanning the full calendar width. Exports even with an empty dev list.
- Triggering (in `Action::ExportPdf`): if exactly **one** project is visible
  (enabled and not closed) the UI opens `pdf_export_window` (Select All, drag-to-
  reorder via egui `dnd_drag_source`/`dnd_release_payload`, per-dev checkbox);
  otherwise it exports all projects.

### RON persistence & external changes

`App` (de)serializes to RON. New persisted fields must be `#[serde(default)]` for
backward compatibility. The app watches the file's mtime and, if it changes on disk
while open, either notifies + auto-applies or asks (Mantieni le mie / Ricarica);
see `sync_merge.rs`.
