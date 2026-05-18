use slint::{ComponentHandle, Global, Model};

use crate::single_dev_utils::single_dev::WeekId;
use crate::single_effort_utils::sinlge_effort::Effort;
use crate::ui_sync::{refresh, sync_project_texts};
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_changed_effort(ui, state);
    register_set_dev_effort(ui, state);
    register_set_note(ui, state);
    register_set_dev_note(ui, state);
    register_editor_accept(ui, state);
}

fn register_changed_effort(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let ui_w = ui.as_weak();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    PjmCallback::get(ui).on_changed_effort(move |effort| {
        let mut a = app.borrow_mut();
        let projects = a.projects.list();
        let devs = a.devs.list();
        let Some(&(proj_id, _)) = projects.get(effort.project as usize) else {
            return;
        };
        let Some(&(dev_id, _)) = devs.get(effort.dev as usize) else {
            return;
        };
        let week = effort.week as usize;
        a.projects.reset_effort(proj_id, dev_id, WeekId(week));

        for person in effort.persons.iter() {
            let lista: Vec<&str> = person.name.split("|").collect();
            if lista.len() != 2 {
                continue;
            }
            if let Some(worker_id) = a.workers.get_id_by_name(lista[0]) {
                let e = lista[1].trim().parse::<usize>().unwrap_or(0);
                a.projects
                    .add_effort(proj_id, dev_id, WeekId(week), worker_id, Effort(e));
                if !person.note.is_empty() {
                    a.projects
                        .set_note(proj_id, dev_id, WeekId(week), worker_id, &person.note);
                }
            }
        }
        if let Some(ui) = ui_w.upgrade() {
            sync_project_texts(&ui, &mut a);
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

fn register_set_dev_effort(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_dev_effort(move |proj_idx, dev_idx, effort| {
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
            a.projects
                .add_dev_effort(proj_id, dev_id, Effort(effort as usize));
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

fn register_set_note(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_note(move |proj_idx, dev_idx, week, person_name_effort, note| {
        let mut a = app.borrow_mut();
        let projects = a.projects.list();
        let devs = a.devs.list();
        let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
            return;
        };
        let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else {
            return;
        };
        let lista: Vec<&str> = person_name_effort.split('|').collect();
        if lista.is_empty() {
            return;
        }
        let worker_name = lista[0];
        if let Some(worker_id) = a.workers.get_id_by_name(worker_name) {
            a.projects
                .set_note(proj_id, dev_id, WeekId(week as usize), worker_id, &note);
        }
        if let Some(ui) = ui_w.upgrade() {
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

fn register_editor_accept(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_editor_accept(move || {
        let Some(ui) = ui_w.upgrade() else { return };
        let proj_idx = PjmCallback::get(&ui).get_editor_project();
        let dev_idx = PjmCallback::get(&ui).get_editor_dev();
        let week = PjmCallback::get(&ui).get_editor_week() as usize;
        let row = PjmCallback::get(&ui).get_editor_row() as usize;
        let new_text = PjmCallback::get(&ui).get_editor_text().to_string();
        let effort = PjmCallback::get(&ui).get_editor_effort();

        PjmCallback::get(&ui).set_editor_active(false);

        let mut a = app.borrow_mut();
        let projects = a.projects.list();
        let devs = a.devs.list();
        let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else { return };
        let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else { return };
        let week_id = WeekId(week);

        // Build a modified persons list: same as stored effort data but with the
        // edited row's name replaced by the new text typed in the floating editor.
        let persons: Vec<(String, String)> = (0..effort.persons.row_count())
            .map(|i| {
                let p = effort.persons.row_data(i).unwrap_or_default();
                let name = if i == row {
                    new_text.clone()
                } else {
                    p.name.to_string()
                };
                (name, p.note.to_string())
            })
            .collect();

        // Same logic as changed_effort: reset the week then re-add all valid entries.
        a.projects.reset_effort(proj_id, dev_id, week_id);
        for (name, note) in &persons {
            let lista: Vec<&str> = name.split('|').collect();
            if lista.len() != 2 {
                continue;
            }
            if let Some(worker_id) = a.workers.get_id_by_name(lista[0]) {
                let e = lista[1].trim().parse::<usize>().unwrap_or(0);
                a.projects.add_effort(proj_id, dev_id, week_id, worker_id, Effort(e));
                if !note.is_empty() {
                    a.projects.set_note(proj_id, dev_id, week_id, worker_id, note);
                }
            }
        }

        sync_project_texts(&ui, &mut a);
        refresh(&ui, &mut a, &live, &row_counts.borrow(), &visibility.borrow());
        PjmCallback::get(&ui).set_changed(true);
    });
}

fn register_set_dev_note(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_set_dev_note(move |proj_idx, dev_idx, note| {
        let mut a = app.borrow_mut();
        let projects = a.projects.list();
        let devs = a.devs.list();
        let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
            return;
        };
        let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else {
            return;
        };
        a.projects.set_dev_note(proj_id, dev_id, &note);
        if let Some(ui) = ui_w.upgrade() {
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
