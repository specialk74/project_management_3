//! Costanti visive e helper colore — riproducono `ui/styles.slint`.

use eframe::egui::{Color32, FontId};

// Dimensioni (px logici), come Styles in styles.slint
pub const ROW_H: f32 = 18.0;
pub const COL_W: f32 = 90.0;
pub const COMPACT_W: f32 = 25.0; // larghezza colonna in vista compatta
pub const LEFT_W: f32 = 300.0;
pub const DEV_BORDER: f32 = 3.0;
pub const FONT_SIZE: f32 = 13.0;

// Suddivisione interna della colonna sinistra (info | strip Dev | righe dev)
pub const DEV_STRIP_W: f32 = 12.0;
pub const DEV_NAME_W: f32 = COL_W; // 90
pub const DEV_TOTAL_W: f32 = 60.0; // larghezza colonna dei totali-anno nel footer
pub const DEV_EFFORT_W: f32 = COL_W - 25.0; // 65
pub const LEFT_DEV_W: f32 = DEV_NAME_W + DEV_EFFORT_W; // 155
pub const LEFT_INFO_W: f32 = LEFT_W - DEV_STRIP_W - LEFT_DEV_W; // 133

pub const DEFAULT_MAX_HOURS: u32 = 40;

// Colori (da styles.slint / Slint Colors.*)
pub const BETWEEN_PROJECTS: Color32 = Color32::from_rgb(0x80, 0x80, 0x80); // Colors.gray
pub const START_STOP: Color32 = Color32::from_rgb(0xff, 0xff, 0x00); // Colors.yellow
/// Colore **di default** dell'evidenziazione della settimana corrente (giallo
/// fosforescente). Non è una costante d'uso diretto: il colore effettivo è
/// `this_week()`, impostato a inizio frame da `set_this_week_color` con il
/// valore letto dal file `.ron` (campo `week_color: "#RRGGBB"`).
pub const THIS_WEEK_DEFAULT: u32 = 0xCC_FF_00;
pub const EFFORT_ORANGE: Color32 = Color32::from_rgb(0xff, 0xa5, 0x00);
pub const NOTE_ORANGE: Color32 = Color32::from_rgb(0xff, 0xa5, 0x00);
pub const DEADLINE_BG: Color32 = Color32::from_rgb(0x00, 0x80, 0x00); // Colors.green
pub const START_BG: Color32 = Color32::from_rgb(70, 130, 180); // steelblue
pub const CANARY: Color32 = Color32::from_rgb(0xff, 0xf0, 0x66); // giallo canarino (colonna confine anno)
/// Porpora usato **solo nella GUI** per i worker "ghost", così da distinguerli
/// dai worker sovra-allocati (che restano rossi). Negli export PDF/SVG i ghost
/// restano rossi.
pub const GHOST_PURPLE: Color32 = Color32::from_rgb(0x9C, 0x27, 0xB0);
pub const CAT_BLUE: Color32 = Color32::from_rgb(0x7a, 0xb0, 0xd4);
pub const SEL_BG: Color32 = Color32::from_rgba_premultiplied(70, 130, 180, 90); // steelblue alpha
pub const FOCUS_BORDER: Color32 = Color32::from_rgb(0xff, 0x45, 0x00); // orangered

use std::cell::Cell;

thread_local! {
    /// Modalità bianco/nero (senza colori). Impostata a inizio frame da `set_bw_mode`.
    static BW_MODE: Cell<bool> = const { Cell::new(false) };
    /// Tema corrente: true = scuro (default), false = chiaro. Impostato a inizio
    /// frame da `set_dark_theme` (risolto da Auto/Chiaro/Scuro in `ui.rs`).
    static DARK_THEME: Cell<bool> = const { Cell::new(true) };
    /// Colore dell'evidenziazione della settimana corrente. Non c'è una voce di
    /// menù: si cambia **solo** dal file `.ron` (`week_color`) e viene impostato
    /// a inizio frame da `set_this_week_color`.
    static THIS_WEEK_COLOR: Cell<u32> = const { Cell::new(THIS_WEEK_DEFAULT) };
}

/// Attiva/disattiva la resa in scala di grigi (chiamata a inizio frame).
pub fn set_bw_mode(on: bool) {
    BW_MODE.with(|b| b.set(on));
}

/// Imposta il tema (chiamata a inizio frame). `true` = scuro, `false` = chiaro.
pub fn set_dark_theme(on: bool) {
    DARK_THEME.with(|b| b.set(on));
}

