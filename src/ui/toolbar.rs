//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Riga "campo di testo + bottone +<label>" usata nel menù "Aggiungi".
/// Chiama `on_submit` col nome (non vuoto) alla pressione di Invio o del
/// bottone e svuota il campo; la tendina resta aperta per inserimenti multipli.
pub(crate) fn add_field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    value: &mut String,
    mut on_submit: impl FnMut(String),
) {
    ui.horizontal(|ui| {
        let e = ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(140.0),
        );
        let submit = (e.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.button(format!("+ {label}")).clicked();
        if submit && !value.is_empty() {
            on_submit(std::mem::take(value));
        }
    });
}

pub(crate) fn toolbar(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    egui::menu::bar(ui, |ui| {
        // ── File ─────────────────────────────────────────────────────────────
        ui.menu_button("File", |ui| {
            if ui.button("Salva").clicked() {
                actions.push(Action::Save);
                ui.close_menu();
            }
            if ui.button("Apri…").clicked() {
                actions.push(Action::Open);
                ui.close_menu();
            }
            if ui
                .button("Confronta/Importa progetto…")
                .on_hover_text(
                    "Confronta i progetti con un altro file .ron e importa i dev scelti",
                )
                .clicked()
            {
                actions.push(Action::OpenCompare);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Esporta…").clicked() {
                actions.push(Action::ExportPdf);
                ui.close_menu();
            }
            if ui
                .button("Andamento…")
                .on_hover_text("PDF dell'andamento nel tempo di % presunta e dichiarata, una pagina per progetto")
                .clicked()
            {
                actions.push(Action::ExportTrend);
                ui.close_menu();
            }
            if ui.button("Minuta…").clicked() {
                // Apre la dialog con i progetti del corpo centrale (abilitati +
                // modalità Vista corrente) preselezionati.
                state.minuta = Some(MinutaState {
                    entries: body_projects(app, state.project_view)
                        .into_iter()
                        .map(|id| (id, true))
                        .collect(),
                    only_current: true,
                    only_with_notes: true,
                });
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Esci").clicked() {
                // Richiede la chiusura: se ci sono modifiche non salvate,
                // `handle_exit` intercetta e mostra la conferma; altrimenti esce.
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                ui.close_menu();
            }
        });

        // ── Aggiungi ─────────────────────────────────────────────────────────
        // I campi di testo restano nel menù: non chiudiamo la tendina dopo un
        // inserimento, così si possono aggiungere più elementi di seguito.
        // Con il comportamento predefinito (`CloseOnClick`) un click su una
        // casella di testo interna chiuderebbe subito la tendina, impedendo di
        // digitare: usiamo `CloseOnClickOutside` così i click sui campi e sui
        // bottoni "+ …" la lasciano aperta.
        egui::containers::menu::MenuButton::new("Aggiungi")
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                if ui.button("+ Progetto").clicked() {
                    actions.push(Action::NewProject);
                    ui.close_menu();
                }
                ui.separator();

                add_field(
                    ui,
                    "Worker",
                    "Nome worker…",
                    &mut state.new_worker,
                    |name| {
                        actions.push(Action::AddWorker(name));
                    },
                );
                add_field(ui, "Dev", "Nome dev…", &mut state.new_dev, |name| {
                    actions.push(Action::AddDev(name));
                });
                add_field(
                    ui,
                    "Categoria",
                    "Nome categoria…",
                    &mut state.new_category,
                    |name| actions.push(Action::AddCategory(name)),
                );
                add_field(
                    ui,
                    "Milestone",
                    "Nome milestone…",
                    &mut state.new_milestone,
                    |name| actions.push(Action::CreateMilestone(name)),
                );
            });

        // ── Filtri ───────────────────────────────────────────────────────────
        ui.menu_button("Filtri", |ui| {
            if ui.button("Progetti…  (⌘/Ctrl+P)").clicked() {
                state.show_project_filter = !state.show_project_filter;
                state.project_filter_just_opened = state.show_project_filter;
                ui.close_menu();
            }
            // La spunta segnala che un filtro worker è attivo.
            if ui
                .selectable_label(state.worker_filter.is_some(), "Workers…  (⌘/Ctrl+F)")
                .clicked()
            {
                state.show_worker_filter = !state.show_worker_filter;
                state.worker_filter_just_opened = state.show_worker_filter;
                ui.close_menu();
            }
            if ui.button("Milestone…").clicked() {
                state.show_milestone_manager = !state.show_milestone_manager;
                state.milestone_manager_just_opened = state.show_milestone_manager;
                ui.close_menu();
            }
            if ui.button("Ghost worker…").clicked() {
                state.show_ghost_manager = !state.show_ghost_manager;
                state.ghost_manager_just_opened = state.show_ghost_manager;
                ui.close_menu();
            }
            let closed_btn = ui.button("Closed…");
            // La finestra "Closed" si ancora sotto questa voce di menù.
            state.closed_btn_pos = closed_btn.rect.left_bottom();
            if closed_btn.clicked() {
                state.show_closed_filter = !state.show_closed_filter;
                state.closed_filter_just_opened = state.show_closed_filter;
                ui.close_menu();
            }
        });

        // ── Vista ────────────────────────────────────────────────────────────
        ui.menu_button("Vista", |ui| {
            if ui
                .selectable_label(state.compact_mode, "Vista compatta")
                .clicked()
            {
                state.compact_mode = !state.compact_mode;
                ui.close_menu();
            }
            if ui.selectable_label(state.bw_mode, "Bianco/Nero").clicked() {
                state.bw_mode = !state.bw_mode;
                ui.close_menu();
            }
            ui.separator();
            // Modalità progetti: filtra il corpo centrale (e di riflesso gli
            // export) tra soli aperti / soli chiusi / tutti. Non persistita.
            ui.label(
                egui::RichText::new("Progetti")
                    .strong()
                    .color(g(EFFORT_ORANGE)),
            );
            if ui
                .selectable_label(
                    state.project_view == ProjectViewMode::Open,
                    "Solo progetti aperti  (⌘/Ctrl+1)",
                )
                .clicked()
            {
                state.project_view = ProjectViewMode::Open;
                ui.close_menu();
            }
            if ui
                .selectable_label(
                    state.project_view == ProjectViewMode::Closed,
                    "Solo progetti chiusi  (⌘/Ctrl+2)",
                )
                .clicked()
            {
                state.project_view = ProjectViewMode::Closed;
                ui.close_menu();
            }
            if ui
                .selectable_label(
                    state.project_view == ProjectViewMode::All,
                    "Tutti  (⌘/Ctrl+3)",
                )
                .clicked()
            {
                state.project_view = ProjectViewMode::All;
                ui.close_menu();
            }
            ui.separator();
            // Tema chiaro/scuro: Auto segue il sistema (macOS "Automatico" =
            // chiaro di giorno, scuro la sera), oppure forzato manualmente.
            ui.label(egui::RichText::new("Tema").strong().color(g(EFFORT_ORANGE)));
            if ui
                .selectable_label(state.theme_pref == ThemePref::Auto, "Auto (sistema)")
                .clicked()
            {
                state.theme_pref = ThemePref::Auto;
                ui.close_menu();
            }
            if ui
                .selectable_label(state.theme_pref == ThemePref::Light, "Chiaro")
                .clicked()
            {
                state.theme_pref = ThemePref::Light;
                ui.close_menu();
            }
            if ui
                .selectable_label(state.theme_pref == ThemePref::Dark, "Scuro")
                .clicked()
            {
                state.theme_pref = ThemePref::Dark;
                ui.close_menu();
            }
            ui.separator();
            // Zoom settimane: mergia 2 o 4 settimane (somma effort, sola lettura).
            // Disponibile solo in vista normale; ignorato in vista compatta.
            ui.add_enabled_ui(!state.compact_mode, |ui| {
                ui.label(
                    egui::RichText::new("Zoom settimane")
                        .strong()
                        .color(g(EFFORT_ORANGE)),
                );
                if ui
                    .selectable_label(state.zoom_level == 0, "Normale")
                    .clicked()
                {
                    state.zoom_level = 0;
                }
                if ui
                    .selectable_label(state.zoom_level == 1, "2 settimane")
                    .clicked()
                {
                    state.zoom_level = 1;
                }
                if ui
                    .selectable_label(state.zoom_level == 2, "4 settimane")
                    .clicked()
                {
                    state.zoom_level = 2;
                }
            });
            ui.separator();
            if ui.button("Saturazione worker…").clicked() {
                state.show_saturation = true;
                ui.close_menu();
            }
        });

        // ── Aiuto ────────────────────────────────────────────────────────────
        ui.menu_button("Aiuto", |ui| {
            if ui.button("Manuale d'uso…").clicked() {
                state.show_help = true;
                ui.close_menu();
            }
        });

        // I selettori Anno e Categoria sono stati spostati nel footer sinistro
        // (vedi `draw_left_footer`): l'anno sopra i totali-anno per dev, la
        // categoria sopra la colonna dei nomi dev.

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let col = if state.changed {
                g(EFFORT_ORANGE)
            } else {
                text_dim()
            };
            let file_name = std::path::Path::new(&state.current_file)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| state.current_file.clone());
            let label = format!("{}{}", file_name, if state.changed { " (*)" } else { "" });
            ui.colored_label(col, label)
                .on_hover_text(&state.current_file);
            ui.separator();
            ui.weak(format!("v{}", env!("CARGO_PKG_VERSION")));

            // Notifica di aggiornamento esterno applicato in automatico.
            if let Some(msg) = state.external_notice.clone() {
                ui.separator();
                if ui.small_button("✕").on_hover_text("Nascondi").clicked() {
                    state.external_notice = None;
                }
                ui.colored_label(
                    g(egui::Color32::from_rgb(90, 200, 250)),
                    egui::RichText::new(msg).strong(),
                );
            }
        });
    });
}

