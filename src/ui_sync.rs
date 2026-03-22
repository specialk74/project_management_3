#![allow(unused)]
#![allow(dead_code)]

use slint::{Model, SharedString};
use std::collections::HashMap;

use crate::AppWindow;
use crate::app::App;
use crate::builders::{
    build_dev_infos, build_project_data, build_sovra_data, build_weeks, find_end_week,
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
    _ui: &AppWindow,
    app: &App,
    live: &LiveModels,
    row_counts: &HashMap<(i32, i32), i32>,
    visibility: &HashMap<(i32, i32), bool>,
) {
    let end_w = find_end_week(app);

    live.projects
        .set_vec(build_project_data(app, row_counts, visibility));
    live.worker_names.set_vec({
        app.workers
            .list()
            .iter()
            .map(|(_, n)| SharedString::from(n.as_str()))
            .collect::<Vec<_>>()
    });
    live.sovra.set_vec(build_sovra_data(app));
    live.weeks.set_vec(build_weeks(app.start_week, end_w));
    live.devs.set_vec(build_dev_infos(app));
}
