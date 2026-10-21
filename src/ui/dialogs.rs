//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

pub(crate) fn note_editor_window(
    ctx: &egui::Context,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
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

// ── Inserimento multiplo (tasto destro su cella vuota) ──────────────────────

/// Dialog dell'inserimento multiplo: elenco worker con spunta (gli stessi del
/// filtro, cioè `show_in_find`), ore settimanali e numero di settimane. Con OK
/// scrive l'effort a tutti i worker selezionati per N settimane consecutive a
/// partire dalla settimana della cella cliccata (inclusa), **sovrascrivendo**
/// eventuali valori già presenti. Le settimane oltre la fine della griglia
/// (`app.end_week`) vengono scartate: il conteggio effettivo è mostrato nella
/// dialog.
pub(crate) fn bulk_fill_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if state.bulk_fill.is_none() {
        return;
    }
    let just_opened = !state.bulk_fill_was_open;
    state.bulk_fill_was_open = true;

    let bf = state.bulk_fill.as_mut().unwrap();
    let dev_label = dev_name(app, bf.dev);
    let start_label = primo_giorno_settimana_corrente(&days_to_local(bf.week))
        .format("%y-%m-%d")
        .to_string();

    // Worker elencabili (come la dialog dei filtri) filtrati dalla ricerca.
    let q = bf.search.trim().to_lowercase();
    let listed: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| app.workers.is_shown_in_find(*id))
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .collect();

    // Valori inseriti: effort (ore/settimana) e numero di settimane.
    let effort = bf.effort_text.trim().parse::<usize>().ok();
    let weeks = bf
        .weeks_text
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0);
    // Settimane realmente scrivibili (la griglia finisce a `end_week`).
    let available =
        ((app.end_week.0 as i64 - bf.week as i64) / WEEK_STEP as i64 + 1).max(0) as usize;
    let effective = weeks.map(|n| n.min(available)).unwrap_or(0);

    let mut open = true;
    let mut confirm = false;
    let mut cancel = false;

    egui::Window::new("Inserimento multiplo")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!("{} — dalla settimana {}", dev_label, start_label))
                    .color(text_dim()),
            );
            ui.separator();

            // ── Worker ──
            let sf = ui.add(
                egui::TextEdit::singleline(&mut bf.search)
                    .hint_text("Cerca worker…")
                    .desired_width(240.0),
            );
            if just_opened {
                sf.request_focus();
            }
            let currently_all =
                !listed.is_empty() && listed.iter().all(|(id, _)| bf.selected.contains(id));
            if let Some(all) = select_all_checkbox(ui, currently_all) {
                for (id, _) in &listed {
                    if all {
                        bf.selected.insert(*id);
                    } else {
                        bf.selected.remove(id);
                    }
                }
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.set_min_width(240.0);
                    if listed.is_empty() {
                        ui.label("(nessun worker)");
                    }
                    for (id, name) in &listed {
                        let mut sel = bf.selected.contains(id);
                        if ui.checkbox(&mut sel, name).changed() {
                            if sel {
                                bf.selected.insert(*id);
                            } else {
                                bf.selected.remove(id);
                            }
                        }
                    }
                });

            ui.separator();

            // ── Ore e settimane ──
            let mut submit = false;
            ui.horizontal(|ui| {
                ui.label("Ore a settimana:");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut bf.effort_text)
                        .desired_width(50.0)
                        .hint_text("es. 8"),
                );
                submit |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
            ui.horizontal(|ui| {
                ui.label("Per quante settimane:");
                let r = ui.add(egui::TextEdit::singleline(&mut bf.weeks_text).desired_width(50.0));
                submit |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });

            // Riepilogo / avvisi.
            if effort.is_none() && !bf.effort_text.trim().is_empty() {
                ui.colored_label(Color32::RED, "Ore non valide (numero intero).");
            }
            if weeks.is_none() && !bf.weeks_text.trim().is_empty() {
                ui.colored_label(Color32::RED, "Settimane non valide (almeno 1).");
            }
            if let (Some(e), Some(n)) = (effort, weeks) {
                let last = bf.week + (effective.saturating_sub(1) * WEEK_STEP) as i32;
                let last_label = primo_giorno_settimana_corrente(&days_to_local(last))
                    .format("%y-%m-%d")
                    .to_string();
                ui.label(format!(
                    "{} worker × {} h × {} settimane ({} → {})",
                    bf.selected.len(),
                    e,
                    effective,
                    start_label,
                    last_label
                ));
                if effective < n {
                    ui.colored_label(
                        Color32::RED,
                        format!(
                            "Solo {} settimane disponibili fino alla fine della griglia.",
                            effective
                        ),
                    );
                }
            }

            let valid = !bf.selected.is_empty() && effort.is_some() && effective > 0;
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(valid, egui::Button::new("Inserisci"))
                    .clicked()
                {
                    confirm = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            if submit && valid {
                confirm = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

    if confirm {
        let mut workers: Vec<WorkerId> = bf.selected.iter().copied().collect();
        workers.sort_by_key(|w| w.0);
        actions.push(Action::BulkFillEffort {
            proj: bf.proj,
            dev: bf.dev,
            start: WeekId(bf.week as usize),
            workers,
            effort: Effort(effort.unwrap_or(0)),
            weeks: effective,
        });
    }
    if confirm || cancel || !open {
        state.bulk_fill = None;
        state.bulk_fill_was_open = false;
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

pub(crate) fn confirm_del_dev_window(
    ctx: &egui::Context,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
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

/// Conferma di «Sposta inizio/fine progetto qua» quando nel nuovo periodo non
/// entrerebbero effort o milestone già inseriti: li elenca e chiede se
/// cancellarli. «Annulla» (o la chiusura) lascia tutto com'era.
pub(crate) fn bound_move_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(bm) = state.bound_move.as_ref() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let fmt = |w: WeekId| days_to_local(w.0 as i32).format("%y-%m-%d").to_string();
    let what = match bm.bound {
        ProjectBound::Start => "l'inizio",
        ProjectBound::End => "la fine",
    };

    // Effort raggruppati per dev: settimane, ore e intervallo.
    let mut per_dev: Vec<(DevId, Vec<WeekId>)> = Vec::new();
    for (dev, w) in &bm.loss.efforts {
        match per_dev.iter_mut().find(|(d, _)| d == dev) {
            Some((_, ws)) => ws.push(*w),
            None => per_dev.push((*dev, vec![*w])),
        }
    }

    egui::Window::new("Conferma spostamento")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(format!(
                "Spostando {what} di {} alla settimana {} resterebbero fuori dal \
                 progetto i dati qui sotto.",
                project_label(app, bm.proj),
                fmt(bm.week)
            ));
            ui.label("Vuoi cancellarli e spostare la data?");
            ui.separator();
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                if !per_dev.is_empty() {
                    ui.label(egui::RichText::new("Effort").strong());
                    for (dev, ws) in &per_dev {
                        let hours: usize = app
                            .projects
                            .get_single_dev(bm.proj, *dev)
                            .map(|sd| ws.iter().map(|w| sd.get_effort_by_week(*w).0).sum())
                            .unwrap_or(0);
                        let first = ws.iter().min().copied().unwrap_or(bm.week);
                        let last = ws.iter().max().copied().unwrap_or(bm.week);
                        let range = if first == last {
                            fmt(first)
                        } else {
                            format!("{} … {}", fmt(first), fmt(last))
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "≡  {}: {} settiman{}, {hours} h ({range})",
                                dev_name(app, *dev),
                                ws.len(),
                                if ws.len() == 1 { "a" } else { "e" },
                            ))
                            .color(dev_color(app, *dev)),
                        );
                    }
                }
                if !bm.loss.milestones.is_empty() {
                    ui.label(egui::RichText::new("Milestone").strong());
                    for (id, w) in &bm.loss.milestones {
                        let name = app.milestones.get_name(*id).unwrap_or("?");
                        let icon = app.milestones.get_kind(*id).icon();
                        ui.label(
                            egui::RichText::new(format!("{icon} {name}  —  {}", fmt(*w))).color(
                                u32_to_color(app.milestones.get_color(*id).unwrap_or(0x808080)),
                            ),
                        );
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Cancella e sposta").clicked() {
                    actions.push(Action::MoveProjectBound {
                        proj: bm.proj,
                        bound: bm.bound,
                        week: bm.week,
                    });
                    close = true;
                }
                if ui.button("Annulla").clicked() {
                    close = true;
                }
            });
        });

    if close || !open {
        state.bound_move = None;
    }
}

