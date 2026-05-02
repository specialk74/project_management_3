use slint::{ComponentHandle, Global};

use crate::ui_sync::{refresh, sync_project_texts};
use crate::workers::worker::WORKER_ID_ZERO;
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
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
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

fn register_search(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_search(move |worker_name| {
        let worker_name: String = worker_name.into();
        if let Some(ui) = ui_w.upgrade() {
            {
                let a = app.borrow();
                let mut vis = visibility.borrow_mut();

                if worker_name.is_empty() {
                    vis.clear();
                } else {
                    let worker_id = a.workers.get_id_by_name(&worker_name);
                    for (pi, (proj_id, _)) in a.projects.list().iter().enumerate() {
                        for dev_id in a.projects.list_devs(*proj_id) {
                            let has_worker = match worker_id {
                                Some(wid) => a
                                    .projects
                                    .get_single_dev(*proj_id, dev_id)
                                    .map(|sd| sd.has_worker(wid))
                                    .unwrap_or(false),
                                None => false,
                            };
                            vis.insert((pi as i32, dev_id.0 as i32), has_worker);
                        }
                    }
                }
            }
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
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
        let hours: u32 = hours_text.parse::<u32>().unwrap_or(40).clamp(0, 40);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            a.set_worker_max_hours_by_idx(worker_idx as usize, hours);
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
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
        let hours: u32 = hours_text.parse::<u32>().unwrap_or(40).clamp(0, 40);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            a.set_worker_week_override_by_idx(worker_idx as usize, week as usize, hours);
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
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
        let hours: u32 = hours_text.parse::<u32>().unwrap_or(40).clamp(0, 40);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            a.set_bulk_week_limit(week as usize, hours);
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
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
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}
