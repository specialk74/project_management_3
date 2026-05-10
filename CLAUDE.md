# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run
cargo run

# Run all tests
cargo test

# Run a single test
cargo test <test_name>
# e.g.: cargo test workers::workers::tests::add_returns_new_id
```

The default data file is `workers.ron`, loaded from the working directory on startup. A different file can be specified as a CLI argument: `cargo run -- myfile.ron`. The "Apri" button opens a native file-picker dialog (via the `rfd` crate) filtered to `.ron` files; the chosen file becomes the new `current_file`.

## Architecture

This is a **Slint + Rust** project management effort tracker. The UI is declared in `.slint` files; Rust handles all logic and state. Slint generates Rust bindings at build time via `build.rs`.

### Data Model

```
App (workers.ron)
├── Workers  — named workers (human resources)
├── Devs     — dev roles (e.g. "Frontend", "Backend")
└── Projects
    └── Project
        └── SingleDev (per DevId)
            ├── effort: Effort       — planned hours
            ├── note: Option<String> — dev-level note
            └── weeks: HashMap<WeekId, SingleEffortWeek>
                └── SingleEffortWeek
                    └── worker_id: HashMap<WorkerId, SingleEffort>
                        ├── effort: Effort
                        └── note: Option<String>
```

- `WeekId(usize)` is an absolute week number
- `Effort(usize)` stores hours directly (not percentage points); `get_hours()` in `builders.rs` is an identity function
- Cell display format: `"WorkerName|effort_value"` (the `|` separator is parsed in Rust callbacks)

### UI ↔ Rust Data Flow

**The critical binding pattern** (see `live_models.rs` comment): Slint does not propagate changes when an `in-out property<Struct>` field is mutated. All mutable data uses stable `Rc<VecModel<T>>` instances. After any state change in Rust, call `ui_sync::refresh()` which calls `set_vec()` on each `LiveModels` field to trigger UI updates.

```
App (Rust state)
  → builders.rs  (pure functions, App → Slint structs)
  → LiveModels   (stable Rc<VecModel> instances kept alive in main)
  → AppWindow.app_project (Slint binding)
```

### Callback Pattern

All Slint→Rust callbacks are registered in `src/callbacks/`. Each file handles a domain:
- `effort.rs` — effort changes, note setting, drag/move
- `members.rs` — add/search workers and devs, autocomplete
- `project.rs` — new project, rename, set tripletta, enable/disable, add/remove dev, set end week
- `rows.rs` — add/delete worker rows per dev
- `file_ops.rs` — save/load RON file (load uses native file dialog via `rfd`)

Every callback follows this pattern:
1. Clone `Rc<RefCell<App>>` and `Rc<LiveModels>` before the closure
2. Borrow `app` mutably, mutate state
3. Call `refresh(&ui, &a, &live, ...)` to push changes to UI
4. Call `PjmCallback::get(&ui).set_changed(true)`

### UI Files

- `global.slint` — all exported types (`SingleEffortGui`, `EffortByDateData`, etc.), `PjmCallback` global singleton, reusable components (`Cell-RW`, `Cell-RO`, `NoteEditorWindow`, `EffortByDataGui`, `EffortByDevGui`, `EffortByPrjGui`)
- `app-window.slint` — root `AppWindow`, `Toolbar` (includes "Progetti ▼" popup for enable/disable), search popup (`im`)
- `left-column.slint` — project list with dev rows; per-project: tripletta `TextEdit` (editable, always visible), project name `TextEdit`, deadline row (right-click to edit), `Devmenu` context menu, dev note editing
- `right-column.slint` — scrollable effort grid
- `header.slint` / `left-footer.slint` / `right-footer.slint` — week headers and totals
- `styles.slint` — global `Styles` singleton (sizes, colors, fonts)

### Key UI Mechanics

**Cell editing**: `Cell-RW` uses a `FocusScope` + `LineEdit`. Press `Return` to enter edit mode; autocomplete triggers via `PjmCallback.find_completion()`. Cell value format is `"WorkerName|effort"`.

**Note system**: A yellow triangle indicator appears on cells/devs with notes. `Ctrl+M` on a non-empty cell opens `NoteEditorWindow` for effort notes. Right-click a dev → "Nota Dev..." opens it for dev-level notes.

**Viewport sync**: Left column (project names) and right column (effort grid) share `viewport_y` via two-way binding in `AppWindow` to scroll in sync.

**Row counts / visibility**: Two `HashMap<(project_idx, dev_idx), _>` maps in `SharedState` track how many worker rows to show per dev and which devs are visible (used by the search filter). These are passed to `builders.rs` on every refresh.

**Project IDs in the UI**: `EffortByPrjData.project_id` is the 0-based sorted-position index (`pi` in `builders.rs`), not the internal `ProjectId(usize)` key. All callbacks (`set_project_name`, `set_project_end_week`, `set_project_enabled`, etc.) receive this index and resolve it via `projects.list().get(idx)`.

**+Dev / -Dev filtering**: `EffortByPrjData` carries two per-dev bool arrays computed in `builders.rs`: `dev_in_project` (true if dev is already in the project) and `dev_has_data` (true if dev has planned effort or any week data). `DevSelectPopup` uses these to filter the list: `+Dev` shows only devs NOT in project; `-Dev` shows only devs IN project. When removing a dev that has data, a confirmation popup (`confirm-dev-popup` in `LeftColumn`) is shown; without data, removal is immediate. The callback chain is: `DevSelectPopup.confirm-remove(proj, dev)` → `LeftColumn` sets state → `confirm-dev-popup.show()` → on confirm: `PjmCallback.add_dev_to_project(proj, dev, false)`.

**Project enable/disable**: `Project.enable: Enable(bool)` persisted in RON. The "Progetti ▼" button in the toolbar opens a `PopupWindow` with a `ListView` listing all projects (enabled and disabled) with a checkbox per row. Toggling calls `PjmCallback.set_project_enabled(project_id, bool)` → `projects.set_enable(ProjectId, Enable)` → `refresh`. Disabled projects are hidden in the main grid (`visible: project.visible && project.enable`) but always appear in the popup list.

**Tripletta**: `Project.tripletta: Option<String>` persisted in RON (`#[serde(default)]`, backward-compatible). Set at project creation and edited via right-click popup in the left column (same pattern as deadline). When non-empty shows styled `Text` (bold, `Styles.effort-color`); when empty shows a dim `"—"` placeholder — always `Styles.height` tall so right-click is always reachable. Editing calls `PjmCallback.set_project_tripletta(project_id, text)` → `projects.set_tripletta(ProjectId, &str)` → `refresh`. Displayed in "Progetti ▼" popup as `"index tripletta"` when non-empty.
