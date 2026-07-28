//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

pub(crate) fn note_editor_window(ctx: &egui::Context, state: &mut UiState, actions: &mut Vec<Action>) {
    if state.note_editor.is_none() {
        return;
    }
    // Prima comparsa: dà il focus al campo nota.
    let just_opened = !state.note_editor_was_open;
    state.note_editor_was_open = true;
    let ne = state.note_editor.as_mut().unwrap();
    let title = match &ne.target {
        NoteTarget::Effort { worker, .. } => format!("Nota: {}", worker),
        NoteTarget::Dev { .. } => "Nota Dev".to_string(),
        NoteTarget::WorkerWeek { name, .. } => format!("Nota: {}", name),
    };
    let mut open = true;
    let mut save = false;
    let mut cancel = false;

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            let te = ui.add(
                egui::TextEdit::multiline(&mut ne.text)
                    .desired_rows(4)
                    .desired_width(320.0)
                    .font(cell_font()),
            );
            if just_opened {
                te.request_focus();
            }
            ui.horizontal(|ui| {
                if ui.button("Salva").clicked() {
                    save = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });

    if save {
        let action = match &ne.target {
            NoteTarget::Effort {
                proj,
                dev,
                week,
                worker,
            } => Action::SetNote {
                proj: *proj,
                dev: *dev,
                week: WeekId(*week as usize),
                worker: worker.clone(),
                note: ne.text.clone(),
            },
            NoteTarget::Dev { proj, dev } => Action::SetDevNote {
                proj: *proj,
                dev: *dev,
                note: ne.text.clone(),
            },
            NoteTarget::WorkerWeek { worker, week, .. } => Action::SetWorkerWeekNote {
                worker: *worker,
                week: *week,
                note: ne.text.clone(),
            },
        };
        actions.push(action);
    }
    if save || cancel || !open {
        state.note_editor = None;
        state.note_editor_was_open = false;
    }
}

// ── Gestione dev del progetto (+Dev / −Dev) ─────────────────────────────────

pub(crate) fn dev_manage_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(proj) = state.dev_manage else {
        return;
    };
    let in_proj = app.projects.list_devs(proj);
    let mut open = true;
    let mut close = false;

    egui::Window::new("Dev del progetto")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            for (dev, name) in app.devs.list() {
                let is_in = in_proj.contains(&dev);
                let col = dev_color(app, dev);
                let txt_col = if is_in {
                    dev_text_color(app, dev)
                } else {
                    Color32::from_gray(0x88)
                };
                let fill = if is_in { col } else { Color32::TRANSPARENT };
                let resp = ui.add(
                    egui::Button::new(egui::RichText::new(&name).color(txt_col).monospace())
                        .fill(fill)
                        .min_size(Vec2::new(160.0, 18.0)),
                );
                if resp.clicked() {
                    if is_in {
                        let has_data = app
                            .projects
                            .get_single_dev(proj, dev)
                            .map(|sd| sd.planned_effort().0 > 0 || !sd.get_weeks().is_empty())
                            .unwrap_or(false);
                        if has_data {
                            state.confirm_del_dev = Some((proj, dev));
                            close = true;
                        } else {
                            actions.push(Action::AddDevToProject {
                                proj,
                                dev,
                                add: false,
                            });
                        }
                    } else {
                        actions.push(Action::AddDevToProject {
                            proj,
                            dev,
                            add: true,
                        });
                    }
                }
            }
        });

    if close || !open {
        state.dev_manage = None;
    }
}

pub(crate) fn confirm_del_dev_window(ctx: &egui::Context, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some((proj, dev)) = state.confirm_del_dev else {
        return;
    };
    let mut open = true;
    let mut close = false;

    egui::Window::new("Conferma rimozione")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Questo dev ha già dati inseriti.\nRimuovere comunque?");
            ui.horizontal(|ui| {
                if ui.button("Sì, rimuovi").clicked() {
                    actions.push(Action::AddDevToProject {
                        proj,
                        dev,
                        add: false,
                    });
                    close = true;
                }
                if ui.button("Annulla").clicked() {
                    close = true;
                }
            });
        });

    if close || !open {
        state.confirm_del_dev = None;
    }
}

// ── Filtro progetti (Progetti ▼) ────────────────────────────────────────────

/// Larghezza (in punti) del titolo della finestra, più lo spazio per il
/// pulsante di chiusura, così da poter dare al contenuto una larghezza minima
/// pari almeno a quella della barra del titolo.
pub(crate) fn title_width(ui: &egui::Ui, title: &str) -> f32 {
    let font_id = egui::TextStyle::Heading.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font_id, egui::Color32::WHITE);
    galley.size().x + 36.0
}

