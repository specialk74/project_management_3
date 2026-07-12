//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Disegna una mini-anteprima (foglio bianco stile Gantt) di come vengono rese
/// le barre di un dev nel formato `fmt`, sugli stessi dati campione. Serve nel
/// selettore del formato di export per far scegliere l'utente a colpo d'occhio.
pub(crate) fn draw_bar_format_preview(ui: &egui::Ui, rect: Rect, fmt: crate::pdf_export::BarFormat) {
    use crate::pdf_export::BarFormat;
    let p = ui.painter();
    // Foglio bianco col bordo (indipendente dal tema, come il PDF stampato).
    p.rect_filled(rect, 3.0, Color32::WHITE);
    p.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, Color32::from_gray(200)),
        egui::StrokeKind::Inside,
    );

    // Geometria ricavata dalla dimensione del riquadro, così scala con esso.
    let pad = (rect.height() * 0.12).clamp(5.0, 10.0);
    let x0 = rect.left() + pad;
    let x1 = rect.right() - pad;
    let span = 181.0;
    let xof = |d: f32| x0 + (d / span) * (x1 - x0);

    // Banda dei mesi (6 celle grigie alternate) come nell'export.
    let mtop = rect.top() + pad;
    let mband = (rect.height() * 0.17).clamp(7.0, 16.0);
    let mbot = mtop + mband;
    let mstart = [0.0, 31.0, 59.0, 90.0, 120.0, 151.0, 181.0];
    for m in 0..6 {
        let col = if m % 2 == 0 {
            Color32::from_rgb(0x9e, 0x9e, 0x9e)
        } else {
            Color32::from_rgb(0xb8, 0xb8, 0xb8)
        };
        p.rect_filled(
            Rect::from_min_max(
                egui::pos2(xof(mstart[m]), mtop),
                egui::pos2(xof(mstart[m + 1]), mbot),
            ),
            0.0,
            col,
        );
    }

    // Dati campione (gli stessi delle immagini di esempio): giorno, ore. Il
    // riferimento per l'altezza segue la regola dell'export (mai sotto 40h).
    let days = [14.0, 21.0, 28.0, 56.0, 63.0, 91.0, 112.0, 119.0, 126.0];
    let hours = [20.0, 32.0, 32.0, 40.0, 24.0, 12.0, 28.0, 28.0, 16.0];
    let denom = hours.iter().copied().fold(0.0, f32::max).max(40.0);
    let dev = Color32::from_rgb(0x2e, 0x8b, 0x9e);
    let rowtop = mbot + pad * 0.6;
    let rowbot = rect.bottom() - pad;
    let yc = (rowtop + rowbot) * 0.5;
    let bhh = (rowbot - rowtop) * 0.42;

    let bar = |a: f32, b: f32, hh: f32| {
        let r = Rect::from_min_max(
            egui::pos2(xof(a), yc - hh),
            egui::pos2(xof(b).max(xof(a) + 1.5), yc + hh),
        );
        p.rect_filled(r, 1.0, dev);
    };

    match fmt {
        // Un unico rettangolo dal primo all'ultimo effort (+1 settimana).
        BarFormat::Continuous => bar(days[0], days[days.len() - 1] + 7.0, bhh),
        // Un rettangolo per tratto di settimane consecutive, con i gap.
        BarFormat::Segmented => {
            for (a, b) in [
                (14.0, 28.0 + 7.0),
                (56.0, 63.0 + 7.0),
                (91.0, 91.0 + 7.0),
                (112.0, 126.0 + 7.0),
            ] {
                bar(a, b, bhh);
            }
        }
        // Un rettangolo per settimana, altezza ∝ ore/riferimento.
        BarFormat::Proportional => {
            for i in 0..days.len() {
                bar(days[i], days[i] + 7.0, bhh * hours[i] / denom);
            }
        }
    }
}