// ── Utility comuni alle finestre ────────────────────────────────────────────

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
                ui.label(
                    "Nessuna milestone. Creane una dalla toolbar (di sistema) o dal menù \
                     della griglia di un progetto (solo di quel progetto).",
                );
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    // Griglia: ogni colonna è larga quanto il suo contenuto più
                    // lungo, così i nomi lunghi spostano Int/Ext/tipo/🗑 a destra
                    // per tutte le righe invece di finirci sotto.
                    egui::Grid::new("milestone_manager_grid")
                        .num_columns(5)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            for (id, name, color) in &milestones {
                                let mut col = u32_to_color(*color);
                                if ui.color_edit_button_srgba(&mut col).changed() {
                                    actions.push(Action::SetMilestoneColor {
                                        milestone: *id,
                                        color: color_to_u32(col),
                                    });
                                }
                                ui.horizontal(|ui| {
                                    ui.label(name);
                                    // Milestone personalizzata: accanto al nome il
                                    // progetto a cui appartiene (non compare altrove).
                                    if let Some(owner) = app.milestones.owner(*id) {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "· {}",
                                                project_label(app, owner)
                                            ))
                                            .small()
                                            .color(text_dim()),
                                        )
                                        .on_hover_text(
                                            "Milestone disponibile solo in questo progetto",
                                        );
                                    }
                                });
                                // Categorie di stampa (Int / Ext): decidono
                                // in quale PDF la milestone viene disegnata.
                                let cats = app.milestones.get_categories(*id);
                                if let Some((category, on)) =
                                    milestone_categories_toggles(ui, &cats)
                                {
                                    actions.push(Action::SetMilestoneCategory {
                                        milestone: *id,
                                        category,
                                        on,
                                    });
                                }
                                // Tipo modificabile anche dopo l'inserimento:
                                // il tipo è della milestone, quindi il cambio
                                // vale in tutti i progetti in cui è collocata.
                                let mut kind = app.milestones.get_kind(*id);
                                if milestone_kind_combo(ui, "", id.0, &mut kind) {
                                    actions.push(Action::SetMilestoneKind {
                                        milestone: *id,
                                        kind,
                                    });
                                }
                                if ui.button("🗑").on_hover_text("Elimina milestone").clicked() {
                                    actions.push(Action::DeleteMilestone { milestone: *id });
                                }
                                ui.end_row();
                            }
                        });
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

