//! Notifiche in-app ("toast"): l'area di stato che rende visibili gli errori che
//! prima finivano solo in `eprintln!` (salvataggio, export, sincronizzazione git)
//! e gli avvisi sul file condiviso.
//!
//! Le notifiche sono impilate in basso a destra, sopra a tutto. Quelle con una
//! durata (`ToastLevel::ttl`) spariscono da sole; gli **errori** restano finché
//! non si preme la ✕, così non possono passare inosservati.

use super::*;

/// Larghezza massima del riquadro di una notifica.
const TOAST_W: f32 = 420.0;

/// Disegna (e fa scadere) le notifiche in-app. Da chiamare una volta per frame.
pub(crate) fn toasts_layer(ctx: &egui::Context, state: &mut UiState) {
    // Scadenze: via quelle con ttl superata.
    state.toasts.retain(|t| match t.level.ttl() {
        Some(d) => t.born.elapsed() < d,
        None => true,
    });
    if state.toasts.is_empty() {
        return;
    }

    // Ridisegna quando la prossima notifica scade (altrimenti resterebbe a
    // schermo finché l'utente non muove il mouse).
    let next = state
        .toasts
        .iter()
        .filter_map(|t| t.level.ttl().map(|d| d.saturating_sub(t.born.elapsed())))
        .min();
    if let Some(d) = next {
        ctx.request_repaint_after(d);
    }

    let mut close: Option<usize> = None;
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-12.0, -12.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_max_width(TOAST_W);
            for (i, t) in state.toasts.iter().enumerate() {
                let color = g(t.level.color());
                egui::Frame::NONE
                    .fill(toast_bg())
                    .stroke(Stroke::new(1.0_f32, color))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_max_width(TOAST_W);
                        // ✕ ancorata a destra, testo e icona a sinistra.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            if ui.small_button("✕").on_hover_text("Chiudi").clicked() {
                                close = Some(i);
                            }
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                ui.colored_label(color, t.level.icon());
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&t.text).color(text()))
                                        .wrap(),
                                );
                            });
                        });
                    });
                ui.add_space(4.0);
            }
        });

    if let Some(i) = close {
        state.toasts.remove(i);
    }
}
