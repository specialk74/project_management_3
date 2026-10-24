//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Disegna una mini-anteprima (foglio bianco stile Gantt) di come vengono rese
/// le barre di un dev nel formato `fmt`, sugli stessi dati campione. Serve nel
/// selettore del formato di export per far scegliere l'utente a colpo d'occhio.
pub(crate) fn draw_bar_format_preview(
    ui: &egui::Ui,
    rect: Rect,
    fmt: crate::pdf_export::BarFormat,
) {
    use crate::pdf_export::BarFormat;
    let p = ui.painter();
    // Foglio bianco col bordo (indipendente dal tema, come il PDF stampato).
    p.rect_filled(rect, 3.0, Color32::WHITE);
    p.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, Color32::from_gray(200)),
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
/// Restituisce `true` se l'utente ha cliccato un formato (anche quello già
/// scelto): la dialog del singolo progetto lo usa come «applica a tutti».
pub(crate) fn bar_format_selector(
    ui: &mut egui::Ui,
    title: &str,
    fmt: &mut crate::pdf_export::BarFormat,
) -> bool {
    use crate::pdf_export::BarFormat;
    const COL_W: f32 = 196.0;
    const PREVIEW: egui::Vec2 = egui::vec2(190.0, 56.0);
    let mut clicked = false;
    ui.label(egui::RichText::new(title).strong());
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
                                clicked = true;
                            }
                            ui.horizontal(|ui| {
                                if ui.radio_value(fmt, val, "").clicked() {
                                    clicked = true;
                                }
                                ui.label(egui::RichText::new(title).strong());
                            });
                            ui.label(egui::RichText::new(desc).small().color(text_dim()));
                        },
                    );
                }
            });
        });
    clicked
}

/// Voci del menu del formato barra di un dev: la forma e, per le segmentate,
/// la variante con le date sotto ogni tratto.
const DEV_BAR_CHOICES: [(crate::pdf_export::DevBar, &str); 5] = {
    use crate::pdf_export::{BarFormat, DevBar};
    [
        (DevBar::new(BarFormat::Continuous, false), "Barra continua"),
        (DevBar::new(BarFormat::Segmented, false), "Segmentata"),
        (
            DevBar::new(BarFormat::Segmented, true),
            "Segmentata, date sotto",
        ),
        (
            DevBar::new(BarFormat::Proportional, false),
            "Segmentata + altezza %",
        ),
        (
            DevBar::new(BarFormat::Proportional, true),
            "Segmentata + altezza %, date sotto",
        ),
    ]
};

/// Etichetta di un formato barra di dev (per la combo).
fn dev_bar_label(b: crate::pdf_export::DevBar) -> &'static str {
    let b = crate::pdf_export::DevBar::new(b.fmt, b.dates_below());
    DEV_BAR_CHOICES
        .iter()
        .find(|(c, _)| *c == b)
        .map_or("?", |(_, l)| l)
}

/// Combo del formato barra di un singolo dev (dentro una finestra: qui la
/// `ComboBox` va bene, vedi la nota sui menù della toolbar).
fn dev_bar_format_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    bar: &mut crate::pdf_export::DevBar,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(dev_bar_label(*bar))
        .width(220.0)
        .show_ui(ui, |ui| {
            for (c, label) in DEV_BAR_CHOICES {
                ui.selectable_value(bar, c, label);
            }
        });
}

/// Spunta «anno nelle date dei dev», condivisa dalle dialog di export.
pub(crate) fn year_checkbox(ui: &mut egui::Ui, show_year: &mut bool) {
    ui.checkbox(show_year, "Includi l'anno nelle date dei dev ('26)")
        .on_hover_text(
            "Accanto a ogni barra le date diventano ad es. «Sep 22 '26 - Oct 5 '26» \
             invece di «Sep 22 - Oct 5». Le date delle milestone non cambiano.",
        );
}

