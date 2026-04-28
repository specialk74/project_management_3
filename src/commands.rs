use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app::App;
use crate::dev_utils::dev::DevId;
use crate::dto::{build_dto, AppStateDto};
use crate::projects::project::ProjectId;
use crate::single_dev::single_dev::WeekId;
use crate::single_efforts::sinlge_effort::Effort;
use crate::workers::worker::WorkerId;
use crate::TauriState;

fn dto(state: &TauriState) -> AppStateDto {
    let app = state.app.lock().unwrap();
    let filter = state.filter.lock().unwrap();
    let current_file = state.current_file.lock().unwrap();
    let changed = *state.changed.lock().unwrap();
    build_dto(&app, &filter, &current_file, changed, state.this_week)
}

fn set_changed(state: &TauriState, val: bool) {
    *state.changed.lock().unwrap() = val;
}

#[tauri::command]
pub fn get_state(state: State<TauriState>) -> AppStateDto {
    dto(&state)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CellUpdate {
    pub worker_name: String,
    pub effort_pct: i32,
}

#[tauri::command]
pub fn set_cells(
    state: State<TauriState>,
    project_idx: usize,
    dev_id: usize,
    week: usize,
    cells: Vec<CellUpdate>,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        let dev_id = DevId(dev_id);
        app.projects.reset_effort(proj_id, dev_id, WeekId(week));
        for cell in &cells {
            if cell.worker_name.is_empty() {
                continue;
            }
            if let Some(worker_id) = app.workers.get_id_by_name(&cell.worker_name) {
                app.projects.add_effort(
                    proj_id,
                    dev_id,
                    WeekId(week),
                    worker_id,
                    Effort(cell.effort_pct as usize),
                );
            }
        }
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn set_dev_effort(
    state: State<TauriState>,
    project_idx: usize,
    dev_id: usize,
    effort_pct: i32,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        app.projects
            .add_dev_effort(proj_id, DevId(dev_id), Effort(effort_pct as usize));
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn set_note(
    state: State<TauriState>,
    project_idx: usize,
    dev_id: usize,
    week: usize,
    worker_name: String,
    note: String,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        if let Some(worker_id) = app.workers.get_id_by_name(&worker_name) {
            app.projects
                .set_note(proj_id, DevId(dev_id), WeekId(week), worker_id, &note);
        }
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn set_dev_note(
    state: State<TauriState>,
    project_idx: usize,
    dev_id: usize,
    note: String,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        app.projects.set_dev_note(proj_id, DevId(dev_id), &note);
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn move_effort(
    state: State<TauriState>,
    project_idx: usize,
    dev_id: usize,
    start_week: usize,
    end_week: usize,
    start_row: usize,
    end_row: usize,
    offset: i32,
) -> AppStateDto {
    if offset == 0 {
        return dto(&state);
    }
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        let dev = DevId(dev_id);
        let workers = app.workers.list();
        let max_row = end_row.min(workers.len().saturating_sub(1));

        let mut moves: Vec<(usize, WorkerId, usize)> = Vec::new();
        for w in start_week..=end_week {
            for r in start_row..=max_row {
                if let Some(&(worker_id, _)) = workers.get(r) {
                    let e = app
                        .projects
                        .get_single_dev(proj_id, dev)
                        .map_or(0, |sd| sd.get_effort(WeekId(w), worker_id).0);
                    moves.push((w, worker_id, e));
                }
            }
        }
        for w in start_week..=end_week {
            for r in start_row..=max_row {
                if let Some(&(worker_id, _)) = workers.get(r) {
                    app.projects
                        .add_effort(proj_id, dev, WeekId(w), worker_id, Effort(0));
                }
            }
        }
        for (w, worker_id, effort) in moves {
            let dst_w = (w as i32 + offset) as usize;
            app.projects
                .add_effort(proj_id, dev, WeekId(dst_w), worker_id, Effort(effort));
        }
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn add_row(state: State<TauriState>, project_id: usize, dev_id: usize) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = ProjectId(project_id);
        let dev = DevId(dev_id);
        if let Some(week) = app.projects.get_week_with_max_worker(proj_id, dev) {
            app.projects
                .add_effort(proj_id, dev, week, WorkerId(0), Effort(0));
        }
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn del_row(state: State<TauriState>, project_id: usize, dev_id: usize) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        app.projects.del_row(ProjectId(project_id), DevId(dev_id));
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn new_project(state: State<TauriState>) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        app.projects.add("Nuovo Progetto");
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn rename_project(
    state: State<TauriState>,
    project_idx: usize,
    name: String,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        app.projects.set_project_info(proj_id, &name);
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn add_dev_to_project(
    state: State<TauriState>,
    project_idx: usize,
    dev_id: usize,
    add: bool,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let proj_id = {
            let projects = app.projects.list();
            match projects.get(project_idx) {
                Some(&(id, _)) => id,
                None => return dto(&state),
            }
        };
        let dev = DevId(dev_id);
        if add {
            app.projects.add_dev(proj_id, dev);
        } else {
            app.projects.del_dev(proj_id, dev);
        }
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn add_worker(state: State<TauriState>, name: String) -> AppStateDto {
    if !name.is_empty() {
        let mut app = state.app.lock().unwrap();
        app.workers.add(&name);
        app.compute_sovra();
        set_changed(&state, true);
    }
    dto(&state)
}

#[tauri::command]
pub fn add_dev(state: State<TauriState>, name: String) -> AppStateDto {
    if !name.is_empty() {
        let mut app = state.app.lock().unwrap();
        app.devs.add(&name);
        app.compute_sovra();
        set_changed(&state, true);
    }
    dto(&state)
}

#[tauri::command]
pub fn set_worker_max_hours(
    state: State<TauriState>,
    worker_idx: usize,
    hours: u32,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let hours = hours.clamp(1, 40);
        app.set_worker_max_hours_by_idx(worker_idx, hours);
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn set_worker_week_override(
    state: State<TauriState>,
    worker_idx: usize,
    week: usize,
    hours: u32,
) -> AppStateDto {
    {
        let mut app = state.app.lock().unwrap();
        let hours = hours.clamp(1, 40);
        app.set_worker_week_override_by_idx(worker_idx, week, hours);
        app.compute_sovra();
    }
    set_changed(&state, true);
    dto(&state)
}

#[tauri::command]
pub fn search_worker(state: State<TauriState>, name: String) -> AppStateDto {
    *state.filter.lock().unwrap() = name;
    dto(&state)
}

#[tauri::command]
pub fn save_file(state: State<TauriState>, path: String) -> Result<(), String> {
    let app = state.app.lock().unwrap();
    app.save(&path);
    *state.current_file.lock().unwrap() = path;
    *state.changed.lock().unwrap() = false;
    Ok(())
}

#[tauri::command]
pub fn open_file(state: State<TauriState>, path: String) -> Result<AppStateDto, String> {
    match App::load(&path) {
        Ok(mut loaded) => {
            loaded.compute_sovra();
            *state.app.lock().unwrap() = loaded;
            *state.current_file.lock().unwrap() = path;
            *state.changed.lock().unwrap() = false;
            *state.filter.lock().unwrap() = String::new();
            Ok(dto(&state))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn find_completions(state: State<TauriState>, prefix: String) -> Vec<String> {
    if prefix.is_empty() {
        return vec![];
    }
    let app = state.app.lock().unwrap();
    app.workers
        .list()
        .into_iter()
        .filter(|(_, name)| name.starts_with(&prefix))
        .map(|(_, name)| name)
        .collect()
}