// ── Stato dei progetti (Filtri ▸ Stato progetti…) ───────────────────────────

pub(crate) fn closed_filter_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_closed_filter {
        return;
    }
    // tutti i progetti, ordinati alfabeticamente per etichetta, ognuno con la
    // combo del suo stato: Attivo / On-Hold / Closed (on-hold = chiuso con
    // un'etichetta diversa)
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

    let mut window = egui::Window::new("Stato progetti")
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
    #[cfg(test)]
    {
        state.status_combo_rects.clear();
        state.status_item_rects.clear();
    }
    let resp = window.show(ctx, |ui| {
        let min_w = title_width(ui, "Stato progetti");
        let rows_key = egui::Id::new("project_status_rows");
        egui::ScrollArea::vertical()
            .max_height(project_list_max_height(ui, rows_key))
            .show(ui, |ui| {
                ui.set_min_width(min_w);
                let top = ui.cursor().top();
                egui::Grid::new("project_status_grid")
                    .num_columns(2)
                    .spacing(egui::vec2(12.0, 4.0))
                    .show(ui, |ui| {
                        for (id, label) in &projects {
                            ui.label(label);
                            let current = app.projects.status(*id);
                            let mut status = current;
                            let _combo = egui::ComboBox::from_id_salt(("project_status", id.0))
                                .selected_text(status.label())
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    for s in ProjectStatus::ALL {
                                        let _r = ui.selectable_value(&mut status, s, s.label());
                                        #[cfg(test)]
                                        state.status_item_rects.push(_r.rect);
                                    }
                                });
                            #[cfg(test)]
                            state.status_combo_rects.push(_combo.response.rect);
                            if status != current {
                                actions.push(Action::SetProjectStatus { proj: *id, status });
                            }
                            ui.end_row();
                        }
                    });
                remember_project_rows(ui, rows_key, top, projects.len());
            });
    });

    // click fuori dalla finestra → chiudi (ma non nello stesso frame
    // dell'apertura). Il menu di una combo è un livello a sé: un click su una sua
    // voce risulterebbe "fuori", quindi con un popup aperto non si chiude.
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    let popup_open = egui::Popup::is_any_open(ctx);
    let popup_recent = popup_open || state.closed_filter_popup_was_open;
    state.closed_filter_popup_was_open = popup_open;
    if !open || (!just_opened && clicked_outside && !popup_recent) {
        state.show_closed_filter = false;
    }
}

