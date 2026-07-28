//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Colonne del cruscotto: per settimana → una colonna per settimana; per mese →
/// le settimane raggruppate per mese (etichetta `yy-mm`).
pub(crate) fn saturation_columns(app: &App, monthly: bool) -> Vec<(String, Vec<i32>)> {
    let weeks = weeks_vec(app);
    if !monthly {
        return weeks
            .into_iter()
            .map(|w| {
                let d = primo_giorno_settimana_corrente(&days_to_local(w));
                (d.format("%y-%m-%d").to_string(), vec![w])
            })
            .collect();
    }
    let mut out: Vec<(String, Vec<i32>)> = Vec::new();
    for w in weeks {
        let d = primo_giorno_settimana_corrente(&days_to_local(w));
        let key = format!("{:02}-{:02}", d.year() % 100, d.month());
        match out.last_mut() {
            Some(last) if last.0 == key => last.1.push(w),
            _ => out.push((key, vec![w])),
        }
    }
    out
}

/// Metriche di una cella: ore allocate e capacità (somma su tutte le settimane
/// del periodo). La capacità è il max ore effettivo della settimana: ferie e
/// malattia NON azzerano la capacità (si possono fare 2 giorni di ferie e
/// lavorare gli altri) — il conteggio orario resta quello reale.
pub(crate) fn saturation_cell(app: &App, wid: WorkerId, weeks: &[i32]) -> (i32, i32) {
    let mut alloc = 0;
    let mut cap = 0;
    for &w in weeks {
        alloc += app
            .sovra
            .get(&(WeekId(w as usize), wid))
            .map_or(0, |e| e.0 as i32);
        cap += app.workers.get_effective_max_hours(wid, w as usize) as i32;
    }
    (alloc, cap)
}

pub(crate) fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Colori (sfondo, testo) di una cella heatmap in base alla saturazione.
pub(crate) fn heat_colors(alloc: i32, cap: i32) -> (Color32, Color32) {
    if alloc == 0 {
        return (bg(), text_dim()); // scarico
    }
    if alloc > cap {
        return (g(Color32::from_rgb(0xcc, 0x30, 0x30)), Color32::WHITE); // sovra
    }
    let r = if cap > 0 {
        alloc as f32 / cap as f32
    } else {
        1.0
    };
    // verde (poco carico) → giallo (quasi pieno)
    let c = lerp_color(
        Color32::from_rgb(0x2e, 0x7d, 0x32),
        Color32::from_rgb(0xc9, 0xa0, 0x00),
        r,
    );
    (g(c), Color32::WHITE)
}

/// Cella colorata della heatmap. `current` evidenzia la settimana corrente con un
/// bordo verde; `selected` (riga worker selezionata) con un bordo arancione che
/// prevale, così tutta la riga risulta evidenziata; le altre hanno un bordo tenue.
pub(crate) fn heat_cell(
    ui: &mut egui::Ui,
    label: &str,
    bgc: Color32,
    fg: Color32,
    current: bool,
    selected: bool,
    tip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, 16.0), Sense::click());
    ui.painter().rect_filled(rect, 2.0, bgc);
    // Riga selezionata → bordo arancione su tutte le celle (prevale). Altrimenti
    // settimana corrente → bordo verde; le altre un bordo tenue.
    let stroke = if selected {
        Stroke::new(2.0, g(EFFORT_ORANGE))
    } else if current {
        Stroke::new(2.5, g(Color32::from_rgb(0x00, 0xe0, 0x4b)))
    } else {
        Stroke::new(0.5, text_faint())
    };
    ui.painter()
        .rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    if !label.is_empty() {
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, label, mono(8.5), fg);
    }
    resp.on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Finestra "Saturazione worker": heatmap worker × (settimana|mese), colorata per
