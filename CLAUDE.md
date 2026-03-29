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

The data file `workers.ron` is loaded/saved at runtime in the working directory. The app auto-loads it on startup.

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
- `Effort(usize)` stores percentage points (not hours); `get_hours()` converts via `* 40 / 100`
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
- `project.rs` — new project, rename
- `rows.rs` — add/delete worker rows per dev
- `file_ops.rs` — save/load RON file

Every callback follows this pattern:
1. Clone `Rc<RefCell<App>>` and `Rc<LiveModels>` before the closure
2. Borrow `app` mutably, mutate state
3. Call `refresh(&ui, &a, &live, ...)` to push changes to UI
4. Call `PjmCallback::get(&ui).set_changed(true)`

### UI Files

- `global.slint` — all exported types (`SingleEffortGui`, `EffortByDateData`, etc.), `PjmCallback` global singleton, reusable components (`Cell-RW`, `Cell-RO`, `NoteEditorWindow`, `EffortByDataGui`, `EffortByDevGui`, `EffortByPrjGui`)
- `app-window.slint` — root `AppWindow`, toolbar, search popup (`im`)
- `left-column.slint` — project list with dev rows, `Devmenu` context menu, dev note editing
- `right-column.slint` — scrollable effort grid
- `header.slint` / `left-footer.slint` / `right-footer.slint` — week headers and totals
- `styles.slint` — global `Styles` singleton (sizes, colors, fonts)

### Key UI Mechanics

**Cell editing**: `Cell-RW` uses a `FocusScope` + `LineEdit`. Press `Return` to enter edit mode; autocomplete triggers via `PjmCallback.find_completion()`. Cell value format is `"WorkerName|effort"`.

**Note system**: A yellow triangle indicator appears on cells/devs with notes. `Ctrl+M` on a non-empty cell opens `NoteEditorWindow` for effort notes. Right-click a dev → "Nota Dev..." opens it for dev-level notes.

**Viewport sync**: Left column (project names) and right column (effort grid) share `viewport_y` via two-way binding in `AppWindow` to scroll in sync.

**Row counts / visibility**: Two `HashMap<(project_idx, dev_idx), _>` maps in `SharedState` track how many worker rows to show per dev and which devs are visible (used by the search filter). These are passed to `builders.rs` on every refresh.
