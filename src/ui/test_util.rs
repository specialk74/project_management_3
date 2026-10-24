//! Helper condivisi dai test dell'interfaccia (`tests.rs`, e i moduli di test di
//! `saturation.rs` e `matrix.rs`). Compilato solo per i test.

use super::*;

/// Schermo `w`×`h` con l'origine in alto a sinistra, per `RawInput::screen_rect`.
pub(crate) fn screen(w: f32, h: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))
}

/// Un click in `pos` col tasto `button`: spostamento, pressione, rilascio.
/// Vanno mandati in **tre frame distinti**: un click compresso in un frame solo
/// non riproduce il comportamento di menù e tendine.
pub(crate) fn click_events(pos: egui::Pos2, button: egui::PointerButton) -> [egui::Event; 3] {
    let btn = |pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    };
    [egui::Event::PointerMoved(pos), btn(true), btn(false)]
}

/// Testi disegnati nel frame, nell'ordine delle shape.
pub(crate) fn drawn_texts(out: &egui::FullOutput) -> Vec<String> {
    out.shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

/// Centro del testo `needle` disegnato nel frame, cercato tra le shape in
/// uscita: evita di indovinare le coordinate dei widget nei test di input.
pub(crate) fn text_pos(out: &egui::FullOutput, needle: &str) -> Option<egui::Pos2> {
    out.shapes.iter().find_map(|cs| match &cs.shape {
        egui::epaint::Shape::Text(t) if t.galley.job.text == needle => {
            Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
        }
        _ => None,
    })
}