// ── Clona progetto (Aggiungi ▸ Clona progetto…) ─────────────────────────────

/// Finestra «Clona progetto»: elenco di **tutti** i progetti (anche chiusi),
/// mostrati con la sola tripletta, con ricerca. «Clona» lancia
/// `Action::CloneProject`: la copia prende la tripletta dell'originale più
/// «Copia» (vedi `clone_tripletta`) e lo stesso nome.
pub(crate) fn clone_project_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(cp) = state.clone_project.as_mut() else {
        return;
    };
    let q = cp.search.trim().to_lowercase();
    let listed: Vec<ProjectId> = app
        .projects
        .list_full()
        .into_iter()
        .map(|(id, _, _)| id)
        .filter(|id| q.is_empty() || project_label(app, *id).to_lowercase().contains(&q))
        .collect();
    let rows_key = egui::Id::new("clone_project_rows");
    let mut open = true;
    let mut confirm = false;
    let mut cancel = false;

    egui::Window::new("Clona progetto")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(
                "Scegli il progetto da copiare: la copia include dev, effort, note, \
                 % dichiarate, date, categoria e milestone, ed è sempre attiva.",
            );
            ui.add(
                egui::TextEdit::singleline(&mut cp.search)
                    .hint_text("Cerca tripletta…")
                    .desired_width(320.0),
            );
            egui::ScrollArea::vertical()
                .max_height(project_list_max_height(ui, rows_key))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let top = ui.cursor().top();
                    if listed.is_empty() {
                        ui.label("(nessun progetto)");
                    }
                    for id in &listed {
                        // Solo la tripletta (senza tripletta: il nome, per non
                        // lasciare una riga vuota).
                        let text = project_label(app, *id);
                        if ui.selectable_label(cp.src == Some(*id), text).clicked() {
                            cp.src = Some(*id);
                        }
                    }
                    remember_project_rows(ui, rows_key, top, listed.len());
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(cp.src.is_some(), egui::Button::new("Clona"))
                    .clicked()
                {
                    confirm = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });

    if confirm {
        if let Some(src) = cp.src {
            actions.push(Action::CloneProject { src });
        }
    }
    if confirm || cancel || !open {
        state.clone_project = None;
    }
}

// ── Dev nei totali del footer (Filtri ▸ Dev nei totali…) ────────────────────