pub(crate) fn project_filter_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_project_filter {
        return;
    }
    // (id, enable, tripletta, name) filtrati per la ricerca e ordinati per etichetta.
    // In elenco si mostra SOLO la tripletta (fallback al nome se assente, altrimenti
    // la riga sarebbe vuota); la ricerca combacia sia con tripletta sia con nome.
    let q = state.project_search.trim().to_lowercase();
    let mut projects: Vec<(ProjectId, Enable, String)> = app
        .projects
        .list_full()
        .into_iter()
        // i progetti chiusi non compaiono tra i progetti "attivi"
        .filter(|(id, _, _)| !app.projects.is_closed(*id))
        .filter_map(|(id, name, en)| {
            let trip = app.projects.get_tripletta(id);
            let hit = q.is_empty()
                || trip.to_lowercase().contains(&q)
                || name.to_lowercase().contains(&q);
            if !hit {
                return None;
            }
            let label = if trip.is_empty() { name } else { trip };
            Some((id, en, label))
        })
        .collect();
    projects.sort_by(|a, b| a.2.to_lowercase().cmp(&b.2.to_lowercase()));
    let mut open = true;

    let just_opened = state.project_filter_just_opened;
    state.project_filter_just_opened = false;

    // Ctrl+P a pannello aperto → toggle del "Select All" (come cliccarlo): agisce
    // sui progetti attualmente elencati (rispetta la ricerca).
    if std::mem::take(&mut state.project_filter_toggle_all) {
        let currently_all = !projects.is_empty() && projects.iter().all(|(_, en, _)| en.0);
        for (id, _, _) in &projects {
            actions.push(Action::SetProjectEnabled {
                proj: *id,
                enabled: !currently_all,
            });
        }
    }

    let mut jump: Option<ProjectId> = None;
    let mut esc = false;

    let resp = egui::Window::new("Progetti")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(90.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(240.0);
            // Ricerca (con auto-focus alla prima apertura).
            let te = ui.add(
                egui::TextEdit::singleline(&mut state.project_search)
                    .hint_text("Cerca per tripletta…")
                    .desired_width(f32::INFINITY),
            );
            if just_opened {
                te.request_focus();
            }
            let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                esc = true;
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // "Select All" agisce sui progetti attualmente elencati.
                    let currently_all =
                        !projects.is_empty() && projects.iter().all(|(_, en, _)| en.0);
                    if let Some(v) = select_all_checkbox(ui, currently_all) {
                        for (id, _, _) in &projects {
                            actions.push(Action::SetProjectEnabled {
                                proj: *id,
                                enabled: v,
                            });
                        }
                    }
                    for (id, en, label) in &projects {
                        ui.horizontal(|ui| {
                            // Spunta di visibilità.
                            let mut on = en.0;
                            if ui.checkbox(&mut on, "").changed() {
                                actions.push(Action::SetProjectEnabled {
                                    proj: *id,
                                    enabled: on,
                                });
                            }
                            // Tripletta cliccabile → salta al progetto.
                            if ui
                                .selectable_label(false, label)
                                .on_hover_text("Vai al progetto")
                                .clicked()
                            {
                                jump = Some(*id);
                            }
                        });
                    }
                    // Invio → salta al primo risultato.
                    if enter {
                        if let Some((id, _, _)) = projects.first() {
                            jump = Some(*id);
                        }
                    }
                });
        });

    if let Some(id) = jump {
        state.jump_to_project = Some(id);
        state.show_project_filter = false;
        state.project_search.clear();
        return;
    }

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || esc || (!just_opened && clicked_outside) {
        state.show_project_filter = false;
    }
}

// ── Gestione milestone (Milestone ▼) ────────────────────────────────────────

/// Colore `u32` 0xRRGGBB → `Color32` grezzo (senza filtro B/N, per l'editing).
pub(crate) fn u32_to_color(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

pub(crate) fn color_to_u32(c: Color32) -> u32 {
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32)
}