/// Selettore del formato barre (Continuo / Segmentato / Proporzionale): le tre
/// opzioni sono **affiancate**, ognuna con anteprima + radio, così l'insieme
/// resta compatto in altezza (adatto a schermi HD). Condiviso dai dialog di
/// export. Scorre in orizzontale se la finestra è più stretta delle tre colonne.
pub(crate) fn bar_format_selector(ui: &mut egui::Ui, fmt: &mut crate::pdf_export::BarFormat) {
    use crate::pdf_export::BarFormat;
    const COL_W: f32 = 196.0;
    const PREVIEW: egui::Vec2 = egui::vec2(190.0, 56.0);
    ui.label(egui::RichText::new("Formato barre").strong());
    ui.add_space(4.0);
    egui::ScrollArea::horizontal()
        .id_salt("bar_format_row")
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (val, title, desc) in [
                    (
                        BarFormat::Continuous,
                        "Barra continua",
                        "un rettangolo dal primo\nall'ultimo effort",
                    ),
                    (
                        BarFormat::Segmented,
                        "Segmentata",
                        "un rettangolo per tratto,\ni buchi restano vuoti",
                    ),
                    (
                        BarFormat::Proportional,
                        "Segmentata + altezza %",
                        "altezza ∝ all'effort\ndella settimana",
                    ),
                ] {
                    ui.allocate_ui_with_layout(
                        egui::vec2(COL_W, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let (prect, presp) = ui.allocate_exact_size(PREVIEW, Sense::click());
                            draw_bar_format_preview(ui, prect, val);
                            if presp.clicked() {
                                *fmt = val;
                            }
                            ui.horizontal(|ui| {
                                ui.radio_value(fmt, val, "");
                                ui.label(egui::RichText::new(title).strong());
                            });
                            ui.label(egui::RichText::new(desc).small().color(text_dim()));
                        },
                    );
                }
            });
        });
}

/// Dialog "Esporta PDF" per singolo progetto: elenco di TUTTI i dev del progetto
/// (anche senza effort) con checkbox di selezione e frecce ▲▼ per riordinarli.
/// Alla conferma lancia `Action::ExportPdfProject` con i dev selezionati, in ordine.
pub(crate) fn pdf_export_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(px) = state.pdf_export.as_mut() else {
        return;
    };
    // Copia locale del formato: evita conflitti di prestito con `px` dentro la
    // closure della finestra; riscritta in `state` a fine funzione.
    let mut fmt = state.bar_format;
    // Idem per la scelta "includi percentuali di avanzamento" nell'export.
    let mut show_pct = state.export_progress_pct;
    let proj = px.proj;
    let trip = app.projects.get_tripletta(proj);
    let title = if trip.is_empty() {
        "Esporta PDF progetto".to_string()
    } else {
        format!("Esporta PDF: {trip}")
    };

    let mut open = true;
    let mut do_export = false;
    let mut do_export_svg = false;
    let mut cancel = false;

    // Altezza massima della finestra e dell'elenco dev, in base allo schermo,
    // così su monitor HD la dialog resta gestibile (contenuti scrollabili).
    let maxh = (ctx.screen_rect().height() - 90.0).max(320.0);
    let list_max = (ctx.screen_rect().height() * 0.42).max(120.0);

    egui::Window::new("Esporta PDF")
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .max_height(maxh)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(&title).strong());
            ui.add_space(4.0);
            ui.label("Seleziona i dev; trascinali per riordinarli:");
            ui.add_space(4.0);

            // Select All in cima.
            let currently_all = !px.entries.is_empty() && px.entries.iter().all(|(_, s)| *s);
            if let Some(v) = select_all_checkbox(ui, currently_all) {
                for e in px.entries.iter_mut() {
                    e.1 = v;
                }
            }
            ui.separator();

            // Elenco dev con riordino drag & drop (payload = indice di partenza),
            // in un'area scorrevole per non far crescere la finestra oltre lo schermo.
            let mut from: Option<usize> = None;
            let mut to: Option<usize> = None;
            egui::ScrollArea::vertical()
                .id_salt("pdf_dev_list")
                .max_height(list_max)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for i in 0..px.entries.len() {
                        let dev = px.entries[i].0;
                        let mut sel = px.entries[i].1;
                        let inner = ui.horizontal(|ui| {
                            ui.checkbox(&mut sel, "");
                            ui.dnd_drag_source(egui::Id::new(("pdf_drag", proj.0, i)), i, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("≡  {}", dev_name(app, dev)))
                                            .monospace()
                                            .color(dev_color(app, dev)),
                                    )
                                    .selectable(false),
                                );
                            });
                        });
                        px.entries[i].1 = sel;

                        let resp = inner.response;
                        let ptr = ui.input(|i| i.pointer.interact_pos());
                        // Indicatore di inserimento mentre si trascina sopra la riga.
                        if resp.dnd_hover_payload::<usize>().is_some() {
                            if let Some(p) = ptr {
                                let y = if p.y > resp.rect.center().y {
                                    resp.rect.bottom()
                                } else {
                                    resp.rect.top()
                                };
                                ui.painter().hline(
                                    resp.rect.x_range(),
                                    y,
                                    Stroke::new(2.0, g(EFFORT_ORANGE)),
                                );
                            }
                        }
                        if let Some(payload) = resp.dnd_release_payload::<usize>() {
                            from = Some(*payload);
                            let after = ptr.map(|p| p.y > resp.rect.center().y).unwrap_or(false);
                            to = Some(if after { i + 1 } else { i });
                        }
                    }
                });
            // Applica lo spostamento a fine passata.
            if let (Some(f), Some(t)) = (from, to) {
                if f != t {
                    let item = px.entries.remove(f);
                    let insert = if f < t { t - 1 } else { t };
                    px.entries.insert(insert.min(px.entries.len()), item);
                }
            }

            ui.separator();
            bar_format_selector(ui, &mut fmt);

            ui.separator();
            ui.checkbox(
                &mut show_pct,
                "Includi percentuali di avanzamento (presunta/dichiarata)",
            );

            ui.separator();
            ui.horizontal(|ui| {
                // Esportabile anche con zero dev: esce comunque il resto (milestone…).
                if ui.button("Esporta PDF…").clicked() {
                    do_export = true;
                }
                if ui
                    .button("Esporta SVG…")
                    .on_hover_text("Solo il grafico, senza tripletta/descrizione né data")
                    .clicked()
                {
                    do_export_svg = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });

    // Ricorda il formato scelto per i prossimi export.
    state.bar_format = fmt;
    state.export_progress_pct = show_pct;

    // Raccolgo i dev selezionati (prestito di `px`) prima di modificare lo stato.
    let devs_if_export = (do_export || do_export_svg).then(|| {
        px.entries
            .iter()
            .filter(|(_, s)| *s)
            .map(|(d, _)| *d)
            .collect::<Vec<_>>()
    });

    if let Some(devs) = devs_if_export {
        if do_export_svg {
            actions.push(Action::ExportSvgProject { proj, devs });
        } else {
            actions.push(Action::ExportPdfProject { proj, devs });
        }
        state.pdf_export = None;
    } else if cancel || !open {
        state.pdf_export = None;
    }
}

