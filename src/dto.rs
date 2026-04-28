use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::date_utils::dates::{days_to_local, get_default_weeks, primo_giorno_settimana_corrente};
use crate::single_dev::single_dev::WeekId;
use crate::single_efforts::sinlge_effort::Effort;
use crate::workers::worker::WORKER_ID_ZERO;

fn get_hours(effort: Effort) -> i32 {
    (effort.0 * 40 / 100) as i32
}

fn packed_to_hex(packed: usize) -> String {
    format!("#{:06X}", packed)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeekDto {
    pub week: usize,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CellDto {
    pub worker_name: String,
    pub effort_pct: i32,
    pub hours: i32,
    pub sovra_hours: i32,
    pub max_hours: i32,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeekCellsDto {
    pub week: usize,
    pub total_hours: i32,
    pub cells: Vec<CellDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DevDataDto {
    pub dev_id: usize,
    pub dev_name: String,
    pub dev_note: String,
    pub planned_hours: i32,
    pub total_hours: i32,
    pub enabled: bool,
    pub weeks: Vec<WeekCellsDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectDto {
    pub idx: usize,
    pub name: String,
    pub enabled: bool,
    pub dev_data: Vec<DevDataDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkerDto {
    pub idx: usize,
    pub name: String,
    pub max_hours: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DevDto {
    pub id: usize,
    pub name: String,
    pub bg_color: String,
    pub text_color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SovraWorkerDto {
    pub worker_idx: usize,
    pub name: String,
    pub hours: i32,
    pub max_hours: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SovraWeekDto {
    pub week: usize,
    pub workers: Vec<SovraWorkerDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStateDto {
    pub weeks: Vec<WeekDto>,
    pub projects: Vec<ProjectDto>,
    pub workers: Vec<WorkerDto>,
    pub devs: Vec<DevDto>,
    pub sovra: Vec<SovraWeekDto>,
    pub this_week: usize,
    pub current_file: String,
    pub changed: bool,
}

pub fn build_dto(
    app: &App,
    filter: &str,
    current_file: &str,
    changed: bool,
    this_week: usize,
) -> AppStateDto {
    let (_, start_w, end_w) = get_default_weeks(Some(app.start_week.0 as i32));
    let start_w = start_w as usize;
    let end_w = end_w as usize;

    let weeks: Vec<WeekDto> = (start_w..=end_w)
        .step_by(7)
        .map(|w| WeekDto {
            week: w,
            label: primo_giorno_settimana_corrente(&days_to_local(w as i32))
                .format("%y-%m-%d")
                .to_string(),
        })
        .collect();

    let workers_list: Vec<_> = app.workers.list();
    let devs_list = app.devs.list();
    let projects_list = app.projects.list();

    let projects_dto: Vec<ProjectDto> = projects_list
        .iter()
        .enumerate()
        .map(|(pi, (proj_id, proj_name))| {
            let proj_enabled = app.projects.get_enable(proj_id).0;

            let dev_data: Vec<DevDataDto> = app
                .projects
                .list_devs(*proj_id)
                .iter()
                .map(|dev_id| {
                    let dev_name = devs_list
                        .iter()
                        .find(|(id, _)| id == dev_id)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_default();

                    let dev_enabled = if filter.is_empty() {
                        true
                    } else {
                        app.workers
                            .get_id_by_name(filter)
                            .and_then(|wid| {
                                app.projects
                                    .get_single_dev(*proj_id, *dev_id)
                                    .map(|sd| sd.has_worker(wid))
                            })
                            .unwrap_or(false)
                    };

                    let sd = app.projects.get_single_dev(*proj_id, *dev_id);
                    let planned_hours = sd.map_or(0, |s| get_hours(s.planned_effort()));
                    let total_hours = sd.map_or(0, |s| get_hours(s.get_effort_tot()));
                    let dev_note = sd.map_or(String::new(), |s| s.get_note());
                    let num_rows = sd.map_or(1, |s| s.max_num_efforts().max(1));

                    let week_cells: Vec<WeekCellsDto> = (start_w..=end_w)
                        .step_by(7)
                        .map(|w| {
                            let week_total =
                                sd.map_or(0, |s| get_hours(s.get_effort_by_week(WeekId(w))));

                            let mut cells: Vec<CellDto> = sd
                                .and_then(|s| s.get_all(WeekId(w)))
                                .map(|sew| {
                                    sew.worker_id
                                        .iter()
                                        .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                                        .map(|(worker_id, single_effort)| {
                                            let sovra_hours = app
                                                .sovra
                                                .get(&(WeekId(w), *worker_id))
                                                .map_or(0, |e| get_hours(*e));
                                            let max_hours = app
                                                .workers
                                                .get_effective_max_hours(*worker_id, w)
                                                as i32;
                                            CellDto {
                                                worker_name: app
                                                    .workers
                                                    .get_name_by_id(*worker_id)
                                                    .to_string(),
                                                effort_pct: single_effort.get_effort().0 as i32,
                                                hours: get_hours(single_effort.get_effort()),
                                                sovra_hours,
                                                max_hours,
                                                note: single_effort.get_note(),
                                            }
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();

                            cells.resize(num_rows, CellDto::default());

                            WeekCellsDto {
                                week: w,
                                total_hours: week_total,
                                cells,
                            }
                        })
                        .collect();

                    DevDataDto {
                        dev_id: dev_id.0,
                        dev_name,
                        dev_note,
                        planned_hours,
                        total_hours,
                        enabled: dev_enabled && proj_enabled,
                        weeks: week_cells,
                    }
                })
                .collect();

            ProjectDto {
                idx: pi,
                name: proj_name.clone(),
                enabled: proj_enabled,
                dev_data,
            }
        })
        .collect();

    let workers_dto: Vec<WorkerDto> = workers_list
        .iter()
        .enumerate()
        .map(|(idx, (id, name))| WorkerDto {
            idx,
            name: name.clone(),
            max_hours: app.workers.get_max_hours(*id) as i32,
        })
        .collect();

    let devs_dto: Vec<DevDto> = app
        .devs
        .list_full()
        .into_iter()
        .map(|(id, name, bg, font)| DevDto {
            id: id.0,
            name,
            bg_color: packed_to_hex(bg),
            text_color: packed_to_hex(font),
        })
        .collect();

    let sovra_dto: Vec<SovraWeekDto> = (start_w..=end_w)
        .step_by(7)
        .map(|w| {
            let worker_data: Vec<SovraWorkerDto> = workers_list
                .iter()
                .enumerate()
                .map(|(idx, (wid, name))| {
                    let total_h: usize = projects_list
                        .iter()
                        .flat_map(|(pid, _)| {
                            devs_list.iter().map(move |(did, _)| {
                                app.projects
                                    .get_single_dev(*pid, *did)
                                    .map_or(0, |sd| sd.get_effort(WeekId(w), *wid).0)
                            })
                        })
                        .sum();
                    SovraWorkerDto {
                        worker_idx: idx,
                        name: name.clone(),
                        hours: get_hours(Effort(total_h)),
                        max_hours: app.workers.get_effective_max_hours(*wid, w) as i32,
                    }
                })
                .collect();

            SovraWeekDto {
                week: w,
                workers: worker_data,
            }
        })
        .collect();

    AppStateDto {
        weeks,
        projects: projects_dto,
        workers: workers_dto,
        devs: devs_dto,
        sovra: sovra_dto,
        this_week,
        current_file: current_file.to_string(),
        changed,
    }
}