/// Spunta «Includi i worker ghost», condivisa dalle dialog di export: con la
/// spunta i ghost sono marcati **anche dove sono assegnati a effort 0**, e un
/// dev che ha solo un ghost senza ore viene stampato lo stesso; senza spunta la
/// stampa ignora del tutto i ghost. Sotto, la sotto-opzione del colore
/// (`ExportGhost.red`): rosso con nome dev rosso, oppure colore del dev con
/// nome nero.
pub(crate) fn ghost_checkbox(ui: &mut egui::Ui, ghost: &mut ExportGhost) {
    // Tutto su una riga: spunta + sotto-opzione del colore, attiva solo con la
    // spunta (risparmia una riga nelle dialog).
    ui.horizontal(|ui| {
        ui.checkbox(&mut ghost.show, "Includi i worker ghost (anche a effort 0)")
            .on_hover_text(
                "Con la spunta i worker ghost sono segnati (in rosso o nel colore del \
                 dev) anche nelle settimane in cui sono assegnati senza ore, e il dev \
                 che ha solo un ghost compare comunque. Senza spunta il PDF non li \
                 evidenzia.",
            );
        ui.add_enabled_ui(ghost.show, |ui| {
            ui.label("— colore:");
            ui.radio_value(&mut ghost.red, true, "Rosso")
                .on_hover_text("Settimane ghost e nome del dev in rosso.");
            ui.radio_value(&mut ghost.red, false, "Colore del dev")
                .on_hover_text(
                    "Settimane ghost nel colore del dev e nome nero: il ghost \
                     non è evidenziato come anomalia.",
                );
        });
    });
}

/// Selettore degli **ambiti milestone** da stampare, condiviso dalle dialog di
/// export. «Solo internal» produce un file con le milestone Internal, «Solo
/// external» uno con le External, «Mix» uno con tutte; spuntandone più d'uno si
/// ottiene un file per ambito (stesso nome + `_internal` / `_external` / `_mix`).
pub(crate) fn milestone_scope_selector(ui: &mut egui::Ui, scopes: &mut ExportScopes) {
    // Titolo con accanto l'esito (quanti file / avviso), poi le tre voci su
    // una riga sola.
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Milestone da stampare").strong());
        let n = scopes.list().len();
        if n > 1 {
            let names: Vec<String> = scopes
                .list()
                .iter()
                .map(|s| format!("…_{}", s.file_suffix()))
                .collect();
            ui.label(
                egui::RichText::new(format!("Verranno creati {n} file: {}", names.join(", ")))
                    .small()
                    .color(text_dim()),
            );
        } else if scopes.is_empty() {
            ui.label(
                egui::RichText::new("Seleziona almeno un ambito per esportare.")
                    .small()
                    .color(warn_amber()),
            );
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut scopes.internal, "Solo internal")
            .on_hover_text("Un file con le milestone Internal (anche quelle Int/Ext).");
        ui.checkbox(&mut scopes.external, "Solo external")
            .on_hover_text("Un file con le milestone External (anche quelle Int/Ext).");
        ui.checkbox(&mut scopes.mix, "Mix")
            .on_hover_text("Un file con tutte le milestone, Internal ed External.");
    });
}

