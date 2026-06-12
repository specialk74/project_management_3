//! Costanti visive e helper colore — riproducono `ui/styles.slint`.

use eframe::egui::{Color32, FontId};

// Dimensioni (px logici), come Styles in styles.slint
pub const ROW_H: f32 = 18.0;
pub const COL_W: f32 = 90.0;
pub const LEFT_W: f32 = 300.0;
pub const DEV_BORDER: f32 = 3.0;
pub const FONT_SIZE: f32 = 13.0;

// Suddivisione interna della colonna sinistra (info | strip Dev | righe dev)
pub const DEV_STRIP_W: f32 = 12.0;
pub const DEV_NAME_W: f32 = COL_W; // 90
pub const DEV_EFFORT_W: f32 = COL_W - 25.0; // 65
pub const LEFT_DEV_W: f32 = DEV_NAME_W + DEV_EFFORT_W; // 155
pub const LEFT_INFO_W: f32 = LEFT_W - DEV_STRIP_W - LEFT_DEV_W; // 133

pub const DEFAULT_MAX_HOURS: u32 = 40;

// Colori (da styles.slint / Slint Colors.*)
pub const BG_DARK: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
pub const PANEL_BG: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
pub const BETWEEN_PROJECTS: Color32 = Color32::from_rgb(0x80, 0x80, 0x80); // Colors.gray
pub const START_STOP: Color32 = Color32::from_rgb(0xff, 0xff, 0x00); // Colors.yellow
pub const THIS_WEEK: Color32 = Color32::from_rgb(0x00, 0xce, 0x3a);
pub const EFFORT_ORANGE: Color32 = Color32::from_rgb(0xff, 0xa5, 0x00);
pub const NOTE_ORANGE: Color32 = Color32::from_rgb(0xff, 0xa5, 0x00);
pub const DEADLINE_BG: Color32 = Color32::from_rgb(0x00, 0x80, 0x00); // Colors.green
pub const START_BG: Color32 = Color32::from_rgb(70, 130, 180); // steelblue
pub const TEXT_WHITE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
pub const CAT_BLUE: Color32 = Color32::from_rgb(0x7a, 0xb0, 0xd4);
pub const SEL_BG: Color32 = Color32::from_rgba_premultiplied(70, 130, 180, 90); // steelblue alpha
pub const FOCUS_BORDER: Color32 = Color32::from_rgb(0xff, 0x45, 0x00); // orangered

#[inline]
pub fn from_hex(rgb: u32) -> Color32 {
    Color32::from_rgb(((rgb >> 16) & 0xFF) as u8, ((rgb >> 8) & 0xFF) as u8, (rgb & 0xFF) as u8)
}

/// Colore del cumulativo per dev (replica `my-function` in global.slint).
pub fn cumulative_color(total: i32, planned: i32) -> Color32 {
    if total == 0 {
        return TEXT_WHITE;
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