/// Dialog "Esporta PDF" con più progetti visibili: elenco dei progetti da
/// esportare (una pagina Gantt ciascuno) con "Select All". Alla conferma lancia
/// `Action::ExportPdfSelected` con i progetti selezionati.
pub(crate) fn pdf_multi_export_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(px) = state.pdf_multi_export.as_mut() else {
        return;
    };
    // Copia locale del formato (vedi nota in `pdf_export_window`).
    let mut fmt = state.bar_format;
    let mut show_pct = state.export_progress_pct;
    let mut open = true;
    let mut do_export = false;
    let mut cancel = false;

    // Cap dell'altezza per schermi HD: l'elenco progetti scorre da sé.
    let maxh = (ctx.screen_rect().height() - 90.0).max(320.0);

    egui::Window::new("Esporta PDF")
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .max_height(maxh)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            // Formato in alto (compatto e ad altezza fissa); sotto l'elenco
            // progetti, che scorre e riserva lo spazio per i pulsanti in fondo.
            bar_format_selector(ui, &mut fmt);
            ui.separator();

            ui.label("Seleziona i progetti da esportare:");
            ui.add_space(4.0);
            project_checklist(ui, app, &mut px.entries);

            ui.separator();
            ui.checkbox(
                &mut show_pct,
                "Includi percentuali di avanzamento (presunta/dichiarata)",
            );
            ui.horizontal(|ui| {
                if ui.button("Esporta PDF…").clicked() {
                    do_export = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });

    state.bar_format = fmt;
    state.export_progress_pct = show_pct;

    if do_export {
        let projects: Vec<ProjectId> = px
            .entries
            .iter()
            .filter(|(_, s)| *s)
            .map(|(p, _)| *p)
            .collect();
        actions.push(Action::ExportPdfSelected { projects });
        state.pdf_multi_export = None;
    } else if cancel || !open {
        state.pdf_multi_export = None;
    }
}