/// Riga «Periodo da disegnare: Da [data] A [data] [Inizio/fine progetto]»
/// della dialog di export di un progetto. Ogni data vale per l'intera
/// settimana che la contiene (vedi `pdf_export::ExportOptions::range`). Ritorna
/// `false` se «Da» è dopo «A» (export disabilitato, con avviso).
fn period_row(ui: &mut egui::Ui, app: &App, proj: ProjectId, range: &mut (i32, i32)) -> bool {
    let picker = |ui: &mut egui::Ui, id: &str, day: &mut i32| {
        let mut date = days_to_jiff(*day);
        let r = ui.add(
            egui_extras::DatePickerButton::new(&mut date)
                .id_salt(id)
                .format("%Y-%m-%d"),
        );
        if r.changed()
            && let Some(d) = jiff_to_days(date)
        {
            *day = d;
        }
    };
    let valid = range.0 <= range.1;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Periodo da disegnare").strong());
        ui.label("Da");
        picker(ui, "pdf_range_from", &mut range.0);
        ui.label("A");
        picker(ui, "pdf_range_to", &mut range.1);
        let proj_range = project_export_range(app, proj);
        if ui
            .add_enabled(*range != proj_range, egui::Button::new("Inizio/fine progetto"))
            .on_hover_text("Rimette le date di inizio e fine del progetto.")
            .clicked()
        {
            *range = proj_range;
        }
        if !valid {
            ui.label(
                egui::RichText::new("«Da» è dopo «A».")
                    .small()
                    .color(warn_amber()),
            );
        }
    })
    .response
    .on_hover_text(
        "Solo questa parte del grafico viene disegnata: barre tagliate ai bordi \
         (le date accanto restano quelle reali), milestone fuori periodo omesse. \
         Ogni data vale per l'intera settimana che la contiene.",
    );
    range.0 <= range.1
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
    let mut show_year = state.export_show_year;
    // Idem per gli ambiti milestone (Internal / External) da stampare…
    let mut scopes = state.export_scopes;
    // …e per la stampa dei worker "ghost".
    let mut ghost = state.export_ghost;
    let proj = px.proj;
    let trip = app.projects.get_tripletta(proj);
    let title = if trip.is_empty() {
        "Esporta PDF/SVG".to_string()
    } else {
        format!("Esporta PDF/SVG - {trip}")
    };

    let mut open = true;
    let mut do_export = false;
    let mut do_export_svg = false;
    let mut cancel = false;

    // Altezza massima della finestra e degli elenchi dev/milestone, in base
    // allo schermo, così su monitor HD la dialog resta gestibile (contenuti
    // scrollabili). Gli elenchi mostrano comunque al massimo
    // `MAX_VISIBLE_EXPORT_ROWS` righe ciascuno, poi scorrono.
    let maxh = (ctx.content_rect().height() - 90.0).max(320.0);
    let list_max = (ctx.content_rect().height() * 0.30).max(110.0);
    let dev_rows = egui::Id::new("pdf_dev_list_rows");
    let ms_rows = egui::Id::new("pdf_milestone_list_rows");

    // Id fisso: il titolo cambia col progetto, la posizione/dimensione no.
    egui::Window::new(&title)
        .id(egui::Id::new("pdf_export_window"))
        .collapsible(false)
        .resizable(true)
        .default_width(1120.0)
        .max_height(maxh)
        // Centrata all'apertura ma trascinabile (con `anchor` non si muove).
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .open(&mut open)
        .show(ctx, |ui| {
            // Periodo da disegnare: una riga sola, in cima.
            let valid_range = period_row(ui, app, proj, &mut px.range);
            ui.separator();

            // Elenchi dev e milestone **affiancati**: su schermi HD con
            // ingrandimento (1920×1080 al 125%) impilati rendevano la dialog
            // troppo alta.
            let mut from: Option<usize> = None;
            let mut to: Option<usize> = None;
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                ui.label(egui::RichText::new("Dev — trascinali per riordinarli").strong());
                ui.add_space(4.0);
                // Elenco dev con riordino drag & drop (payload = indice di
                // partenza), in un'area scorrevole.
                egui::ScrollArea::vertical()
                    .id_salt("pdf_dev_list")
                    // +1: anche la riga del Select All sta nell'area scorrevole.
                    .max_height(list_max.min(rows_max_height(
                        ui,
                        dev_rows,
                        MAX_VISIBLE_EXPORT_ROWS + 1,
                    )))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let top = ui.cursor().top();
                        // Select All in cima, dentro lo scroll dei dev.
                        let currently_all =
                            !px.entries.is_empty() && px.entries.iter().all(|(_, s)| *s);
                        if let Some(v) = select_all_checkbox(ui, currently_all) {
                            for e in px.entries.iter_mut() {
                                e.1 = v;
                            }
                        }
                        for i in 0..px.entries.len() {
                            let dev = px.entries[i].0;
                            let mut sel = px.entries[i].1;
                            let mut dfmt = px.formats.get(&dev).copied().unwrap_or(fmt.into());
                            let inner = ui.horizontal(|ui| {
                                ui.checkbox(&mut sel, "");
                                // Formato della barra di questo dev.
                                dev_bar_format_combo(
                                    ui,
                                    ("pdf_dev_fmt", proj.0, dev.0),
                                    &mut dfmt,
                                );
                                ui.dnd_drag_source(
                                    egui::Id::new(("pdf_drag", proj.0, i)),
                                    i,
                                    |ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "≡  {}",
                                                    dev_name(app, dev)
                                                ))
                                                .monospace()
                                                .color(dev_color(app, dev)),
                                            )
                                            .selectable(false),
                                        );
                                    },
                                );
                            });
                            px.formats.insert(dev, dfmt);
                            px.entries[i].1 = sel;

                            let resp = inner.response;
                            let ptr = ui.input(|i| i.pointer.interact_pos());
                            // Indicatore di inserimento mentre si trascina sopra la riga.
                            if resp.dnd_hover_payload::<usize>().is_some()
                                && let Some(p) = ptr
                            {
                                let y = if p.y > resp.rect.center().y {
                                    resp.rect.bottom()
                                } else {
                                    resp.rect.top()
                                };
                                ui.painter().hline(
                                    resp.rect.x_range(),
                                    y,
                                    Stroke::new(2.0_f32, g(EFFORT_ORANGE)),
                                );
                            }
                            if let Some(payload) = resp.dnd_release_payload::<usize>() {
                                from = Some(*payload);
                                let after =
                                    ptr.map(|p| p.y > resp.rect.center().y).unwrap_or(false);
                                to = Some(if after { i + 1 } else { i });
                            }
                        }
                        remember_project_rows(ui, dev_rows, top, px.entries.len() + 1);
                    });

                // Milestone del progetto: si sceglie una per una quali disegnare.
                let ui = &mut cols[1];
                ui.label(egui::RichText::new("Milestone da includere").strong());
                ui.add_space(4.0);
                if px.milestones.is_empty() {
                    ui.label(
                        egui::RichText::new("Nessuna milestone collocata in questo progetto.")
                            .small()
                            .color(text_dim()),
                    );
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt("pdf_milestone_list")
                    // +1 come per i dev: Select All dentro lo scroll, così le
                    // due colonne restano allineate riga per riga.
                    .max_height(list_max.min(rows_max_height(
                        ui,
                        ms_rows,
                        MAX_VISIBLE_EXPORT_ROWS + 1,
                    )))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let top = ui.cursor().top();
                        let all_ms = px.milestones.iter().all(|(_, _, s)| *s);
                        if let Some(v) = select_all_checkbox(ui, all_ms) {
                            for m in px.milestones.iter_mut() {
                                m.2 = v;
                            }
                        }
                        let rows = px.milestones.len() + 1;
                        for (id, week, sel) in px.milestones.iter_mut() {
                            let name = app.milestones.get_name(*id).unwrap_or("?");
                            let kind = app.milestones.get_kind(*id);
                            let date = week_label(week.0 as i32);
                            let cats = app
                                .milestones
                                .get_categories(*id)
                                .iter()
                                .map(|c| c.short())
                                .collect::<Vec<_>>()
                                .join("/");
                            ui.checkbox(
                                sel,
                                egui::RichText::new(format!(
                                    "{} {name}  —  {date}  [{cats}]",
                                    kind.icon()
                                ))
                                .color(u32_to_color(
                                    app.milestones.get_color(*id).unwrap_or(0x808080),
                                )),
                            );
                        }
                        remember_project_rows(ui, ms_rows, top, rows);
                    });
            });
            // Applica lo spostamento a fine passata.
            if let (Some(f), Some(t)) = (from, to)
                && f != t
            {
                let item = px.entries.remove(f);
                let insert = if f < t { t - 1 } else { t };
                px.entries.insert(insert.min(px.entries.len()), item);
            }

            ui.separator();
            // Formato barre a sinistra, opzioni a destra: affiancati per tenere
            // bassa la dialog (vedi nota sugli elenchi qui sopra).
            ui.horizontal_top(|ui| {
                let left = ui.vertical(|ui| {
                    // Click su un formato = lo applica a tutti i dev (poi ogni
                    // riga si può cambiare dalla sua combo).
                    if bar_format_selector(ui, "Formato barre — applica a tutti i dev", &mut fmt)
                    {
                        for f in px.formats.values_mut() {
                            *f = fmt.into();
                        }
                    }
                });
                // Linea divisoria disegnata a mano, alta quanto il contenuto:
                // un `ui.separator()` verticale prenderebbe tutta l'altezza
                // disponibile e allungherebbe la finestra fino a `max_height`.
                let sep_x = left.response.rect.right() + ui.spacing().item_spacing.x;
                ui.add_space(2.0 * ui.spacing().item_spacing.x);
                let right = ui.vertical(|ui| {
                    ui.checkbox(
                        &mut show_pct,
                        "Includi percentuali di avanzamento (presunta/dichiarata)",
                    );
                    year_checkbox(ui, &mut show_year.0);
                    ghost_checkbox(ui, &mut ghost);
                    ui.add_space(4.0);
                    milestone_scope_selector(ui, &mut scopes);
                });
                let top = left.response.rect.top();
                let bottom = left.response.rect.bottom().max(right.response.rect.bottom());
                ui.painter().vline(
                    sep_x,
                    top..=bottom,
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
            });

            ui.separator();
            let can_export = !scopes.is_empty() && valid_range;
            ui.horizontal(|ui| {
                // Esportabile anche con zero dev: esce comunque il resto (milestone…).
                if ui
                    .add_enabled(can_export, egui::Button::new("Esporta PDF…"))
                    .clicked()
                {
                    do_export = true;
                }
                if ui
                    .add_enabled(can_export, egui::Button::new("Esporta SVG…"))
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
    state.export_show_year = show_year;
    state.export_scopes = scopes;
    state.export_ghost = ghost;
    // Ricorda dev (ordine + spunta) e milestone spuntate di questo progetto.
    state.export_project_memory.insert(
        px.proj,
        ExportProjectMemory {
            devs: px.entries.clone(),
            milestones: px.milestones.iter().map(|(id, _, on)| (*id, *on)).collect(),
            formats: px.formats.clone(),
            range: Some(px.range),
        },
    );

    // Raccolgo i dev selezionati (prestito di `px`) prima di modificare lo stato.
    let devs_if_export = (do_export || do_export_svg).then(|| {
        px.entries
            .iter()
            .filter(|(_, s)| *s)
            .map(|(d, _)| (*d, px.formats.get(d).copied().unwrap_or(fmt.into())))
            .collect::<Vec<_>>()
    });

    // Milestone spuntate (stesso prestito di `px` dei dev, prima di azzerarlo).
    let milestones_if_export = (do_export || do_export_svg).then(|| {
        px.milestones
            .iter()
            .filter(|(_, _, s)| *s)
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>()
    });

    if let Some(devs) = devs_if_export {
        let scopes = scopes.list();
        let milestones = milestones_if_export.unwrap_or_default();
        let range = px.range;
        if do_export_svg {
            actions.push(Action::ExportSvgProject {
                proj,
                devs,
                scopes,
                milestones,
                range,
            });
        } else {
            actions.push(Action::ExportPdfProject {
                proj,
                devs,
                scopes,
                milestones,
                range,
            });
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
    let mut show_year = state.export_show_year;
    let mut scopes = state.export_scopes;
    let mut ghost = state.export_ghost;
    let mut open = true;
    let mut do_export = false;
    let mut cancel = false;

    // Cap dell'altezza per schermi HD: l'elenco progetti scorre da sé.
    let maxh = (ctx.content_rect().height() - 90.0).max(320.0);

    egui::Window::new("Esporta PDF")
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .max_height(maxh)
        // Centrata all'apertura ma trascinabile (con `anchor` non si muove).
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .open(&mut open)
        .show(ctx, |ui| {
            // Formato in alto (compatto e ad altezza fissa); sotto l'elenco
            // progetti, che scorre e riserva lo spazio per i pulsanti in fondo.
            bar_format_selector(ui, "Formato barre", &mut fmt);
            ui.add_enabled(
                fmt != crate::pdf_export::BarFormat::Continuous,
                egui::Checkbox::new(
                    &mut state.export_dates_below,
                    "Date sotto ogni tratto (inizio e fine su due righe)",
                ),
            )
            .on_hover_text(
                "Solo per i formati segmentati: le date vanno sotto ogni tratto \
                 invece che a destra della barra (a destra restano le percentuali).",
            );
            ui.separator();

            ui.label("Seleziona i progetti da esportare:");
            ui.add_space(4.0);
            project_checklist(ui, app, &mut px.entries);

            ui.separator();
            ui.checkbox(
                &mut show_pct,
                "Includi percentuali di avanzamento (presunta/dichiarata)",
            );
            year_checkbox(ui, &mut show_year.0);
            ghost_checkbox(ui, &mut ghost);

            ui.separator();
            milestone_scope_selector(ui, &mut scopes);

            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!scopes.is_empty(), egui::Button::new("Esporta PDF…"))
                    .clicked()
                {
                    do_export = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            checklist_dialog_end(ui);
        });

    state.bar_format = fmt;
    state.export_progress_pct = show_pct;
    state.export_show_year = show_year;
    state.export_scopes = scopes;
    state.export_ghost = ghost;
    // Ricorda la spunta di ogni progetto elencato.
    for (p, on) in &px.entries {
        state.export_multi_memory.insert(*p, *on);
    }

    if do_export {
        let projects: Vec<ProjectId> = px
            .entries
            .iter()
            .filter(|(_, s)| *s)
            .map(|(p, _)| *p)
            .collect();
        actions.push(Action::ExportPdfSelected {
            projects,
            scopes: scopes.list(),
        });
        state.pdf_multi_export = None;
    } else if cancel || !open {
        state.pdf_multi_export = None;
    }
}

// ── Minuta (esportazione note in Markdown) ──────────────────────────────────

/// Dialog "Minuta" (File ▸ Minuta…): seleziona i progetti non chiusi e sceglie
/// se includere solo le note della settimana corrente o tutte; alla conferma
/// lancia `Action::GenerateMinuta`.
pub(crate) fn minuta_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(m) = state.minuta.as_mut() else {
        return;
    };
    let mut open = true;
    let mut generate = false;
    let mut cancel = false;

    egui::Window::new("Minuta")
        .collapsible(false)
        .resizable(true)
        // Centrata all'apertura ma trascinabile (con `anchor` non si muove).
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
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
            ui.checkbox(&mut m.worker_notes, "Includi le note dei worker")
                .on_hover_text(
                    "Aggiunge sotto ogni settimana le note delle celle effort \
                     (tasto destro su una cella), con nome del worker e dev.",
                );
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
            checklist_dialog_end(ui);
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
            worker_notes: m.worker_notes,
        });
        state.minuta = None;
    } else if cancel || !open {
        state.minuta = None;
    }
}