// ── Header (date settimane) ─────────────────────────────────────────────────

pub(crate) fn header(ui: &mut egui::Ui, app: &App, state: &mut UiState) {
    // zoom attivo solo in vista normale
    let level = if state.compact_mode {
        0
    } else {
        state.zoom_level
    };
    let cols = columns_vec(app, level);
    // Riserva i 300px sinistri con lo stesso meccanismo della griglia (SidePanel),
    // così l'origine X delle colonne coincide esattamente.
    egui::SidePanel::left("hdr_left")
        .exact_width(LEFT_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(bg()))
        .show_inside(ui, |_ui| {});

    egui::ScrollArea::horizontal()
        .id_salt("header_scroll")
        .scroll_offset(Vec2::new(state.scroll_x, 0.0))
        .enable_scrolling(false)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let compact = state.compact_mode;
            let cw = col_w(compact);
            let content_w = cols_width(&cols, cw);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, ROW_H), Sense::hover());
            let mut x = rect.left();
            for c in cols.iter() {
                let colw = col_width(c, cw);
                let cell = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(colw, ROW_H));
                x += colw;
                let ws = match c {
                    Col::YearEnd(_) => {
                        // colonna di confine: sfondo canarino + titolo "Effort residuo"
                        ui.painter().rect_filled(cell, 0.0, g(CANARY));
                        draw_boundary_title(ui, cell);
                        continue;
                    }
                    Col::Weeks(ws) => ws,
                };
                let Some(w) = ws.first().copied() else {
                    continue;
                };
                let merged = ws.len() > 1;
                if ws.contains(&state.this_week) {
                    ui.painter()
                        .rect_filled(cell, 0.0, g(THIS_WEEK).gamma_multiply(0.5));
                }
                // etichetta = data della prima settimana del gruppo
                let txt = primo_giorno_settimana_corrente(&days_to_local(w))
                    .format("%y-%m-%d")
                    .to_string();
                // Click sulla data → popup limite ore settimana per TUTTI i worker.
                let mut resp = ui
                    .interact(cell, egui::Id::new(("hdr", w)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if compact {
                    // colonne troppo strette per il testo → la data nel tooltip
                    resp = resp.on_hover_text(txt.clone());
                } else {
                    ui.painter().text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        &txt,
                        cell_font(),
                        text(),
                    );
                }
                if merged {
                    // gruppo mergiato: sola lettura. Tooltip con l'intervallo di date.
                    if let Some(last) = ws.last().copied() {
                        let end = primo_giorno_settimana_corrente(&days_to_local(last))
                            .format("%y-%m-%d")
                            .to_string();
                        resp.on_hover_text(format!("{txt} … {end}"));
                    }
                } else if resp.clicked() {
                    state.popup = Some(Popup::BulkWeekMax {
                        week: w as usize,
                        date: txt,
                        text: DEFAULT_MAX_HOURS.to_string(),
                    });
                }
            }
        });
}

// ── Cruscotto saturazione worker (Vista ▸ Saturazione worker…) ──────────────

