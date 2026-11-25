//! Finestra «Scambia worker» (Aggiungi ▸ Scambia worker…): a sinistra il
//! worker da sostituire, al centro quello che prende il suo posto (uno solo
//! per parte: scelto uno, gli altri si disattivano), a destra i progetti in
//! cui il primo ha effort e, sotto ognuno, i suoi dev (con Select All), in
//! basso da quando. «OK» manda
//! `Action::SwapWorker`, che sposta le celle con le stesse ore
//! (`swap_worker`).

use super::*;

/// Larghezza delle colonne dei worker e di quella dei progetti (px).
const SWAP_WORKER_W: f32 = 220.0;
const SWAP_PROJECT_W: f32 = 300.0;
/// Altezza massima degli elenchi (px), poi si scorre.
const SWAP_LIST_H: f32 = 320.0;
/// Spazio fra le colonne (px). Non un separatore verticale: in una finestra
/// che si adatta al contenuto prenderebbe tutta l'altezza disponibile e la
/// farebbe crescere a ogni frame.
const SWAP_GAP: f32 = 16.0;

/// Celle in cui `to` ha già una cella dove `from` ne ha una (nelle coppie
/// progetto × dev `targets`, dalla settimana `start`): lì le ore si sommano.
pub(crate) fn swap_conflicts(
    app: &App,
    from: WorkerId,
    to: WorkerId,
    targets: &[(ProjectId, DevId)],
    start: WeekId,
) -> usize {
    targets
        .iter()
        .map(|&(proj, dev)| {
            app.projects.get_single_dev(proj, dev).map_or(0, |sd| {
                sd.get_weeks()
                    .into_iter()
                    .filter(|w| *w >= start)
                    .filter(|w| {
                        cell_effort(app, proj, dev, *w, from).is_some() && cell_effort(app, proj, dev, *w, to).is_some()
                    })
                    .count()
            })
        })
        .sum()
}

/// Spunta (`on`) o toglie tutte le coppie `items`.
fn set_ticks(ticked: &mut HashSet<(ProjectId, DevId)>, items: &[(ProjectId, DevId)], on: bool) {
    for t in items {
        if on {
            ticked.insert(*t);
        } else {
            ticked.remove(t);
        }
    }
}

/// Colonna di worker a scelta singola: ricerca e una casella per worker;
/// scelto uno, gli altri si disattivano (togliendo la spunta si riattivano).
/// `blocked` = worker non selezionabile (quello scelto nell'altra colonna).
#[allow(clippy::too_many_arguments)]
fn single_worker_column(
    ui: &mut egui::Ui,
    salt: &str,
    title: &str,
    workers: &[(WorkerId, String)],
    search: &mut String,
    chosen: &mut Option<WorkerId>,
    blocked: Option<WorkerId>,
    enabled: bool,
) {
    ui.vertical(|ui| {
        ui.set_width(SWAP_WORKER_W);
        ui.strong(title);
        ui.add_enabled(
            enabled,
            egui::TextEdit::singleline(search).hint_text("Cerca worker…").desired_width(SWAP_WORKER_W - 8.0),
        );
        let q = search.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt(salt)
            .scroll_source(drag_scroll())
            .max_height(SWAP_LIST_H)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let listed: Vec<&(WorkerId, String)> = workers.iter().filter(|(_, n)| search_matches(&q, n)).collect();
                if listed.is_empty() {
                    ui.label("(nessun worker)");
                }
                for (id, name) in listed {
                    let mut on = *chosen == Some(*id);
                    let free = chosen.is_none() || on;
                    let ok = enabled && free && blocked != Some(*id);
                    if ui.add_enabled(ok, egui::Checkbox::new(&mut on, name)).changed() {
                        *chosen = on.then_some(*id);
                    }
                }
            });
    });
}