/// Dialog "Report" (File ▸ Report…): scelta dei progetti da includere nel PDF
/// di riepilogo (`pdf_export::build_report_pdf`).
pub(crate) fn report_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(r) = state.report.as_mut() else {
        return;
    };
    let mut open = true;
    let mut generate = false;
    let mut cancel = false;

    egui::Window::new("Report")
        .collapsible(false)
        .resizable(true)
        // Centrata all'apertura ma trascinabile (con `anchor` non si muove).
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Progetti da includere nel report:");
            ui.add_space(4.0);
            project_checklist(ui, app, &mut r.entries);
            ui.separator();
            let any = r.entries.iter().any(|(_, s)| *s);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(any, egui::Button::new("Genera Report…"))
                    .clicked()
                {
                    generate = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            checklist_dialog_end(ui);
        });

    if generate {
        let projects: Vec<ProjectId> = r
            .entries
            .iter()
            .filter(|(_, s)| *s)
            .map(|(p, _)| *p)
            .collect();
        actions.push(Action::GenerateReport { projects });
        state.report = None;
    } else if cancel || !open {
        state.report = None;
    }
}

/// Etichetta del progetto nella minuta: la stessa di `project_label`.
pub(crate) fn minuta_project_label(app: &App, proj: ProjectId) -> String {
    project_label(app, proj)
}