pub(crate) fn milestone_manager_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_milestone_manager {
        return;
    }
    let milestones = app.milestones.list();
    let mut open = true;
    let just_opened = state.milestone_manager_just_opened;
    state.milestone_manager_just_opened = false;

    let resp = egui::Window::new("Milestone")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(120.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(title_width(ui, "Milestone").max(220.0));
            if milestones.is_empty() {
                ui.label("Nessuna milestone. Creane una dalla toolbar.");
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for (id, name, color) in &milestones {
                        ui.horizontal(|ui| {
                            let mut col = u32_to_color(*color);
                            if ui.color_edit_button_srgba(&mut col).changed() {
                                actions.push(Action::SetMilestoneColor {
                                    milestone: *id,
                                    color: color_to_u32(col),
                                });
                            }
                            ui.label(name);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("🗑").on_hover_text("Elimina milestone").clicked()
                                    {
                                        actions.push(Action::DeleteMilestone { milestone: *id });
                                    }
                                },
                            );
                        });
                    }
                });
        });

    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    // niente auto-chiusura al click-fuori: il color picker apre popup esterni.
    if !open {
        state.show_milestone_manager = false;
    }
    let _ = (just_opened, clicked_outside);
}

// ── Filtro progetti chiusi (Closed ▼) ───────────────────────────────────────

pub(crate) fn closed_filter_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_closed_filter {
        return;
    }
    // tutti i progetti, ordinati alfabeticamente per etichetta; il check indica "chiuso"
    let mut projects: Vec<(ProjectId, String)> = app
        .projects
        .list_full()
        .into_iter()
        .map(|(id, name, _)| {
            let trip = app.projects.get_tripletta(id);
            let label = if trip.is_empty() { name } else { trip };
            (id, label)
        })
        .collect();
    projects.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    let mut open = true;
    let just_opened = state.closed_filter_just_opened;
    state.closed_filter_just_opened = false;

    let mut window = egui::Window::new("Closed")
        .collapsible(false)
        .resizable(false)
        .open(&mut open);
    // all'apertura ancora la finestra sotto il pulsante "Closed ▼";
    // dopo resta trascinabile dall'utente
    window = if just_opened {
        window.current_pos(state.closed_btn_pos)
    } else {
        window.default_pos(state.closed_btn_pos)
    };
    let resp = window.show(ctx, |ui| {
        let min_w = title_width(ui, "Closed");
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                ui.set_min_width(min_w);
                for (id, label) in &projects {
                    let mut closed = app.projects.is_closed(*id);
                    if ui.checkbox(&mut closed, label).changed() {
                        actions.push(Action::SetProjectClosed { proj: *id, closed });
                    }
                }
            });
    });

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_closed_filter = false;
    }
}

// ── Filtro worker (Ctrl+F / Workers ▼) ──────────────────────────────────────

/// Per ogni nome worker: (numero di progetti APERTI, numero di progetti CHIUSI)
/// distinti in cui il worker ha effort > 0. Con `current_week_only` (modalità
/// Ctrl+G) conta solo la presenza nella **settimana corrente**; altrimenti (Ctrl+F)
/// su **tutte** le settimane. Ignora filtri e vista. Chiave per nome (come il
/// filtro worker).
fn worker_project_counts(app: &App, current_week_only: bool) -> HashMap<String, (u32, u32)> {
    let cur = current_week_id();
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for (proj, _) in app.projects.list() {
        let closed = app.projects.is_closed(proj);
        // worker distinti presenti (effort > 0) in questo progetto
        let mut seen: HashSet<WorkerId> = HashSet::new();
        for dev in app.projects.list_devs(proj) {
            if let Some(sd) = app.projects.get_single_dev(proj, dev) {
                let weeks = if current_week_only {
                    vec![cur]
                } else {
                    sd.get_weeks()
                };
                for wk in weeks {
                    if let Some(sew) = sd.get_all(wk) {
                        for (wid, se) in sew.worker_id.iter() {
                            if *wid != WORKER_ID_ZERO && se.get_effort().0 > 0 {
                                seen.insert(*wid);
                            }
                        }
                    }
                }
            }
        }
        for wid in seen {
            let name = app.workers.get_name_by_id(wid).to_string();
            let e = counts.entry(name).or_default();
            if closed {
                e.1 += 1;
            } else {
                e.0 += 1;
            }
        }
    }
    counts
}