/// saturazione (allocato / capacità). Mostra solo i worker visibili nel footer.
pub(crate) fn saturation_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_saturation {
        return;
    }
    let this_week = state.this_week;
    // Worker mostrati nel cruscotto: quelli con `show_in_find` attivo OPPURE non
    // nascosti nel footer. Non dipende dal filtro worker (Ctrl+F).
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| {
            app.workers.is_shown_in_find(*id) || !app.workers.is_hidden_in_footer(*id)
        })
        .collect();

    let mut columns = saturation_columns(app, state.saturation_monthly);
    if state.saturation_future_only {
        // Tieni la settimana corrente e quelle successive.
        columns.retain(|(_, wks)| wks.iter().any(|&w| w >= this_week));
    }

    // Riepilogo (sovra-allocazioni e ore in eccesso) sui dati mostrati.
    let mut over_n = 0;
    let mut total_excess = 0;
    for (wid, _) in &workers {
        for (_, wks) in &columns {
            let (a, c) = saturation_cell(app, *wid, wks);
            if a > c && a > 0 {
                over_n += 1;
                total_excess += a - c;
            }
        }
    }

    let mut open = true;
    let mut jump_week: Option<i32> = None;

    egui::Window::new("Saturazione worker")
        .collapsible(true)
        // Larghezza ridimensionabile, altezza no.
        .resizable([true, false])
        .default_width(900.0)
        .default_pos(egui::pos2(80.0, 60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Vista:");
                ui.selectable_value(&mut state.saturation_monthly, false, "Settimana");
                ui.selectable_value(&mut state.saturation_monthly, true, "Mese");
                ui.separator();
                ui.checkbox(
                    &mut state.saturation_future_only,
                    "Solo da settimana corrente",
                );
            });
            ui.label(format!(
                "Sovra-allocazioni: {over_n}  ·  Ore in eccesso totali: {total_excess}"
            ));
            ui.separator();
            ui.label(
                "Heatmap (allocato / capacità): verde=libero, giallo=pieno, rosso=oltre; \
                 bordo verde = settimana corrente",
            );

            // Altezza uguale delle righe nelle due griglie affiancate (colonna fissa
            // + heatmap), così restano allineate riga per riga. Il valore supera con
            // margine sia le celle (16px) sia il testo, così `min_row_height` fissa
            // ogni riga esattamente a `SAT_RH` in entrambe le griglie.
            const SAT_RH: f32 = 22.0;
            const SAT_H: f32 = 440.0;
            ui.horizontal_top(|ui| {
                // ── Colonna fissa: nome worker + Σ (all/cap). Non scorre in
                //    orizzontale; scorre in verticale insieme alla heatmap (offset
                //    condiviso, barra nascosta). Click sul nome = seleziona/deseleziona
                //    la riga effort (multi-selezione).
                egui::ScrollArea::vertical()
                    .id_salt("sat_left")
                    .max_height(SAT_H)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .vertical_scroll_offset(state.sat_scroll_y)
                    .show(ui, |ui| {
                        egui::Grid::new("sat_left_grid")
                            .min_row_height(SAT_RH)
                            .spacing(egui::vec2(8.0, 2.0))
                            .show(ui, |ui| {
                                // Nomi cliccabili senza padding verticale extra: così
                                // la loro altezza non supera `SAT_RH` (righe allineate).
                                ui.spacing_mut().button_padding.y = 0.0;
                                ui.label("");
                                ui.label(egui::RichText::new("Σ  all/cap").strong());
                                ui.end_row();

                                for (wid, wname) in &workers {
                                    let mut tot_a = 0;
                                    let mut tot_c = 0;
                                    let mut over_w = 0;
                                    for (_, wks) in &columns {
                                        let (a, c) = saturation_cell(app, *wid, wks);
                                        tot_a += a;
                                        tot_c += c;
                                        if a > c {
                                            over_w += 1;
                                        }
                                    }
                                    // Nome cliccabile: toggle della selezione riga.
                                    let sel = state.sat_selected.contains(wid);
                                    if ui
                                        .selectable_label(
                                            sel,
                                            egui::RichText::new(wname).monospace(),
                                        )
                                        .on_hover_text("Click: seleziona/deseleziona la riga")
                                        .clicked()
                                    {
                                        if sel {
                                            state.sat_selected.remove(wid);
                                        } else {
                                            state.sat_selected.insert(*wid);
                                        }
                                    }
                                    let tcol = if over_w > 0 {
                                        g(Color32::from_rgb(0xcc, 0x30, 0x30))
                                    } else {
                                        text()
                                    };
                                    // Anche la cella Σ risalta quando la riga è
                                    // selezionata (stesso sfondo del nome).
                                    let mut sigma_rt = egui::RichText::new(format!(
                                        "{tot_a}/{tot_c}  ({over_w}⚠)"
                                    ))
                                    .color(tcol)
                                    .monospace();
                                    if state.sat_selected.contains(wid) {
                                        sigma_rt = sigma_rt
                                            .background_color(ui.visuals().selection.bg_fill);
                                    }
                                    ui.label(sigma_rt);
                                    ui.end_row();
                                }
                            });
                    });

                // ── Heatmap: scorre in orizzontale (e verticale). Guida l'offset
                //    verticale condiviso con la colonna fissa.
                let out = egui::ScrollArea::both()
                    .id_salt("sat_heat")
                    .max_height(SAT_H)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        egui::Grid::new("sat_grid")
                            .min_row_height(SAT_RH)
                            .spacing(egui::vec2(2.0, 2.0))
                            .show(ui, |ui| {
                                for (lab, wks) in &columns {
                                    let mut rt = egui::RichText::new(lab).monospace().small();
                                    if wks.contains(&this_week) {
                                        rt = rt.strong().color(Color32::BLACK).background_color(g(
                                            Color32::from_rgb(0x00, 0xe0, 0x4b),
                                        ));
                                    }
                                    ui.label(rt);
                                }
                                ui.end_row();

                                for (wid, wname) in &workers {
                                    let sel = state.sat_selected.contains(wid);
                                    for (lab, wks) in &columns {
                                        let (alloc, cap) = saturation_cell(app, *wid, wks);
                                        let (bgc, fg) = heat_colors(alloc, cap);
                                        let is_cur = wks.contains(&this_week);
                                        let txt = if alloc == 0 {
                                            String::new()
                                        } else {
                                            alloc.to_string()
                                        };
                                        let tip = format!("{wname} · {lab}\n{alloc} / {cap} h");
                                        if heat_cell(ui, &txt, bgc, fg, is_cur, sel, &tip).clicked()
                                        {
                                            jump_week = Some(wks[0]);
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                // La heatmap comanda: la colonna fissa seguirà questo offset.
                state.sat_scroll_y = out.state.offset.y;
            });
        });

    // Click su una cella → scrolla la griglia a quella settimana. Per capire quale
    // settimana è selezionata basta spostare la finestra (non è ridimensionabile).
    if let Some(w) = jump_week {
        let level = if state.compact_mode {
            0
        } else {
            state.zoom_level
        };
        let cols = columns_vec(app, level);
        let cw = col_w(state.compact_mode);
        if let Some(idx) = cols.iter().position(|c| c.contains_week(w)) {
            state.pending_scroll_x = Some((col_x_offset(&cols, idx, cw) - 80.0).max(0.0));
        }
    }
    if !open {
        state.show_saturation = false;
    }
}

// ── Corpo: colonna sinistra + griglia con scroll sincronizzato ──────────────
