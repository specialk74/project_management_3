//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Finestra "Manuale d'uso" (Aiuto ▸ Manuale d'uso…): indice cliccabile a
/// sinistra (che scrolla al capitolo) e testo Markdown a destra, reso via
/// `egui_commonmark`; una casella di ricerca filtra le sezioni (livello `##`).
///
/// La versione mostrata nel manuale viene sostituita a runtime dal segnaposto
/// `{{VERSION}}` con quella di `Cargo.toml`, così non resta mai indietro. Le
/// ancore Markdown (`[cap](#…)`) non sono navigabili in `egui_commonmark`: per
/// questo l'indice è ricostruito con veri pulsanti che usano `scroll_to_rect`.
pub(crate) fn help_window(ctx: &egui::Context, state: &mut UiState) {
    if !state.show_help {
        return;
    }
    // Versione sempre allineata a Cargo.toml.
    let manual = MANUAL_MD.replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
    let sections = manual_sections(&manual);

    let mut open = true;
    egui::Window::new("Manuale d'uso")
        .collapsible(true)
        .resizable(true)
        .default_size(egui::vec2(880.0, 640.0))
        .open(&mut open)
        .show(ctx, |ui| {
            // Barra di ricerca.
            ui.horizontal(|ui| {
                ui.label("🔎");
                ui.add(
                    egui::TextEdit::singleline(&mut state.help_search)
                        .hint_text("Cerca nel manuale…")
                        .desired_width(360.0),
                );
                if ui.button("✕").on_hover_text("Pulisci").clicked() {
                    state.help_search.clear();
                }
            });
            ui.separator();

            let q = state.help_search.trim().to_lowercase();

            // ── Ricerca attiva: elenco piatto delle sezioni che combaciano ──
            if !q.is_empty() {
                egui::ScrollArea::vertical()
                    .id_salt("help_search_results")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut any = false;
                        for sec in &sections {
                            if sec.to_lowercase().contains(&q) {
                                any = true;
                                CommonMarkViewer::new().show(ui, &mut state.help_md_cache, sec);
                                ui.separator();
                            }
                        }
                        if !any {
                            ui.label(format!("Nessun risultato per «{}».", state.help_search));
                        }
                    });
                return;
            }

            // ── Indice cliccabile a sinistra + contenuto a destra ──
            egui::SidePanel::left("help_toc")
                .resizable(true)
                .default_width(230.0)
                .show_inside(ui, |ui| {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("Indice")
                            .strong()
                            .color(g(EFFORT_ORANGE)),
                    );
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .id_salt("help_toc_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (i, sec) in sections.iter().enumerate() {
                                if let Some(title) = chapter_title(sec) {
                                    if ui.link(title).clicked() {
                                        state.help_scroll_to = Some(i);
                                    }
                                }
                            }
                        });
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("help_content_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, sec) in sections.iter().enumerate() {
                            // L'indice del Markdown è sostituito dalla colonna a
                            // sinistra: non lo ripetiamo nel contenuto.
                            if is_index_section(sec) {
                                continue;
                            }
                            // Ancora invisibile a inizio sezione: se è il capitolo
                            // scelto, porta lì lo scroll.
                            let (anchor, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 0.1),
                                Sense::hover(),
                            );
                            if state.help_scroll_to == Some(i) {
                                ui.scroll_to_rect(anchor, Some(egui::Align::TOP));
                                state.help_scroll_to = None;
                            }
                            CommonMarkViewer::new().show(ui, &mut state.help_md_cache, sec);
                        }
                    });
            });
        });
    if !open {
        state.show_help = false;
        state.help_scroll_to = None;
    }
}

/// Titolo di capitolo per l'indice laterale: `Some("1. Cos'è il programma")` per
/// le sezioni `## N. …`; `None` per il titolo/intro e per la stessa sezione
/// "Indice" (che l'app rimpiazza con la colonna cliccabile).
pub(crate) fn chapter_title(section: &str) -> Option<&str> {
    let first = section.lines().next()?.trim();
    let rest = first.strip_prefix("## ")?.trim();
    (rest != "Indice").then_some(rest)
}

/// True se la sezione è l'indice del Markdown (`## Indice`).
pub(crate) fn is_index_section(section: &str) -> bool {
    section
        .lines()
        .next()
        .is_some_and(|l| l.trim() == "## Indice")
}

/// Divide il manuale in sezioni tagliando all'inizio di ogni intestazione di
/// livello 2 (`## `). La prima sezione contiene titolo, introduzione e indice.
pub(crate) fn manual_sections(md: &str) -> Vec<&str> {
    let bytes = md.as_bytes();
    let mut out = Vec::new();
    let mut last = 0;
    for (pos, _) in md.match_indices("## ") {
        // deve essere a inizio riga e non far parte di "### "/"#### "
        let at_line_start = pos == 0 || bytes[pos - 1] == b'\n';
        if at_line_start {
            if pos > last {
                out.push(&md[last..pos]);
            }
            last = pos;
        }
    }
    out.push(&md[last..]);
    out
}