/// Imposta il colore della settimana corrente (chiamata a inizio frame con il
/// valore del file `.ron`). `rgb` è 0xRRGGBB.
pub fn set_this_week_color(rgb: u32) {
    THIS_WEEK_COLOR.with(|c| c.set(rgb));
}

/// Colore dell'evidenziazione della settimana corrente (griglia, footer,
/// intestazione settimane). I chiamanti lo passano per `g(...)` come gli altri
/// accenti.
#[inline]
pub fn this_week() -> Color32 {
    let rgb = THIS_WEEK_COLOR.with(|c| c.get());
    Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// Converte `"#RRGGBB"` (con `#` opzionale, maiuscole o minuscole) in 0xRRGGBB.
/// `None` se la stringa non è un colore esadecimale a 6 cifre.
pub fn parse_hex_rgb(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// Forma canonica scritta nel `.ron`: `#RRGGBB` maiuscolo.
pub fn hex_rgb_string(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

#[inline]
fn is_dark() -> bool {
    DARK_THEME.with(|b| b.get())
}

// ── Colori dipendenti dal tema ──────────────────────────────────────────────
// Sfondo principale e testi cambiano tra scuro e chiaro; gli accenti (arancio,
// verde, blu, canarino…) restano invariati.

/// Sfondo principale (pannelli, celle).
#[inline]
pub fn bg() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x1e, 0x1e, 0x1e)
    } else {
        Color32::from_rgb(0xf5, 0xf5, 0xf5)
    }
}

/// Testo principale (massimo contrasto sullo sfondo).
#[inline]
pub fn text() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xff, 0xff, 0xff)
    } else {
        Color32::from_rgb(0x1a, 0x1a, 0x1a)
    }
}

/// Testo secondario attenuato.
#[inline]
pub fn text_dim() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x88, 0x88, 0x88)
    } else {
        Color32::from_rgb(0x5a, 0x5a, 0x5a)
    }
}

/// Testo molto tenue (placeholder "—").
#[inline]
pub fn text_faint() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x55, 0x55, 0x55)
    } else {
        Color32::from_rgb(0xa8, 0xa8, 0xa8)
    }
}

/// Sfondo delle righe pari (footer worker).
#[inline]
pub fn row_even() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x80, 0x80, 0x80)
    } else {
        Color32::from_rgb(0xcf, 0xcf, 0xcf)
    }
}

/// Sfondo delle righe dispari (footer worker).
#[inline]
pub fn row_alt() -> Color32 {
    if is_dark() {
        Color32::BLACK
    } else {
        Color32::from_rgb(0xee, 0xee, 0xee)
    }
}

/// Sfondo della striscia verticale "Dev" nella colonna sinistra (normale/hover).
#[inline]
pub fn strip_bg(hovered: bool) -> Color32 {
    match (is_dark(), hovered) {
        (true, false) => Color32::from_rgb(0x22, 0x22, 0x22),
        (true, true) => Color32::from_rgb(0x3a, 0x3a, 0x3a),
        (false, false) => Color32::from_rgb(0xdd, 0xdd, 0xdd),
        (false, true) => Color32::from_rgb(0xc4, 0xc4, 0xc4),
    }
}

/// Bordo della striscia verticale "Dev".
#[inline]
pub fn strip_border() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x55, 0x55, 0x55)
    } else {
        Color32::from_rgb(0xaa, 0xaa, 0xaa)
    }
}

// Colori "semantici" del footer worker: chiari (per sfondi scuri) nel tema scuro,
// scuri (per sfondi chiari) nel tema chiaro, così restano leggibili in entrambi.

/// Verde "ok" (valore presente).
#[inline]
pub fn ok_green() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x90, 0xee, 0x90)
    } else {
        Color32::from_rgb(0x17, 0x7a, 0x33)
    }
}

/// Giallo "zero" (valore nullo).
#[inline]
pub fn zero_yellow() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xff, 0xff, 0x00)
    } else {
        Color32::from_rgb(0x86, 0x6a, 0x00)
    }
}

// Colori "semantici" delle notifiche in-app (toast), stessa logica: chiari sul
// tema scuro, scuri sul tema chiaro.

/// Rosso "errore".
#[inline]
pub fn error_red() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xff, 0x6b, 0x6b)
    } else {
        Color32::from_rgb(0xb0, 0x1c, 0x1c)
    }
}

/// Ambra "attenzione".
#[inline]
pub fn warn_amber() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xff, 0xc1, 0x07)
    } else {
        Color32::from_rgb(0x8a, 0x5a, 0x00)
    }
}