/// Elenca **tutti** i dev con una spunta: tolta, il dev sparisce dai totali in
/// basso a sinistra del footer (riga e somma «Totale») ma resta ovunque
/// altrove. È l'unico punto da cui rimettere un dev già nascosto.
pub(crate) fn dev_totals_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_dev_totals {
        return;
    }
    let devs = app.devs.list();
    let mut open = true;
    let just_opened = state.dev_totals_just_opened;
    state.dev_totals_just_opened = false;

    let resp = egui::Window::new("Dev nei totali")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(160.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(title_width(ui, "Dev nei totali").max(240.0));
            ui.label(
                "Togli la spunta per escludere un dev dai totali in basso a sinistra \
                 del footer: la sua riga sparisce e le sue ore non entrano nel \
                 «Totale». Resta visibile in tutto il resto del programma.",
            );
            ui.add_space(4.0);
            ui.separator();
            if devs.is_empty() {
                ui.label("Nessun dev.");
            }
            let currently_all = devs
                .iter()
                .all(|(id, _)| !app.devs.is_hidden_in_totals(*id));
            if !devs.is_empty() {
                if let Some(all) = select_all_checkbox(ui, currently_all) {
                    for (id, _) in &devs {
                        actions.push(Action::SetDevHiddenInTotals {
                            dev: *id,
                            hidden: !all,
                        });
                    }
                }
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (id, name) in &devs {
                        let mut on = !app.devs.is_hidden_in_totals(*id);
                        let text = egui::RichText::new(name).color(dev_color(app, *id));
                        if ui.checkbox(&mut on, text).changed() {
                            actions.push(Action::SetDevHiddenInTotals {
                                dev: *id,
                                hidden: !on,
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
        state.show_dev_totals = false;
    }
}

// ── Worker: visibilità e ghost (Filtri ▸ Worker (visibilità e ghost)…) ──────

/// Elenca **tutti** i worker con tre spunte: **Footer** (la riga nel footer,
/// `hide_in_footer`), **Elenchi** (la presenza negli elenchi di scelta —
/// filtri, matrice, inserimento multiplo, saturazione —, `show_in_find`) e
/// **Ghost** (`Worker.ghost`, anomalia). Ricerca per nome; Footer ed Elenchi
/// hanno il loro Select All sui worker elencati, Ghost no (marcare tutti come
/// anomalia non ha senso). È l'unico punto sempre raggiungibile per il ghost,
/// anche per i worker nascosti nel footer. Le scelte sono salvate nel file.
pub(crate) fn worker_visibility_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_worker_visibility {
        return;
    }
    let mut open = true;
    let just_opened = state.worker_visibility_just_opened;
    state.worker_visibility_just_opened = false;
    let q = state.worker_visibility_search.trim().to_lowercase();
    let listed: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .collect();

    let resp = egui::Window::new("Worker")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(160.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.label(
                "Footer: la riga del worker in fondo alla finestra. Elenchi: il \
                 worker compare nei filtri, nella matrice, nell'inserimento \
                 multiplo e nella saturazione. Ghost: se inserito negli effort di \
                 un dev viene evidenziato come anomalia (cella viola, nome dev \
                 lampeggiante, barra/linea rossa negli export).",
            );
            ui.add_space(4.0);
            let sf = ui.add(
                egui::TextEdit::singleline(&mut state.worker_visibility_search)
                    .hint_text("Cerca worker…")
                    .desired_width(240.0),
            );
            if just_opened {
                sf.request_focus();
            }
            ui.separator();
            if listed.is_empty() {
                ui.label("(nessun worker)");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("worker_visibility_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            // Intestazione: Select All per Footer ed Elenchi.
                            ui.label(egui::RichText::new("Worker").strong());
                            let all_footer = listed
                                .iter()
                                .all(|(id, _)| !app.workers.is_hidden_in_footer(*id));
                            let mut f = all_footer;
                            if ui.checkbox(&mut f, "Footer").changed() {
                                for (id, _) in &listed {
                                    actions.push(Action::SetWorkerHiddenInFooter {
                                        worker: *id,
                                        hidden: !f,
                                    });
                                }
                            }
                            let all_lists =
                                listed.iter().all(|(id, _)| app.workers.is_shown_in_find(*id));
                            let mut l = all_lists;
                            if ui.checkbox(&mut l, "Elenchi").changed() {
                                for (id, _) in &listed {
                                    actions.push(Action::SetWorkerShownInFind {
                                        worker: *id,
                                        shown: l,
                                    });
                                }
                            }
                            ui.label(egui::RichText::new("Ghost").strong());
                            ui.end_row();

                            for (id, name) in &listed {
                                ui.label(name);
                                let mut in_footer = !app.workers.is_hidden_in_footer(*id);
                                if ui.checkbox(&mut in_footer, "").changed() {
                                    actions.push(Action::SetWorkerHiddenInFooter {
                                        worker: *id,
                                        hidden: !in_footer,
                                    });
                                }
                                let mut in_lists = app.workers.is_shown_in_find(*id);
                                if ui.checkbox(&mut in_lists, "").changed() {
                                    actions.push(Action::SetWorkerShownInFind {
                                        worker: *id,
                                        shown: in_lists,
                                    });
                                }
                                let mut ghost = app.workers.is_ghost(*id);
                                if ui.checkbox(&mut ghost, "").changed() {
                                    actions.push(Action::SetWorkerGhost {
                                        worker: *id,
                                        ghost,
                                    });
                                }
                                ui.end_row();
                            }
                        });
                });
        });

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || (!just_opened && clicked_outside) {
        state.show_worker_visibility = false;
        state.worker_visibility_search.clear();
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
pub(crate) fn make_move_params(
    app: &App,
    proj: ProjectId,
    moves: Vec<(DevId, Vec<WeekId>)>,
) -> MoveDialog {
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

pub(crate) fn popup_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
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
