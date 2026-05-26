use chrono::Datelike;
use slint::{ModelRc, SharedString, VecModel};
use std::collections::HashMap;

use crate::app::App;
use crate::date_utils::dates::{days_to_local, primo_giorno_settimana_corrente};
use crate::single_dev_utils::single_dev::{SingleDev, WeekId};
use crate::single_effort_utils::sinlge_effort::Effort;
use crate::workers_utils::worker::WORKER_ID_ZERO;
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
    effort.0 as i32
}

// ── Calcolo range settimane ───────────────────────────────────────────────────

pub fn build_weeks(start: usize, end: usize, app: &App) -> Vec<DayData> {
    (start..=end)
        .step_by(7)
        .map(|w| DayData {
            week: w as i32,
            text: SharedString::from(
                primo_giorno_settimana_corrente(&days_to_local(w as i32))
                    .format("%y-%m-%d")
                    .to_string(),
            ),
            all_limited: app.workers.all_workers_have_override(w),
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
    let start_w = app.start_week.0;
    let end_w = app.end_week.0;
    let n_weeks = (end_w - start_w) / 7;

    projects
        .iter()
        .enumerate()
        .map(|(pi, (proj_id, proj_name))| {
            let deadline_week = app
                .projects
                .get_project_end_week(*proj_id)
                .map(|w| w.0 as i32)
                .unwrap_or(-1);

            let deadline_text = if deadline_week >= 0 {
                SharedString::from(
                    primo_giorno_settimana_corrente(&days_to_local(deadline_week))
                        .format("%y-%m-%d")
                        .to_string(),
                )
            } else {
                SharedString::default()
            };

            let proj_start = app
                .projects
                .get_project_start_week(*proj_id)
                .map(|w| w.0 as i32)
                .unwrap_or(-1);

            let start_text = if proj_start >= 0 {
                SharedString::from(
                    primo_giorno_settimana_corrente(&days_to_local(proj_start))
                        .format("%y-%m-%d")
                        .to_string(),
                )
            } else {
                SharedString::default()
            };

            let enable = app.projects.get_enable(proj_id).0;

            let dev_data: Vec<EffortByDevData> = if !enable {
                vec![]
            } else {
                app.projects
                    .list_devs(*proj_id)
                    .iter()
                    .map(|dev_id| {
                        let max = *row_counts.get(&(pi as i32, dev_id.0 as i32)).unwrap_or(&0);
                        let dev_enable = *visibility
                            .get(&(pi as i32, dev_id.0 as i32))
                            .unwrap_or(&true);
                        if !dev_enable {
                            if let Some(sd) = app.projects.get_single_dev(*proj_id, *dev_id) {
                                build_dev_hidden(app, pi as i32, dev_id.0 as i32, sd, start_w, end_w, proj_start, deadline_week)
                            } else {
                                empty_dev_hidden(pi as i32, dev_id.0 as i32, n_weeks * 7, start_w, max, proj_start, deadline_week)
                            }
                        } else if let Some(sd) = app.projects.get_single_dev(*proj_id, *dev_id) {
                            build_dev(
                                app,
                                pi as i32,
                                dev_id.0 as i32,
                                sd,
                                start_w,
                                end_w,
                                true,
                                proj_start,
                                deadline_week,
                            )
                        } else {
                            empty_dev(
                                pi as i32,
                                dev_id.0 as i32,
                                n_weeks * 7,
                                start_w,
                                max,
                                proj_start,
                                deadline_week,
                            )
                        }
                    })
                    .collect()
            };
            let project_visible = dev_data.is_empty() || dev_data.iter().any(|d| d.enable);

            let all_devs = app.devs.list();
            let project_dev_ids = app.projects.list_devs(*proj_id);
            let dev_in_project: Vec<bool> = all_devs
                .iter()
                .map(|(dev_id, _)| project_dev_ids.contains(dev_id))
                .collect();
            let dev_has_data: Vec<bool> = all_devs
                .iter()
                .map(|(dev_id, _)| {
                    app.projects
                        .get_single_dev(*proj_id, *dev_id)
                        .map(|sd| sd.planned_effort().0 > 0 || !sd.get_weeks().is_empty())
                        .unwrap_or(false)
                })
                .collect();

            EffortByPrjData {
                project_id: pi as i32,
                text: SharedString::from(proj_name.as_str()),
                tripletta: SharedString::from(app.projects.get_tripletta(*proj_id).as_str()),
                start_week: proj_start,
                start_text,
                end_week: deadline_week,
                deadline_text,
                visible: project_visible,
                enable,
                devs_of_the_project: mk(dev_data),
                dev_in_project: mk(dev_in_project),
                dev_has_data: mk(dev_has_data),
            }
        })
        .collect()
}

fn activity_range(sd: &SingleDev) -> (i32, i32) {
    let weeks = sd.get_weeks();
    let start = weeks.iter().map(|w| w.0 as i32).min().unwrap_or(-1);
    let end = weeks.iter().map(|w| w.0 as i32).max().unwrap_or(-1);
    (start, end)
}

fn build_dev(
    app: &App,
    proj_idx: i32,
    dev_idx: i32,
    sd: &SingleDev,
    start_w: usize,
    end_w: usize,
    enable: bool,
    proj_start: i32,
    deadline_week: i32,
) -> EffortByDevData {
    let planned = sd.planned_effort().0 as i32;
    let total = get_hours(sd.get_effort_tot());
    let max = (sd.max_num_efforts() as i32).max(1);
    let (activity_start, activity_end) = activity_range(sd);

    let mut week_data: Vec<EffortByDateData> = (start_w..=end_w)
        .step_by(7)
        //.inspect(|f| println!("build_dev-{}: {}", proj_idx, f))
        .map(|w| {
            let post_deadline = deadline_week >= 0 && w as i32 > deadline_week;
            let pre_start = proj_start >= 0 && (w as i32) < proj_start;
            let (workers_in_week, has_workers) = if post_deadline || pre_start {
                (vec![], false)
            } else {
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
                                let max_hours =
                                    app.workers.get_effective_max_hours(*worker_id, w) as i32;
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
                                    max_hours,
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let has = !v.is_empty();
                v.resize(max as usize, SingleEffortGui::default());
                (v, has)
            };

            let week_total = get_hours(sd.get_effort_by_week(WeekId(w)));

            EffortByDateData {
                total: week_total,
                cumulative: 0,
                remains: planned - week_total,
                dev: dev_idx,
                project: proj_idx,
                effort: planned,
                week: w as i32,
                has_workers,
                persons: mk(workers_in_week),
            }
        })
        .collect();

    let mut running = 0i32;
    for wd in &mut week_data {
        running += wd.total;
        wd.cumulative = running;
    }

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
        start_week: proj_start,
        deadline_week,
        hide_effort: sd.get_hide_effort(),
        activity_start,
        activity_end,
        datas: mk(week_data),
    }
}

fn empty_dev(
    proj_idx: i32,
    dev_idx: i32,
    n_weeks: usize,
    start_w: usize,
    max: i32,
    proj_start: i32,
    deadline_week: i32,
) -> EffortByDevData {
    let n_persons = max.max(0) as usize;
    let week_data: Vec<EffortByDateData> = (0..n_weeks)
        .step_by(7)
        //.inspect(|f| println!("empty: {}", f))
        .map(|i| {
            let w = start_w + i;
            let post_deadline = deadline_week >= 0 && w as i32 > deadline_week;
            let pre_start = proj_start >= 0 && (w as i32) < proj_start;
            EffortByDateData {
                total: 0,
                cumulative: 0,
                remains: 0,
                dev: dev_idx,
                project: proj_idx,
                effort: 0,
                week: w as i32,
                has_workers: false,
                persons: mk(if post_deadline || pre_start {
                    vec![]
                } else {
                    vec![SingleEffortGui::default(); n_persons]
                }),
            }
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
        start_week: proj_start,
        deadline_week,
        hide_effort: false,
        activity_start: -1,
        activity_end: -1,
        datas: mk(week_data),
    }
}

// Dev filtrato dalla ricerca: mantiene la struttura delle settimane (per l'allineamento
// con la colonna sinistra) ma con persons vuote — zero Cell-RW istanziati.
fn build_dev_hidden(
    _app: &App,
    proj_idx: i32,
    dev_idx: i32,
    sd: &SingleDev,
    start_w: usize,
    end_w: usize,
    proj_start: i32,
    deadline_week: i32,
) -> EffortByDevData {
    let planned = sd.planned_effort().0 as i32;
    let total = get_hours(sd.get_effort_tot());
    let max = (sd.max_num_efforts() as i32).max(1);
    let week_data: Vec<EffortByDateData> = (start_w..=end_w)
        .step_by(7)
        .map(|w| EffortByDateData {
            total: 0,
            cumulative: 0,
            remains: 0,
            dev: dev_idx,
            project: proj_idx,
            effort: planned,
            week: w as i32,
            has_workers: false,
            persons: mk(vec![]),
        })
        .collect();
    EffortByDevData {
        project: proj_idx,
        dev: dev_idx,
        total,
        effort: planned,
        remains: planned - total,
        visible: false,
        enable: false,
        max,
        note: SharedString::from(sd.get_note()),
        start_week: proj_start,
        deadline_week,
        hide_effort: sd.get_hide_effort(),
        activity_start: -1,
        activity_end: -1,
        datas: mk(week_data),
    }
}

fn empty_dev_hidden(
    proj_idx: i32,
    dev_idx: i32,
    n_weeks: usize,
    start_w: usize,
    max: i32,
    proj_start: i32,
    deadline_week: i32,
) -> EffortByDevData {
    let week_data: Vec<EffortByDateData> = (0..n_weeks)
        .step_by(7)
        .map(|i| EffortByDateData {
            total: 0,
            cumulative: 0,
            remains: 0,
            dev: dev_idx,
            project: proj_idx,
            effort: 0,
            week: (start_w + i) as i32,
            has_workers: false,
            persons: mk(vec![]),
        })
        .collect();
    EffortByDevData {
        project: proj_idx,
        dev: dev_idx,
        total: 0,
        effort: 0,
        remains: 0,
        visible: false,
        enable: false,
        max: (max - 1).max(0),
        note: SharedString::default(),
        start_week: proj_start,
        deadline_week,
        hide_effort: false,
        activity_start: -1,
        activity_end: -1,
        datas: mk(week_data),
    }
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
            let max_hours: Vec<i32> = workers
                .iter()
                .map(|(wid, _)| app.workers.get_effective_max_hours(*wid, w) as i32)
                .collect();
            SovraData {
                week: w as i32,
                value: mk(values),
                max_hours: mk(max_hours),
            }
        })
        .collect()
}