// ── Minuta (esportazione note in Markdown) ──────────────────────────────────

/// Dialog "Minuta" (File ▸ Minuta…): seleziona i progetti non chiusi e sceglie
/// se includere solo le note della settimana corrente o tutte; alla conferma
/// lancia `Action::GenerateMinuta`.
pub(crate) fn minuta_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(m) = state.minuta.as_mut() else {
        return;
    };
    let mut open = true;
    let mut generate = false;
    let mut cancel = false;

    egui::Window::new("Minuta")
        .collapsible(false)
        .resizable(true)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Progetti da includere nella minuta:");
            ui.add_space(4.0);

            // Elenco progetti non chiusi (etichetta = tripletta, fallback nome).
            project_checklist(ui, app, &mut m.entries);

            ui.separator();
            ui.label("Note da includere:");
            ui.radio_value(&mut m.only_current, false, "Tutte le note");
            ui.radio_value(
                &mut m.only_current,
                true,
                "Solo la settimana corrente (ultime note)",
            );

            ui.add_space(4.0);
            ui.checkbox(&mut m.only_with_notes, "Solo progetti con note")
                .on_hover_text(
                    "Se attivo, esclude i progetti senza note nell'ambito scelto; \
                     altrimenti compaiono con un segnaposto.",
                );

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Genera Minuta…").clicked() {
                    generate = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });

    if generate {
        let projects: Vec<ProjectId> = m
            .entries
            .iter()
            .filter(|(_, s)| *s)
            .map(|(p, _)| *p)
            .collect();
        actions.push(Action::GenerateMinuta {
            projects,
            only_current: m.only_current,
            only_with_notes: m.only_with_notes,
        });
        state.minuta = None;
    } else if cancel || !open {
        state.minuta = None;
    }
}

/// Etichetta del progetto nella minuta: la tripletta, o la descrizione se la
/// tripletta è vuota, o infine l'id.
pub(crate) fn minuta_project_label(app: &App, proj: ProjectId) -> String {
    let trip = app.projects.get_tripletta(proj);
    if !trip.is_empty() {
        return trip;
    }
    let name = app.projects.get_info(proj);
    if !name.trim().is_empty() {
        name
    } else {
        format!("Progetto {}", proj.0)
    }
}

/// Costruisce il testo Markdown della minuta per i progetti indicati.
/// `only_current`: se true include solo la nota della settimana corrente, altrimenti
/// tutte le settimane (più recenti prima). `only_with_notes`: se true i progetti
/// senza note nell'ambito scelto vengono esclusi; altrimenti compaiono con un
/// segnaposto.
pub(crate) fn build_minuta(
    app: &App,
    projects: &[ProjectId],
    only_current: bool,
    only_with_notes: bool,
) -> String {
    let current = current_week_id();
    let today = Utc::now().date_naive().format("%y-%m-%d");
    let mut out = format!("# Minuta — {today}\n\n");

    for &proj in projects {
        let notes = app.projects.get_notes(proj);
        // Settimane in ambito, ordinate per data decrescente, senza voci vuote.
        let mut weeks: Vec<(WeekId, String)> = if only_current {
            notes
                .get(&current)
                .map(|t| vec![(current, t.clone())])
                .unwrap_or_default()
        } else {
            notes.into_iter().collect()
        };
        weeks.retain(|(_, t)| !t.trim().is_empty());
        weeks.sort_by(|a, b| b.0.cmp(&a.0));

        // Filtro "Solo progetti con note": salta i progetti vuoti nell'ambito.
        if weeks.is_empty() && only_with_notes {
            continue;
        }

        out.push_str(&format!("## {}\n\n", minuta_project_label(app, proj)));
        if weeks.is_empty() {
            out.push_str("_(nessuna nota)_\n\n");
        } else {
            for (w, t) in weeks {
                let label = days_to_local(w.0 as i32).format("%y-%m-%d");
                out.push_str(&format!("**{label}**\n\n{}\n\n", t.trim_end()));
            }
        }
    }

    out
}

// ── Aiuto / Manuale ─────────────────────────────────────────────────────────

