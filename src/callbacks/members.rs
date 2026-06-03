use slint::{ComponentHandle, Global};
use std::collections::HashSet;

use crate::app::App;
use crate::ui_sync::{refresh, sync_project_texts};
use crate::workers_utils::worker::{DEFAULT_MAX_HOURS, WorkerId, WORKER_ID_ZERO};
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_add_worker(ui, state);
    register_add_dev(ui, state);
    register_find_completion(ui, state);
    register_search(ui, state);
    register_remove_last_char(ui);
    register_set_worker_max_hours(ui, state);
    register_set_worker_week_override(ui, state);
    register_set_bulk_week_limit(ui, state);
    register_set_worker_filter(ui, state);
    register_set_all_workers_filter(ui, state);
}

fn register_add_worker(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_add_worker(move |name| {
        if name.is_empty() {
            return;
        }
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            a.workers.add(&name);
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}

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

fn register_find_completion(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    PjmCallback::get(ui).on_find_completion(move |prefix, pipe| {
        let prefix: String = prefix.into();
        if prefix.is_empty() {
            return "".into();
        }
        let app = app.borrow();
        let matches: Vec<String> = app
            .workers
            .list()
            .into_iter()
            .filter(|(id, name)| id != &WORKER_ID_ZERO && name.starts_with(&prefix))
            .map(|(_, name)| name)
            .collect();
        if matches.is_empty() {
            "".into()
        } else if matches.len() == 1 {
            if pipe {
                format!("{}|", matches.into_iter().next().unwrap()).into()
            } else {
                matches.into_iter().next().unwrap().into()
            }
        } else {
            common_prefix(&matches).into()
        }
    });
}

fn apply_worker_filter(
    app: &App,
    visibility: &mut std::collections::HashMap<(i32, i32), bool>,
    worker_filter: &HashSet<String>,
) {
    if worker_filter.is_empty() {
        visibility.clear();
        return;
    }
    let worker_ids: Vec<WorkerId> = worker_filter
        .iter()
        .filter_map(|name| app.workers.get_id_by_name(name))
        .collect();
    for (pi, (proj_id, _)) in app.projects.list().iter().enumerate() {
        for dev_id in app.projects.list_devs(*proj_id) {
            let has_any = worker_ids.iter().any(|&wid| {
                app.projects
                    .get_single_dev(*proj_id, dev_id)
                    .map(|sd| sd.has_worker(wid))
                    .unwrap_or(false)
            });
            visibility.insert((pi as i32, dev_id.0 as i32), has_any);
        }
    }
}

fn register_search(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_search(move |_| {
        if let Some(ui) = ui_w.upgrade() {
            {
                live.worker_filter.borrow_mut().clear();
                visibility.borrow_mut().clear();
            }
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_filter_active(false);
        }
    });
}

fn register_set_worker_filter(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_worker_filter(move |worker_idx, selected| {
        if let Some(ui) = ui_w.upgrade() {
            {
                let a = app.borrow();
                let workers = a.workers.list();
                if let Some((_, name)) = workers.get(worker_idx as usize) {
                    let mut filter = live.worker_filter.borrow_mut();
                    if selected {
                        filter.insert(name.clone());
                    } else {
                        filter.remove(name.as_str());
                    }
                    apply_worker_filter(&a, &mut visibility.borrow_mut(), &filter);
                }
            }
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            let filter_active = !live.worker_filter.borrow().is_empty();
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_filter_active(filter_active);
        }
    });
}

fn register_set_all_workers_filter(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_all_workers_filter(move |selected| {
        if let Some(ui) = ui_w.upgrade() {
            {
                let a = app.borrow();
                let workers = a.workers.list();
                let mut filter = live.worker_filter.borrow_mut();
                filter.clear();
                if selected {
                    for (_, name) in &workers {
                        filter.insert(name.clone());
                    }
                }
                apply_worker_filter(&a, &mut visibility.borrow_mut(), &filter);
            }
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            let filter_active = !live.worker_filter.borrow().is_empty();
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_filter_active(filter_active);
        }
    });
}

fn register_remove_last_char(ui: &AppWindow) {
    PjmCallback::get(ui).on_remove_last_char(|s| {
        let s: String = s.into();
        let mut chars = s.chars();
        chars.next_back();
        chars.as_str().to_string().into()
    });
}

fn register_set_worker_max_hours(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_worker_max_hours(move |worker_idx, hours_text| {
        let hours: u32 = hours_text
            .parse::<u32>()
            .unwrap_or(DEFAULT_MAX_HOURS)
            .clamp(0, DEFAULT_MAX_HOURS);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            a.set_worker_max_hours_by_idx(worker_idx as usize, hours);
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}

fn register_set_worker_week_override(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_worker_week_override(move |worker_idx, week, hours_text| {
        let hours: u32 = hours_text
            .parse::<u32>()
            .unwrap_or(DEFAULT_MAX_HOURS)
            .clamp(0, DEFAULT_MAX_HOURS);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            a.set_worker_week_override_by_idx(worker_idx as usize, week as usize, hours);
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}

fn register_set_bulk_week_limit(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_bulk_week_limit(move |week, hours_text| {
        let hours: u32 = hours_text
            .parse::<u32>()
            .unwrap_or(DEFAULT_MAX_HOURS)
            .clamp(0, DEFAULT_MAX_HOURS);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            a.set_bulk_week_limit(week as usize, hours);
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}

fn register_add_dev(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_add_dev(move |name| {
        if name.is_empty() {
            return;
        }
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            a.devs.add(&name);
            refresh(
                &ui,
                &mut a,
                &live,
                &row_counts.borrow(),
                &visibility.borrow(),
            );
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}
