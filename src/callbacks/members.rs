use slint::{ComponentHandle, Global, SharedString};

use crate::ui_sync::{refresh, sync_project_texts};
use crate::workers::WORKER_ID_ZERO;
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_add_worker(ui, state);
    register_add_dev(ui, state);
    register_find_completion(ui, state);
    register_remove_last_char(ui);
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

fn register_remove_last_char(ui: &AppWindow) {
    PjmCallback::get(ui).on_remove_last_char(|s| {
        let s: String = s.into();
        let mut chars = s.chars();
        chars.next_back();
        chars.as_str().to_string().into()
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