/// Azzurro "informazione".
#[inline]
pub fn info_blue() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x5a, 0xc8, 0xfa)
    } else {
        Color32::from_rgb(0x0b, 0x5f, 0x8a)
    }
}

/// Sfondo del riquadro di una notifica.
#[inline]
pub fn toast_bg() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x2a, 0x2a, 0x2a)
    } else {
        Color32::from_rgb(0xf4, 0xf4, 0xf4)
    }
}

/// Marrone "override a zero".
#[inline]
pub fn override_brown() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xcd, 0x85, 0x3f)
    } else {
        Color32::from_rgb(0x8a, 0x53, 0x1f)
    }
}

/// Converte un colore in grigio (luminanza percepita) se la modalità B/N è
/// attiva; altrimenti lo restituisce invariato. I grigi restano grigi, quindi
/// può essere applicata indistintamente. L'alpha è preservato.
#[inline]
pub fn g(c: Color32) -> Color32 {
    if !BW_MODE.with(|b| b.get()) {
        return c;
    }
    let [r, gr, b, a] = c.to_array();
    let lum = (0.299 * r as f32 + 0.587 * gr as f32 + 0.114 * b as f32).round() as u8;
    Color32::from_rgba_premultiplied(lum, lum, lum, a)
}

/// Colore di testo (nero o bianco) leggibile sopra `background`, scelto in base
/// alla luminanza percepita del colore di sfondo — così resta leggibile in
/// qualunque tema e con qualunque colore (es. celle milestone tinte).
#[inline]
pub fn contrast_text(background: Color32) -> Color32 {
    let [r, gr, b, _] = background.to_array();
    let lum = 0.299 * r as f32 + 0.587 * gr as f32 + 0.114 * b as f32;
    if lum > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[inline]
pub fn from_hex(rgb: u32) -> Color32 {
    g(Color32::from_rgb(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    ))
}

/// Colore del cumulativo per dev (replica `my-function` in global.slint).
pub fn cumulative_color(total: i32, planned: i32) -> Color32 {
    if total == 0 {
        return text();
    }
    let lt = |pct: i32| total < planned * pct / 100;
    if lt(10) {
        from_hex(0x018730)
    } else if lt(20) {
        from_hex(0x558d00)
    } else if lt(30) {
        from_hex(0x6d8800)
    } else if lt(40) {
        from_hex(0x888800)
    } else if lt(50) {
        from_hex(0xa88700)
    } else if lt(60) {
        from_hex(0xb57a00)
    } else if lt(70) {
        from_hex(0xcf6f00)
    } else if lt(80) {
        from_hex(0xc54300)
    } else if lt(90) {
        from_hex(0xd52c00)
    } else {
        from_hex(0xd10100)
    }
}

#[inline]
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

#[inline]
pub fn cell_font() -> FontId {
    FontId::monospace(FONT_SIZE)
}

/// Font più piccolo per le celle worker della griglia (più testo entra nei 90px).
#[inline]
pub fn person_font() -> FontId {
    FontId::monospace(11.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors_with_or_without_hash() {
        assert_eq!(parse_hex_rgb("#CCFF00"), Some(0xCCFF00));
        assert_eq!(parse_hex_rgb("ccff00"), Some(0xCCFF00));
        assert_eq!(parse_hex_rgb("  #ff8800 "), Some(0xFF8800));
    }

    #[test]
    fn rejects_malformed_colors() {
        for s in ["", "#", "#FFF", "#GGFFAA", "#CCFF001", "verde"] {
            assert_eq!(parse_hex_rgb(s), None, "«{s}» non è un colore valido");
        }
    }

    #[test]
    fn hex_string_round_trips() {
        assert_eq!(hex_rgb_string(THIS_WEEK_DEFAULT), "#CCFF00");
        assert_eq!(parse_hex_rgb(&hex_rgb_string(0x01A2B3)), Some(0x01A2B3));
    }

    /// Il colore effettivo è quello impostato dal file, non una costante.
    #[test]
    fn this_week_follows_the_configured_color() {
        set_bw_mode(false);
        set_this_week_color(0xFF8800);
        assert_eq!(this_week(), Color32::from_rgb(0xFF, 0x88, 0x00));
        set_this_week_color(THIS_WEEK_DEFAULT);
        assert_eq!(this_week(), Color32::from_rgb(0xCC, 0xFF, 0x00));
    }
}
