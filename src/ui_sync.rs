#![allow(unused)]
#![allow(dead_code)]

use slint::{Global, Model, SharedString};
use std::collections::HashMap;

use crate::AppWindow;
use crate::PjmCallback;
use crate::app::App;
use crate::builders::{
    build_available_years, build_category_list, build_dev_infos, build_dev_year_totals,
    build_project_data, build_sovra_data, build_weeks, build_worker_max_hours,
};
use crate::live_models::LiveModels;

pub fn sync_project_texts(ui: &AppWindow, app: &mut App) {
    let efforts = ui.get_app_project();
    let projects = app.projects.list();
    for (idx, (proj_id, _)) in projects.iter().enumerate() {
        if let Some(item) = efforts.projects.row_data(idx) {
            app.projects.set_project_info(*proj_id, &item.text);
        }
    }
}

pub fn refresh(
    ui: &AppWindow,
    app: &mut App,
    live: &LiveModels,
    row_counts: &HashMap<(i32, i32), i32>,
    visibility: &HashMap<(i32, i32), bool>,
) {
    app.recompute_week_range();
    app.compute_sovra();
    live.projects
        .set_vec(build_project_data(app, row_counts, visibility, &live.worker_filter.borrow()));
    live.worker_names.set_vec({
        app.workers
            .list()
            .iter()
            .map(|(_, n)| SharedString::from(n.as_str()))
            .collect::<Vec<_>>()
    });
    live.worker_max_hours.set_vec(build_worker_max_hours(app));
    live.worker_footer_visible.set_vec(
        app.workers
            .list()
            .iter()
            .map(|(id, _)| !app.workers.is_hidden_in_footer(*id))
            .collect::<Vec<_>>(),
    );
    live.sovra.set_vec(build_sovra_data(app));
    live.weeks
        .set_vec(build_weeks(app.start_week.0, app.end_week.0, app));
    live.devs.set_vec(build_dev_infos(app));
    live.years.set_vec(build_available_years(app));
    live.categories.set_vec(build_category_list(app));
    let selected_year = PjmCallback::get(ui).get_selected_year();
    let selected_category_id = PjmCallback::get(ui).get_selected_category_id();
    live.dev_year_totals.set_vec(build_dev_year_totals(app, selected_year, selected_category_id));

    let projects = app.projects.list_full();
    let all_enabled = !projects.is_empty() && projects.iter().all(|(_, _, e)| e.0);
    let all_disabled = !projects.is_empty() && projects.iter().all(|(_, _, e)| !e.0);

    let workers = app.workers.list();
    let filter = live.worker_filter.borrow();
    // When the visibility map is empty no filter is active: show every worker as selected.
    // When a filter has been applied, reflect the actual filter membership.
    let filter_selected: Vec<bool> = if visibility.is_empty() {
        workers.iter().map(|_| true).collect()
    } else {
        workers
            .iter()
            .map(|(_, name)| filter.contains(name.as_str()))
            .collect()
    };
    let all_workers_on = !workers.is_empty() && filter_selected.iter().all(|&b| b);
    let all_workers_off = !workers.is_empty() && !visibility.is_empty() && filter_selected.iter().all(|&b| !b);
    live.worker_filter_selected.set_vec(filter_selected);

    let pcb = PjmCallback::get(ui);
    pcb.set_all_projects_enabled(all_enabled);
    pcb.set_all_projects_disabled(all_disabled);
    pcb.set_all_workers_filter_on(all_workers_on);
    pcb.set_all_workers_filter_off(all_workers_off);
}
