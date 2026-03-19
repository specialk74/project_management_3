use slint::{ComponentHandle, Global};

use crate::ui_sync::{refresh, sync_project_texts};
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_add_worker(ui, state);
    register_add_dev(ui, state);
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
            refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
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
            refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
            PjmCallback::get(&ui).set_changed(true);
        }
    });
}
