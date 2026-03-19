use slint::{ComponentHandle, Global};

use crate::devs::DevId;
use crate::projects::ProjectId;
use crate::sinlge_effort::Effort;
use crate::ui_sync::{refresh, sync_project_texts};
use crate::workers::WorkerId;
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_add_row(ui, state);
    register_del_row(ui, state);
    register_hide_dev(ui, state);
}

fn register_add_row(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_add_row(move |proj_idx, dev_idx| {
        let proj_id = ProjectId(proj_idx as usize);
        let dev_id = DevId(dev_idx as usize);
        {
            let mut a = app.borrow_mut();
            if let Some(week) = a.projects.get_week_with_max_worker(proj_id, dev_id) {
                a.projects
                    .add_effort(proj_id, dev_id, week, WorkerId(0), Effort(0));
            }
        }
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
        }
    });
}

fn register_del_row(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_del_row(move |proj_idx, dev_idx| {
        {
            let mut rc = row_counts.borrow_mut();
            let entry = rc.entry((proj_idx, dev_idx)).or_insert(0);
            if *entry > 0 {
                *entry -= 1;
            }
        }
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
        }
    });
}

fn register_hide_dev(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_hide_dev(move |proj_idx, dev_idx, new_enable| {
        visibility
            .borrow_mut()
            .insert((proj_idx, dev_idx), new_enable);
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
        }
    });
}
