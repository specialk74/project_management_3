use slint::{ModelRc, SharedString, VecModel};
use std::collections::HashMap;

use crate::app::App;
use crate::date_utils::dates::{days_to_local, get_default_weeks, primo_giorno_settimana_corrente};
use crate::single_dev::single_dev::{SingleDev, WeekId};
use crate::single_efforts::sinlge_effort::Effort;
use crate::workers::worker::{WORKER_ID_ZERO, WorkerId};
use crate::{
    DayData, DevInfo, EffortByDateData, EffortByDevData, EffortByPrjData, SingleEffortGui,
    SovraData,
};

// ── Utility ───────────────────────────────────────────────────────────────────

pub fn packed_to_color(packed: usize) -> slint::Color {
    slint::Color::from_rgb_u8(
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

pub fn mk<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

fn get_hours(effort: Effort) -> i32 {
    (effort.0 * 40 / 100) as i32
}

// ── Calcolo range settimane ───────────────────────────────────────────────────

pub fn build_weeks(start: usize, end: usize) -> Vec<DayData> {
    // println!("start: {}", start);
    // println!("end: {}", end);

    (start..=end)
        .step_by(7)
        //.inspect(|w| println!("week: {}", w)) // 👈 stampa ogni valore
        .map(|w| DayData {
            week: w as i32,
            text: SharedString::from(
                primo_giorno_settimana_corrente(&days_to_local(w as i32))
                    .format("%y-%m-%d")
                    .to_string(),
            ),
        })
        .collect()
}

// ── Costruzione dati Slint ────────────────────────────────────────────────────

pub fn build_project_data(
    app: &App,
    row_counts: &HashMap<(i32, i32), i32>,
    visibility: &HashMap<(i32, i32), bool>,
) -> Vec<EffortByPrjData> {
    let projects = app.projects.list();
    let (n_weeks, start_w, end_w) = get_default_weeks(Some(app.start_week.0 as i32));

    projects
        .iter()
        .enumerate()
        .map(|(pi, (proj_id, proj_name))| {
            let dev_data: Vec<EffortByDevData> = app
                .projects
                .list_devs(*proj_id)
                .iter()
                .map(|dev_id| {
                    let max = *row_counts.get(&(pi as i32, dev_id.0 as i32)).unwrap_or(&0);
                    let enable = *visibility
                        .get(&(pi as i32, dev_id.0 as i32))
                        .unwrap_or(&true)
                        & app.projects.get_enable(proj_id).0;
                    if let Some(sd) = app.projects.get_single_dev(*proj_id, *dev_id) {
                        build_dev(
                            app,
                            pi as i32,
                            dev_id.0 as i32,
                            sd,
                            start_w as usize,
                            end_w as usize,
                            enable,
                        )
                    } else {
                        empty_dev(
                            pi as i32,
                            dev_id.0 as i32,
                            (n_weeks * 7) as usize,
                            start_w as usize,
                            max,
                        )
                    }
                })
                .collect();

            let enable = app.projects.get_enable(proj_id).0;
            let project_visible = dev_data.is_empty() || dev_data.iter().any(|d| d.enable);
            EffortByPrjData {
                project_id: pi as i32,
                text: SharedString::from(proj_name.as_str()),
                start_week: start_w as i32,
                end_week: end_w as i32,
                visible: project_visible,
                enable,
                devs_of_the_project: mk(dev_data),
            }
        })
        .collect()
}

fn build_dev(
    app: &App,
    proj_idx: i32,
    dev_idx: i32,
    sd: &SingleDev,
    start_w: usize,
    end_w: usize,
    enable: bool,
) -> EffortByDevData {
    let planned = sd.planned_effort().0 as i32;
    let total = get_hours(sd.get_effort_tot());
    let max = (sd.max_num_efforts() as i32).max(1);

    let week_data: Vec<EffortByDateData> = (start_w..=end_w)
        .step_by(7)
        //.inspect(|f| println!("build_dev-{}: {}", proj_idx, f))
        .map(|w| {
            let workers_in_week: Vec<SingleEffortGui> = {
                let mut v = sd
                    .get_all(WeekId(w))
                    .map(|s| {
                        s.worker_id
                            .iter()
                            .filter(|(worker_id, _)| **worker_id != WORKER_ID_ZERO)
                            .map(|(worker_id, single_effort)| {
                                let sovra_effort = app
                                    .sovra
                                    .get(&(WeekId(w), *worker_id))
                                    .map_or(0, |e| get_hours(*e));
                                SingleEffortGui {
                                    name: SharedString::from(format!(
                                        "{}|{}",
                                        app.workers.get_name_by_id(*worker_id),
                                        single_effort.get_effort().0
                                    )),
                                    note: SharedString::from(single_effort.get_note()),
                                    week: w as i32,
                                    dev: dev_idx,
                                    project: proj_idx,
                                    effort: single_effort.get_effort().0 as i32,
                                    sovra: sovra_effort,
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                v.resize(max as usize, SingleEffortGui::default());
                v
            };

            let week_total = get_hours(sd.get_effort_by_week(WeekId(w)));

            EffortByDateData {
                total: week_total,
                remains: planned - week_total,
                dev: dev_idx,
                project: proj_idx,
                effort: planned,
                week: w as i32,
                persons: mk(workers_in_week),
            }
        })
        .collect();

    EffortByDevData {
        project: proj_idx,
        dev: dev_idx,
        total,
        effort: planned,
        remains: planned - total,
        visible: enable,
        enable,
        max,
        note: SharedString::from(sd.get_note()),
        datas: mk(week_data),
    }
}

fn empty_dev(
    proj_idx: i32,
    dev_idx: i32,
    n_weeks: usize,
    start_w: usize,
    max: i32,
) -> EffortByDevData {
    let n_persons = max.max(0) as usize;
    let week_data: Vec<EffortByDateData> = (0..n_weeks)
        .step_by(7)
        //.inspect(|f| println!("empty: {}", f))
        .map(|i| EffortByDateData {
            total: 0,
            remains: 0,
            dev: dev_idx,
            project: proj_idx,
            effort: 0,
            week: (start_w + i) as i32,
            persons: mk(vec![SingleEffortGui::default(); n_persons]),
        })
        .collect();

    EffortByDevData {
        project: proj_idx,
        dev: dev_idx,
        total: 0,
        effort: 0,
        remains: 0,
        visible: true,
        enable: true,
        note: SharedString::from(""),
        max: (max - 1).max(0),
        datas: mk(week_data),
    }
}

struct SovraByWeekAndWorker {
    by_week: HashMap<(WeekId, WorkerId), Effort>,
}

pub fn build_sovra_data(app: &App) -> Vec<SovraData> {
    let workers = app.workers.list();
    let projects = app.projects.list();
    let devs = app.devs.list();
    let start_w = app.start_week.0;
    let end_w = app.end_week.0;

    (start_w..=end_w)
        .step_by(7)
        .map(|w| {
            let values: Vec<i32> = workers
                .iter()
                .map(|(wid, _)| {
                    let total_h: usize = projects
                        .iter()
                        .flat_map(|(pid, _)| {
                            devs.iter().map(move |(did, _)| {
                                app.projects
                                    .get_single_dev(*pid, *did)
                                    .map_or(0, |sd| sd.get_effort(WeekId(w), *wid).0)
                            })
                        })
                        .sum();
                    get_hours(Effort(total_h))
                })
                .collect();
            SovraData {
                week: w as i32,
                value: mk(values),
            }
        })
        .collect()
}

pub fn build_dev_infos(app: &App) -> Vec<DevInfo> {
    app.devs
        .list_full()
        .into_iter()
        .map(|(_, name, bg, font)| DevInfo {
            title: SharedString::from(name.as_str()),
            col: packed_to_color(bg),
            text_color: packed_to_color(font),
        })
        .collect()
}