/// Una nota di cella effort da riportare nella minuta: chi l'ha scritta, in quale
/// dev e il testo.
struct WorkerNote {
    worker: String,
    dev: String,
    note: String,
}

/// Note delle celle effort di un progetto (`SingleEffort.note`), raggruppate per
/// settimana. Dentro la settimana l'ordine è quello dei dev del progetto e, a
/// parità di dev, il nome del worker. Note vuote e worker "zero" sono ignorati;
/// le settimane senza note non compaiono nella mappa.
fn project_worker_notes(app: &App, proj: ProjectId) -> HashMap<WeekId, Vec<WorkerNote>> {
    let mut out: HashMap<WeekId, Vec<WorkerNote>> = HashMap::new();
    for dev in app.projects.list_devs(proj) {
        let Some(sd) = app.projects.get_single_dev(proj, dev) else {
            continue;
        };
        let dname = dev_name(app, dev);
        for week in sd.get_weeks() {
            let Some(sew) = sd.get_all(week) else {
                continue;
            };
            let mut here: Vec<WorkerNote> = sew
                .worker_id
                .iter()
                .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                .filter_map(|(wid, se)| {
                    let note = se.get_note();
                    (!note.trim().is_empty()).then(|| WorkerNote {
                        worker: app.workers.get_name_by_id(*wid).to_string(),
                        dev: dname.clone(),
                        note,
                    })
                })
                .collect();
            here.sort_by(|a, b| a.worker.cmp(&b.worker));
            if !here.is_empty() {
                out.entry(week).or_default().append(&mut here);
            }
        }
    }
    out
}