pub(crate) fn worker_filter_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_worker_filter {
        return;
    }
    // L'unico filtro dell'elenco è `show_in_find` (default true): la visibilità
    // nel footer non influenza più questa lista.
    let all: Vec<String> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| app.workers.is_shown_in_find(*id))
        .map(|(_, n)| n)
        .collect();
    // Conteggio progetti per worker (aperti/chiusi), mostrato accanto al nome.
    // In modalità Ctrl+G conta solo i progetti con presenza nella settimana corrente.
    let counts = worker_project_counts(app, state.worker_filter_current_week);
    let mut open = true;
    let mut filter = state.worker_filter.clone();
    // Titolo diverso in modalità "settimana corrente" (Ctrl+G) così si distingue
    // dal filtro su tutte le settimane (Ctrl+F).
    let title = if state.worker_filter_current_week {
        "Workers (settimana corrente)"
    } else {
        "Workers"
    };

    let just_opened = state.worker_filter_just_opened;
    state.worker_filter_just_opened = false;

    // Ctrl+F a pannello aperto → toggle del "Select All" (come cliccarlo).
    if std::mem::take(&mut state.worker_filter_toggle_all) {
        let currently_all = match &filter {
            None => !all.is_empty(),
            Some(s) => !all.is_empty() && all.iter().all(|n| s.contains(n)),
        };
        filter = if currently_all { Some(HashSet::new()) } else { None };
    }

    let resp = egui::Window::new(title)
        .id(egui::Id::new("worker_filter_window")) // id stabile: il titolo cambia con la modalità
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(140.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            let min_w = title_width(ui, title);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(min_w);
                let currently_all = match &filter {
                    None => !all.is_empty(),
                    Some(s) => !all.is_empty() && all.iter().all(|n| s.contains(n)),
                };
                if let Some(v) = select_all_checkbox(ui, currently_all) {
                    filter = if v { None } else { Some(HashSet::new()) };
                }
                for name in &all {
                    let mut sel = match &filter {
                        None => true,
                        Some(s) => s.contains(name),
                    };
                    // Etichetta = nome + conteggio progetti aperti/chiusi. Il nome
                    // resta l'identificatore del filtro (solo l'etichetta cambia).
                    let (op, cl) = counts.get(name).copied().unwrap_or((0, 0));
                    let label = format!("{name}    {op}/Aperti - {cl}/Chiuso");
                    if ui.checkbox(&mut sel, label).changed() {
                        let set = filter.get_or_insert_with(|| all.iter().cloned().collect());
                        if sel {
                            set.insert(name.clone());
                        } else {
                            set.remove(name);
                        }
                    }
                }
            });
        });

    // se tutti selezionati → nessun filtro
    if let Some(set) = &filter {
        if set.len() == all.len() && all.iter().all(|n| set.contains(n)) {
            filter = None;
        }
    }
    state.worker_filter = filter;
    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_worker_filter = false;
    }
}

/// Filtro per Dev (Filtri ▸ Dev… / Ctrl+D). Elenca tutti i dev definiti con una
/// spunta: restano visibili solo i progetti in cui almeno un dev selezionato ha
/// effort > 0, e dentro il progetto solo quei dev. Si combina in AND col filtro
/// worker (Ctrl+F/Ctrl+G). Selezione non persistita.
pub(crate) fn dev_filter_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_dev_filter {
        return;
    }
    let all: Vec<(DevId, String)> = app.devs.list();
    let mut open = true;
    let mut filter = state.dev_filter.clone();
    let title = "Dev";

    let just_opened = state.dev_filter_just_opened;
    state.dev_filter_just_opened = false;

    // Ctrl+D a pannello aperto → toggle del "Select All" (come cliccarlo).
    if std::mem::take(&mut state.dev_filter_toggle_all) {
        let currently_all = match &filter {
            None => !all.is_empty(),
            Some(s) => !all.is_empty() && all.iter().all(|(id, _)| s.contains(id)),
        };
        filter = if currently_all { Some(HashSet::new()) } else { None };
    }

    let resp = egui::Window::new(title)
        .id(egui::Id::new("dev_filter_window"))
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(180.0, 60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            let min_w = title_width(ui, title);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(min_w);
                let currently_all = match &filter {
                    None => !all.is_empty(),
                    Some(s) => !all.is_empty() && all.iter().all(|(id, _)| s.contains(id)),
                };
                if let Some(v) = select_all_checkbox(ui, currently_all) {
                    filter = if v { None } else { Some(HashSet::new()) };
                }
                for (id, name) in &all {
                    let mut sel = match &filter {
                        None => true,
                        Some(s) => s.contains(id),
                    };
                    if ui.checkbox(&mut sel, name).changed() {
                        let set =
                            filter.get_or_insert_with(|| all.iter().map(|(i, _)| *i).collect());
                        if sel {
                            set.insert(*id);
                        } else {
                            set.remove(id);
                        }
                    }
                }
            });
        });

    // se tutti selezionati → nessun filtro
    if let Some(set) = &filter {
        if set.len() == all.len() && all.iter().all(|(id, _)| set.contains(id)) {
            filter = None;
        }
    }
    state.dev_filter = filter;
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_dev_filter = false;
    }
}