pub(crate) fn swap_worker_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(sw) = state.swap_worker.as_mut() else {
        return;
    };
    // In entrambe le colonne solo i worker visibili («Elenchi» accesa).
    let listed: Vec<(WorkerId, String)> =
        app.workers.list().into_iter().filter(|(id, _)| app.workers.is_shown_in_find(*id)).collect();
    // Una scelta tolta dagli elenchi nel frattempo non vale più.
    if sw.from.is_some_and(|f| !listed.iter().any(|(id, _)| *id == f)) {
        sw.from = None;
    }
    if sw.to.is_some() && sw.to == sw.from {
        sw.to = None;
    }
    let start = sw.start();
    let projects: Vec<SwapProject> = sw.from.map(|f| swap_worker_projects(app, f, start)).unwrap_or_default();
    // Tutte le coppie progetto × dev in elenco, nell'ordine mostrato; le
    // spunte restano solo su queste.
    let all_targets: Vec<(ProjectId, DevId)> =
        projects.iter().flat_map(|p| p.devs.iter().map(move |d| (p.proj, d.dev))).collect();
    sw.ticked.retain(|t| all_targets.contains(t));
    let chosen: Vec<(ProjectId, DevId)> = all_targets.iter().copied().filter(|t| sw.ticked.contains(t)).collect();
    let chosen_projects = {
        let mut ps: Vec<ProjectId> = chosen.iter().map(|(p, _)| *p).collect();
        ps.dedup();
        ps.len()
    };

    let mut open = true;
    let mut confirm = false;
    let mut cancel = false;
    egui::Window::new("Scambia worker")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Il worker a sinistra viene sostituito da quello al centro nei progetti scelti: \
                     ogni sua cella passa all'altro con le stesse ore.",
                )
                .color(text_dim()),
            );
            ui.separator();
            ui.horizontal_top(|ui| {
                single_worker_column(
                    ui,
                    "swap_from",
                    "Worker da sostituire",
                    &listed,
                    &mut sw.from_search,
                    &mut sw.from,
                    None,
                    true,
                );
                ui.add_space(SWAP_GAP);
                let to_enabled = sw.from.is_some();
                single_worker_column(
                    ui,
                    "swap_to",
                    "Sostituito da",
                    &listed,
                    &mut sw.to_search,
                    &mut sw.to,
                    sw.from,
                    to_enabled,
                );
                ui.add_space(SWAP_GAP);
                ui.vertical(|ui| {
                    ui.set_width(SWAP_PROJECT_W);
                    ui.strong("Progetti e dev");
                    let Some(from) = sw.from else {
                        ui.label(egui::RichText::new("Scegli prima il worker da sostituire.").color(text_dim()));
                        return;
                    };
                    if projects.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} non ha effort dal {} in poi.",
                                app.workers.get_name_by_id(from),
                                week_label(start.0 as i32)
                            ))
                            .color(text_dim()),
                        );
                        return;
                    }
                    let currently_all = all_targets.iter().all(|t| sw.ticked.contains(t));
                    if let Some(on) = select_all_checkbox(ui, currently_all) {
                        set_ticks(&mut sw.ticked, &all_targets, on);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("swap_projects")
                        .scroll_source(drag_scroll())
                        .max_height(SWAP_LIST_H)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for sp in &projects {
                                let p = sp.proj;
                                let mine: Vec<(ProjectId, DevId)> = sp.devs.iter().map(|d| (p, d.dev)).collect();
                                let n_on = mine.iter().filter(|t| sw.ticked.contains(t)).count();
                                // Il progetto spunta / toglie tutti i suoi dev;
                                // solo alcuni ⇒ stato intermedio.
                                ui.horizontal(|ui| {
                                    let mut on = n_on == mine.len();
                                    let partial = n_on > 0 && n_on < mine.len();
                                    let cb = egui::Checkbox::new(&mut on, project_label(app, p)).indeterminate(partial);
                                    if ui.add(cb).changed() {
                                        set_ticks(&mut sw.ticked, &mine, on);
                                    }
                                    let status = app.projects.status(p);
                                    let extra = if status == ProjectStatus::Active {
                                        format!("{}h", sp.hours())
                                    } else {
                                        format!("{}h · {}", sp.hours(), status.label())
                                    };
                                    ui.label(egui::RichText::new(extra).small().color(text_dim()));
                                });
                                ui.indent(("swap_devs", p.0), |ui| {
                                    for d in &sp.devs {
                                        ui.horizontal(|ui| {
                                            let t = (p, d.dev);
                                            let mut on = sw.ticked.contains(&t);
                                            if ui.checkbox(&mut on, dev_name(app, d.dev)).changed() {
                                                set_ticks(&mut sw.ticked, &[t], on);
                                            }
                                            ui.label(
                                                egui::RichText::new(format!("{}h", d.hours)).small().color(text_dim()),
                                            );
                                        });
                                    }
                                });
                            }
                        });
                });
            });
            ui.separator();
            ui.radio_value(&mut sw.from_date, false, "Da oggi in avanti");
            ui.horizontal(|ui| {
                ui.radio_value(&mut sw.from_date, true, "Da una data in avanti:");
                let mut date = days_to_jiff(sw.date);
                let r = ui.add_enabled(
                    sw.from_date,
                    egui_extras::DatePickerButton::new(&mut date).id_salt("swap_worker_date").format("%Y-%m-%d"),
                );
                if r.changed()
                    && let Some(d) = jiff_to_days(date)
                {
                    sw.date = d;
                }
            });
            // Riepilogo.
            if let (Some(from), Some(to)) = (sw.from, sw.to)
                && !chosen.is_empty()
            {
                let ticked = &sw.ticked;
                let (cells, hours) = projects
                    .iter()
                    .flat_map(|p| p.devs.iter().filter(move |d| ticked.contains(&(p.proj, d.dev))))
                    .fold((0, 0), |(c, h), d| (c + d.cells, h + d.hours));
                ui.label(format!(
                    "{} → {}: {cells} celle ({hours}h) in {chosen_projects} progetti ({} dev), dalla settimana {}.",
                    app.workers.get_name_by_id(from),
                    app.workers.get_name_by_id(to),
                    chosen.len(),
                    week_label(start.0 as i32)
                ));
                let n = swap_conflicts(app, from, to, &chosen, start);
                if n > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{n} celle hanno già {}: le ore si sommano.",
                            app.workers.get_name_by_id(to)
                        ))
                        .color(info_blue()),
                    );
                }
            }
            ui.separator();
            let valid = sw.from.is_some() && sw.to.is_some() && !chosen.is_empty();
            ui.horizontal(|ui| {
                if ui.add_enabled(valid, egui::Button::new("OK")).clicked() {
                    confirm = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

    if confirm && let (Some(from), Some(to)) = (sw.from, sw.to) {
        actions.push(Action::SwapWorker { from, to, targets: chosen, start });
    }
    if confirm || cancel || !open {
        state.swap_worker = None;
    }
}