/// Voce di elenco per una nota worker: le righe successive alla prima sono
/// rientrate di due spazi, così una nota multi-riga resta dentro il punto elenco.
fn worker_note_bullet(n: &WorkerNote) -> String {
    let body = n.note.trim().replace('\n', "\n  ");
    format!("- **{}** ({}): {body}\n", n.worker, n.dev)
}

/// Costruisce il testo Markdown della minuta per i progetti indicati.
/// `only_current`: se true include solo la settimana corrente, altrimenti tutte le
/// settimane (più recenti prima). `only_with_notes`: se true i progetti senza note
/// nell'ambito scelto vengono esclusi; altrimenti compaiono con un segnaposto.
/// `worker_notes`: se true sotto la nota di ogni settimana viene aggiunto l'elenco
/// delle note delle celle effort (una riga per worker); una settimana con **solo**
/// note dei worker compare comunque, e quelle note contano anche per il filtro
/// `only_with_notes`.
pub(crate) fn build_minuta(
    app: &App,
    projects: &[ProjectId],
    only_current: bool,
    only_with_notes: bool,
    worker_notes: bool,
) -> String {
    let current = current_week_id();
    let today = today().format("%y-%m-%d");
    let mut out = format!("# Minuta — {today}\n\n");

    for &proj in projects {
        let notes = app.projects.get_notes(proj);
        let mut wnotes = if worker_notes {
            project_worker_notes(app, proj)
        } else {
            HashMap::new()
        };

        // Settimane in ambito: quelle con nota di progetto più quelle con note dei
        // worker (una settimana può avere solo queste ultime). Più recenti prima.
        let mut keys: Vec<WeekId> = notes
            .keys()
            .chain(wnotes.keys())
            .copied()
            .filter(|w| !only_current || *w == current)
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys.reverse();

        // Testo + note worker di ogni settimana, saltando quelle senza nulla.
        let weeks: Vec<(WeekId, String, Vec<WorkerNote>)> = keys
            .into_iter()
            .map(|w| {
                let text = notes.get(&w).cloned().unwrap_or_default();
                (w, text, wnotes.remove(&w).unwrap_or_default())
            })
            .filter(|(_, text, wn)| !text.trim().is_empty() || !wn.is_empty())
            .collect();

        // Filtro "Solo progetti con note": salta i progetti vuoti nell'ambito.
        if weeks.is_empty() && only_with_notes {
            continue;
        }

        out.push_str(&format!("## {}\n\n", minuta_project_label(app, proj)));
        if weeks.is_empty() {
            out.push_str("_(nessuna nota)_\n\n");
        } else {
            for (w, text, wn) in weeks {
                let label = week_label(w.0 as i32);
                out.push_str(&format!("**{label}**\n\n"));
                if !text.trim().is_empty() {
                    out.push_str(&format!("{}\n\n", text.trim_end()));
                }
                if !wn.is_empty() {
                    for n in &wn {
                        out.push_str(&worker_note_bullet(n));
                    }
                    out.push('\n');
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P1 ha una nota di progetto nella settimana corrente + note di cella per due
    /// worker; nella settimana precedente ha **solo** una nota di cella. P2 non ha
    /// note di progetto, solo una nota di cella.
    fn app_with_notes() -> (App, ProjectId, ProjectId) {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let front = app.devs.add("Frontend");
        let cur = current_week_id();
        let prev = WeekId(cur.0 - WEEK_STEP);

        let p1 = app.projects.add("P1", Some("AAA"), Some(cur));
        app.projects.add_dev(p1, front);
        app.projects.set_notes(
            p1,
            HashMap::from([(cur, "Riunione settimanale".to_string())]),
        );
        app.projects.add_effort(p1, front, cur, alice, Effort(8));
        app.projects.add_effort(p1, front, cur, bob, Effort(4));
        app.projects.add_effort(p1, front, prev, alice, Effort(8));
        app.projects
            .set_note(p1, front, cur, alice, "serve conferma API");
        app.projects
            .set_note(p1, front, cur, bob, "blocco su migrazione");
        app.projects
            .set_note(p1, front, prev, alice, "ambiente di test giù");

        let p2 = app.projects.add("P2", Some("BBB"), Some(cur));
        app.projects.add_dev(p2, front);
        app.projects.add_effort(p2, front, cur, bob, Effort(8));
        app.projects
            .set_note(p2, front, cur, bob, "solo nota di cella");

        (app, p1, p2)
    }

    #[test]
    fn minuta_includes_worker_notes_only_when_requested() {
        let (app, p1, _) = app_with_notes();

        // Spento: la minuta resta quella di prima, solo note di progetto.
        let off = build_minuta(&app, &[p1], false, true, false);
        assert!(off.contains("Riunione settimanale"));
        assert!(!off.contains("serve conferma API"));

        // Acceso: le note di cella compaiono sotto la settimana, con worker e dev.
        let on = build_minuta(&app, &[p1], false, true, true);
        assert!(on.contains("Riunione settimanale"));
        assert!(on.contains("- **Alice** (Frontend): serve conferma API\n"));
        assert!(on.contains("- **Bob** (Frontend): blocco su migrazione\n"));

        // Ordine dentro la settimana: worker in ordine alfabetico.
        let a = on.find("serve conferma API").unwrap();
        let b = on.find("blocco su migrazione").unwrap();
        assert!(a < b);
    }

    #[test]
    fn minuta_shows_weeks_with_only_worker_notes() {
        let (app, p1, _) = app_with_notes();
        let cur = current_week_id();
        let prev = WeekId(cur.0 - WEEK_STEP);
        let prev_label = days_to_local(prev.0 as i32).format("%y-%m-%d").to_string();

        // Senza note worker la settimana precedente (priva di nota progetto) non c'è.
        let off = build_minuta(&app, &[p1], false, true, false);
        assert!(!off.contains(&prev_label));

        // Con le note worker compare, con il solo elenco (nessun testo di progetto).
        let on = build_minuta(&app, &[p1], false, true, true);
        assert!(on.contains(&format!(
            "**{prev_label}**\n\n- **Alice** (Frontend): ambiente"
        )));

        // «Solo settimana corrente» continua a tagliarla via.
        let cur_only = build_minuta(&app, &[p1], true, true, true);
        assert!(!cur_only.contains(&prev_label));
        assert!(cur_only.contains("serve conferma API"));
    }

    #[test]
    fn worker_notes_count_for_the_only_with_notes_filter() {
        let (app, _, p2) = app_with_notes();

        // P2 non ha note di progetto: senza note worker viene escluso…
        let off = build_minuta(&app, &[p2], false, true, false);
        assert!(!off.contains("BBB"));

        // …e con le note worker attive rientra.
        let on = build_minuta(&app, &[p2], false, true, true);
        assert!(on.contains("## BBB"));
        assert!(on.contains("- **Bob** (Frontend): solo nota di cella\n"));

        // Senza il filtro compare comunque, ma con il segnaposto.
        let placeholder = build_minuta(&app, &[p2], false, false, false);
        assert!(placeholder.contains("## BBB"));
        assert!(placeholder.contains("_(nessuna nota)_"));
    }

    #[test]
    fn multiline_worker_note_stays_inside_the_bullet() {
        let n = WorkerNote {
            worker: "Alice".to_string(),
            dev: "Frontend".to_string(),
            note: "prima riga\nseconda riga".to_string(),
        };
        assert_eq!(
            worker_note_bullet(&n),
            "- **Alice** (Frontend): prima riga\n  seconda riga\n"
        );
    }
}

// ── Aiuto / Manuale ─────────────────────────────────────────────────────────