// ── Gestione "ghost" (Filtri ▸ Ghost worker…) ───────────────────────────────

/// Elenca **tutti** i worker (escluso solo il worker "zero"), a prescindere da
/// `hide_in_footer`, `show_in_find` o dal filtro attivo, ognuno con una spunta
/// **Ghost**. È l'unico punto sempre raggiungibile per associare/togliere il
/// ghost anche a worker non visibili nel footer.
pub(crate) fn ghost_manager_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_ghost_manager {
        return;
    }
    let workers = app.workers.list(); // (WorkerId, nome), ordinati per nome, senza lo zero
    let mut open = true;
    let just_opened = state.ghost_manager_just_opened;
    state.ghost_manager_just_opened = false;

    let resp = egui::Window::new("Ghost worker")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(160.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(title_width(ui, "Ghost worker").max(240.0));
            ui.label(
                "Spunta i worker \"ghost\": se inseriti nell'effort di un dev vengono \
                 evidenziati come anomalia (cella rossa, nome dev lampeggiante, barra/linea \
                 rossa negli export).",
            );
            ui.add_space(4.0);
            ui.separator();
            if workers.is_empty() {
                ui.label("Nessun worker.");
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (id, name) in &workers {
                        let mut on = app.workers.is_ghost(*id);
                        if ui.checkbox(&mut on, name).changed() {
                            actions.push(Action::SetWorkerGhost {
                                worker: *id,
                                ghost: on,
                            });
                        }
                    }
                });
        });

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_ghost_manager = false;
    }
}

// ── Flusso "Sposta" effort (blocco / devs) ──────────────────────────────────