pub fn build_worker_max_hours(app: &App) -> Vec<i32> {
    app.workers
        .list()
        .iter()
        .map(|(id, _)| app.workers.get_max_hours(*id) as i32)
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

pub fn build_available_years(app: &App) -> Vec<i32> {
    use std::collections::BTreeSet;
    let mut years = BTreeSet::new();
    for (proj_id, _) in app.projects.list() {
        if let Some(w) = app.projects.get_project_start_week(proj_id) {
            years.insert(days_to_local(w.0 as i32).year());
        }
        if let Some(w) = app.projects.get_project_end_week(proj_id) {
            years.insert(days_to_local(w.0 as i32).year());
        }
    }
    years.into_iter().collect()
}

pub fn build_dev_year_totals(app: &App, year: i32) -> Vec<i32> {
    let devs = app.devs.list();
    let projects = app.projects.list();
    devs.iter()
        .map(|(dev_id, _)| {
            if year == 0 {
                return 0;
            }
            projects
                .iter()
                .map(|(proj_id, _)| {
                    app.projects
                        .get_single_dev(*proj_id, *dev_id)
                        .map(|sd| {
                            sd.get_weeks()
                                .iter()
                                .filter(|w| days_to_local(w.0 as i32).year() == year)
                                .map(|w| sd.get_effort_by_week(*w).0 as i32)
                                .sum::<i32>()
                        })
                        .unwrap_or(0)
                })
                .sum()
        })
        .collect()
}
