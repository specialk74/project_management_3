use slint::{ComponentHandle, Global};

use crate::date_utils::dates::parse_date_str;
use crate::single_dev::single_dev::WeekId;
use crate::ui_sync::{refresh, sync_project_texts};
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_new_project(ui, state);
    register_set_project_name(ui, state);
    register_add_dev_to_project(ui, state);
    register_set_project_end_week(ui, state);
}

fn register_new_project(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_new_project(move |name, start_date| {
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            let start_week = parse_date_str(&start_date).map(|d| WeekId(d as usize));
            let proj_name = if name.is_empty() { "Nuovo Progetto" } else { &name };
            a.projects.add(proj_name, start_week);
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}

fn register_set_project_end_week(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_project_end_week(move |proj_idx, date_str| {
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            let projects = a.projects.list();
            let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                return;
            };
            let end_week = if date_str.is_empty() {
                None
            } else {
                parse_date_str(&date_str).map(|d| WeekId(d as usize))
            };
            a.projects.set_project_end_week(proj_id, end_week);
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}

fn register_set_project_name(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_project_name(move |proj_idx, name| {
        if let Some(_ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            let projects = a.projects.list();
            let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                return;
            };
            a.projects.set_project_info(proj_id, name.as_str());
            PjmCallback::get(&_ui).set_changed(true);
        }
    });
}

fn register_add_dev_to_project(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_add_dev_to_project(move |proj_idx, dev_idx, add| {
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            let projects = a.projects.list();
            let devs = a.devs.list();
            let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                return;
            };
            let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else {
                return;
            };
            sync_project_texts(&ui, &mut a);
            if add {
                a.projects.add_dev(proj_id, dev_id);
            } else {
                a.projects.del_dev(proj_id, dev_id);
            }
            refresh(&ui, &mut *a, &live, &row_counts.borrow(), &visibility.borrow());
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}