/// Intervallo [min, max] di settimana coperto da un insieme di spostamenti.
pub(crate) fn moves_span(moves: &[(DevId, Vec<WeekId>)]) -> Option<(usize, usize)> {
    let mut lo = usize::MAX;
    let mut hi = usize::MIN;
    for (_, ws) in moves {
        for w in ws {
            lo = lo.min(w.0);
            hi = hi.max(w.0);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

/// Costruisce il passo "Params": numero settimane vuoto e milestone del
/// progetto, pre-selezionando quelle che cadono nell'intervallo spostato.
pub(crate) fn make_move_params(app: &App, proj: ProjectId, moves: Vec<(DevId, Vec<WeekId>)>) -> MoveDialog {
    let span = moves_span(&moves);
    let milestones = app
        .projects
        .list_project_milestones(proj)
        .into_iter()
        .map(|(id, w)| {
            let name = app.milestones.get_name(id).unwrap_or("?").to_string();
            let inside = span.is_some_and(|(lo, hi)| w.0 >= lo && w.0 <= hi);
            (id, name, inside)
        })
        .collect();
    MoveDialog::Params {
        proj,
        moves,
        weeks_text: String::new(),
        milestones,
    }
}

pub(crate) fn move_dialog_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if state.move_dialog.is_none() {
        return;
    }
    let just_opened = !state.move_dialog_was_open;
    state.move_dialog_was_open = true;

    let mut open = true; // pulsante [x] della finestra
    let mut close = false; // termina l'intero flusso
    let mut next: Option<MoveDialog> = None; // passa allo step successivo

    match state.move_dialog.as_mut().unwrap() {
        MoveDialog::SelectDevs {
            proj,
            candidates,
            selected,
        } => {
            egui::Window::new("Sposta devs")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Seleziona i dev da spostare:");
                    ui.add_space(4.0);
                    let currently_all = candidates.iter().all(|(id, _)| selected.contains(id));
                    if let Some(v) = select_all_checkbox(ui, currently_all) {
                        if v {
                            for (id, _) in candidates.iter() {
                                selected.insert(*id);
                            }
                        } else {
                            selected.clear();
                        }
                    }
                    for (id, name) in candidates.iter() {
                        let mut on = selected.contains(id);
                        if ui.checkbox(&mut on, name).changed() {
                            if on {
                                selected.insert(*id);
                            } else {
                                selected.remove(id);
                            }
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        let any = !selected.is_empty();
                        if ui.add_enabled(any, egui::Button::new("Avanti")).clicked() {
                            let mut moves: Vec<(DevId, Vec<WeekId>)> = Vec::new();
                            for (d, _) in candidates.iter() {
                                if selected.contains(d) {
                                    let ws = app.projects.dev_effort_weeks(*proj, *d);
                                    if !ws.is_empty() {
                                        moves.push((*d, ws));
                                    }
                                }
                            }
                            next = Some(make_move_params(app, *proj, moves));
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
        MoveDialog::Params {
            proj,
            moves,
            weeks_text,
            milestones,
        } => {
            egui::Window::new("Sposta effort")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Di quante settimane spostare?");
                    ui.weak("(+ verso destra, − verso sinistra)");
                    let le = ui.add(
                        egui::TextEdit::singleline(weeks_text)
                            .desired_width(80.0)
                            .hint_text("es. 2 o -3"),
                    );
                    if just_opened {
                        le.request_focus();
                    }
                    let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if !milestones.is_empty() {
                        ui.separator();
                        ui.label("Milestone da spostare insieme:");
                        let currently_all = milestones.iter().all(|(_, _, s)| *s);
                        if let Some(v) = select_all_checkbox(ui, currently_all) {
                            for (_, _, s) in milestones.iter_mut() {
                                *s = v;
                            }
                        }
                        for (_, name, sel) in milestones.iter_mut() {
                            ui.checkbox(sel, name.as_str());
                        }
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        let confirm = ui.button("Sposta").clicked() || entered;
                        if confirm {
                            if let Ok(n) = weeks_text.trim().parse::<i64>() {
                                if n == 0 {
                                    close = true;
                                } else {
                                    let sel_ms: Vec<MilestoneId> = milestones
                                        .iter()
                                        .filter(|(_, _, s)| *s)
                                        .map(|(id, _, _)| *id)
                                        .collect();
                                    match app.projects.move_overflow_side(*proj, moves, n) {
                                        Some(end_side) => {
                                            next = Some(MoveDialog::Overflow {
                                                proj: *proj,
                                                moves: moves.clone(),
                                                delta: n,
                                                milestones: sel_ms,
                                                end_side,
                                            });
                                        }
                                        None => {
                                            actions.push(Action::MoveEffort {
                                                proj: *proj,
                                                moves: moves.clone(),
                                                delta: n,
                                                milestones: sel_ms,
                                                resolution: OverflowResolution::None,
                                            });
                                            close = true;
                                        }
                                    }
                                }
                            }
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
        MoveDialog::Overflow {
            proj,
            moves,
            delta,
            milestones,
            end_side,
        } => {
            egui::Window::new("Sforamento confine")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(if *end_side {
                        "L'effort spostato supera la fine del progetto."
                    } else {
                        "L'effort spostato supera l'inizio del progetto."
                    });
                    ui.add_space(6.0);
                    let push = |actions: &mut Vec<Action>, resolution| {
                        actions.push(Action::MoveEffort {
                            proj: *proj,
                            moves: moves.clone(),
                            delta: *delta,
                            milestones: milestones.clone(),
                            resolution,
                        });
                    };
                    ui.horizontal(|ui| {
                        let move_lbl = if *end_side {
                            "Sposta la fine del progetto"
                        } else {
                            "Sposta l'inizio del progetto"
                        };
                        if ui.button(move_lbl).clicked() {
                            push(actions, OverflowResolution::MoveBoundary);
                            close = true;
                        }
                        let lose_lbl = if *end_side {
                            "Perdi le settimane oltre la fine"
                        } else {
                            "Perdi le settimane prima dell'inizio"
                        };
                        if ui.button(lose_lbl).clicked() {
                            push(actions, OverflowResolution::Truncate);
                            close = true;
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
    }

    if let Some(n) = next {
        state.move_dialog = Some(n);
        state.move_dialog_was_open = false; // rifocalizza lo step successivo
    } else if close || !open {
        state.move_dialog = None;
        state.move_dialog_was_open = false;
    }
}

// ── Popup di modifica (tripletta / inizio / fine / categoria) ───────────────

pub(crate) fn popup_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    if state.popup.is_none() {
        return;
    }
    // Prima comparsa della finestra: dà il focus al campo di testo.
    let just_opened = !state.popup_was_open;
    state.popup_was_open = true;
    let popup = state.popup.as_mut().unwrap();
    let mut open = true;
    let mut close = false;

    match popup {
        Popup::Tripletta { proj, text } => {
            egui::Window::new("Tripletta")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    let le = ui.add(
                        egui::TextEdit::singleline(text)
                            .desired_width(220.0)
                            .font(cell_font()),
                    );
                    if just_opened {
                        le.request_focus();
                    }
                    let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || entered {
                            actions.push(Action::SetProjectTripletta {
                                proj: *proj,
                                text: text.clone(),
                            });
                            close = true;
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
        Popup::ProjectNotes {
            proj,
            weeks,
            current,
        } => {
            let title = format!("Note progetto: {}", app.projects.get_tripletta(*proj));
            egui::Window::new(title)
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    // Cap in altezza + scroll: con tante settimane l'elenco
                    // scorre invece di far crescere la finestra oltre lo schermo.
                    let max_h = (ui.ctx().screen_rect().height() - 160.0).max(200.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_h.min(480.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (i, (week, text)) in weeks.iter_mut().enumerate() {
                                let label =
                                    days_to_local(week.0 as i32).format("%y-%m-%d").to_string();
                                let is_current = *week == *current;
                                if i > 0 {
                                    ui.add_space(8.0);
                                }
                                if is_current {
                                    // Settimana corrente: intestazione evidenziata
                                    // e nota modificabile.
                                    ui.colored_label(g(EFFORT_ORANGE), format!("--- {label} ---"));
                                    let te = ui.add(
                                        egui::TextEdit::multiline(text)
                                            .desired_rows(3)
                                            .desired_width(420.0)
                                            .font(cell_font()),
                                    );
                                    if just_opened {
                                        te.request_focus();
                                    }
                                } else {
                                    // Settimane passate: sola lettura, solo testo.
                                    ui.label(format!("--- {label} ---"));
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(text.as_str()).font(cell_font()),
                                        )
                                        .wrap(),
                                    );
                                }
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Salva").clicked() {
                            // Scarta le voci vuote: una settimana viene salvata
                            // solo se contiene testo.
                            let map: HashMap<WeekId, String> = weeks
                                .iter()
                                .filter(|(_, t)| !t.trim().is_empty())
                                .map(|(w, t)| (*w, t.clone()))
                                .collect();
                            actions.push(Action::SetProjectNotes {
                                proj: *proj,
                                notes: map,
                            });
                            close = true;
                        }
                        if ui.button("Annulla").clicked() {
                            close = true;
                        }
                    });
                });
        }
        Popup::Start { proj, text } => {
            date_popup_window(
                ctx,
                "Data inizio (yy-mm-dd)",
                text,
                &mut open,
                &mut close,
                just_opened,
                |date| {
                    actions.push(Action::SetProjectStartWeek { proj: *proj, date });
                },
            );
        }
        Popup::End { proj, text } => {
            date_popup_window(
                ctx,
                "Data fine (yy-mm-dd)",
                text,
                &mut open,
                &mut close,
                just_opened,
                |date| {
                    actions.push(Action::SetProjectEndWeek { proj: *proj, date });
                },
            );
        }
        Popup::Category { proj } => {
            egui::Window::new("Categoria")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    if ui.button("— Nessuna").clicked() {
                        actions.push(Action::SetProjectCategory {
                            proj: *proj,
                            cat: None,
                        });
                        close = true;
                    }
                    for (id, name) in app.categories.list() {
                        if ui.button(name).clicked() {
                            actions.push(Action::SetProjectCategory {
                                proj: *proj,
                                cat: Some(id),
                            });
                            close = true;
                        }
                    }
                });
        }
        Popup::WorkerMax { worker, name, text } => {
            hours_popup_window(
                ctx,
                &format!("Ore max: {name}"),
                text,
                &mut open,
                &mut close,
                just_opened,
                |hours| {
                    actions.push(Action::SetWorkerMaxHours {
                        worker: *worker,
                        hours,
                    });
                },
                DEFAULT_MAX_HOURS,
            );
        }
        Popup::WorkerWeekMax {
            worker,
            name,
            week,
            text,
        } => {
            // "Default" = max globale del worker (azzera l'override per la settimana).
            let global_max = app.workers.get_max_hours(*worker);
            hours_popup_window(
                ctx,
                &format!("Override settimana - {name}"),
                text,
                &mut open,
                &mut close,
                just_opened,
                |hours| {
                    actions.push(Action::SetWorkerWeekOverride {
                        worker: *worker,
                        week: *week,
                        hours,
                    });
                },
                global_max,
            );
        }
        Popup::BulkWeekMax { week, date, text } => {
            // "Default" = 40 → azzera l'override per tutti i worker su questa settimana.
            hours_popup_window(
                ctx,
                &format!("Limite settimana (tutti) - {date}"),
                text,
                &mut open,
                &mut close,
                just_opened,
                |hours| {
                    actions.push(Action::SetBulkWeekLimit { week: *week, hours });
                },
                DEFAULT_MAX_HOURS,
            );
        }
    }

    if close || !open {
        state.popup = None;
        state.popup_was_open = false;
    }
}

/// Finestra con campo data + OK / Rimuovi / Annulla. `on_confirm` riceve la stringa
/// data ("" = rimuovi).
pub(crate) fn date_popup_window(
    ctx: &egui::Context,
    title: &str,
    text: &mut String,
    open: &mut bool,
    close: &mut bool,
    just_opened: bool,
    mut on_confirm: impl FnMut(String),
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            let _ = just_opened;
            // Selezione col mouse: il calendario è inizializzato con la data
            // corrente del campo (o con oggi, se non impostata). Quando l'utente
            // sceglie un giorno, la stringa `text` viene aggiornata.
            let mut date = date_str_to_jiff(text).unwrap_or_else(today_jiff);
            let picked = ui.add(
                egui_extras::DatePickerButton::new(&mut date)
                    .id_salt(title)
                    .format("%Y-%m-%d"),
            );
            if picked.changed() {
                *text = jiff_to_date_str(&date);
            }
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    on_confirm(text.clone());
                    *close = true;
                }
                if ui.button("Rimuovi").clicked() {
                    on_confirm(String::new());
                    *close = true;
                }
                if ui.button("Annulla").clicked() {
                    *close = true;
                }
            });
        });
}

/// Finestra con campo numerico (ore max) + OK / Default / Annulla.
/// `on_confirm` riceve le ore (u32); il pulsante "Default" conferma `default_hours`,
/// che azzera l'override/limite (vedi `Worker::set_max_hours` / `set_week_override`).
pub(crate) fn hours_popup_window(
    ctx: &egui::Context,
    title: &str,
    text: &mut String,
    open: &mut bool,
    close: &mut bool,
    just_opened: bool,
    mut on_confirm: impl FnMut(u32),
    default_hours: u32,
) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(open)
        .show(ctx, |ui| {
            let le = ui.add(
                egui::TextEdit::singleline(text)
                    .desired_width(120.0)
                    .font(cell_font()),
            );
            if just_opened {
                le.request_focus();
            }
            let entered = le.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || entered {
                    if let Ok(hours) = text.trim().parse::<u32>() {
                        on_confirm(hours);
                    }
                    *close = true;
                }
                if ui.button("Default").clicked() {
                    on_confirm(default_hours);
                    *close = true;
                }
                if ui.button("Zero").clicked() {
                    on_confirm(0);
                    *close = true;
                }
                if ui.button("Annulla").clicked() {
                    *close = true;
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_project_counts_splits_open_and_closed() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let dev = app.devs.add("Frontend");
        let cur = current_week_id();
        let other = WeekId(20000); // settimana lontana (non corrente)

        // P1 aperto: Alice nella settimana corrente, Bob in un'altra settimana.
        let p1 = app.projects.add("P1", Some("AAA"), Some(cur));
        app.projects.add_dev(p1, dev);
        app.projects.add_effort(p1, dev, cur, alice, Effort(8));
        app.projects.add_effort(p1, dev, other, bob, Effort(8));

        // P2 chiuso: Alice nella settimana corrente.
        let p2 = app.projects.add("P2", Some("BBB"), Some(cur));
        app.projects.add_dev(p2, dev);
        app.projects.add_effort(p2, dev, cur, alice, Effort(8));
        app.projects.set_closed(p2, true);

        // P3 aperto: Alice con effort 0 nella settimana corrente → non conta.
        let p3 = app.projects.add("P3", Some("CCC"), Some(cur));
        app.projects.add_dev(p3, dev);
        app.projects.add_effort(p3, dev, cur, alice, Effort(0));

        // P4 aperto: Bob nella settimana corrente.
        let p4 = app.projects.add("P4", Some("DDD"), Some(cur));
        app.projects.add_dev(p4, dev);
        app.projects.add_effort(p4, dev, cur, bob, Effort(8));

        // Ctrl+F: tutte le settimane. Bob è in P1 (altra settimana) + P4 = 2 aperti.
        let all = worker_project_counts(&app, false);
        assert_eq!(all.get("Alice").copied(), Some((1, 1)));
        assert_eq!(all.get("Bob").copied(), Some((2, 0)));

        // Ctrl+G: solo settimana corrente. Bob non è in P1 questa settimana → solo P4.
        let cw = worker_project_counts(&app, true);
        assert_eq!(cw.get("Alice").copied(), Some((1, 1)));
        assert_eq!(cw.get("Bob").copied(), Some((1, 0)));
    }
}

