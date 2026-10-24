use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub struct DevId(pub usize);

const DEV_COLORS: &[(u8, u8, u8)] = &[
    (0x00, 0x99, 0xFF), // blu
    (0x80, 0x00, 0x80), // viola
    (0x80, 0x40, 0x00), // marrone
    (0x00, 0x80, 0x00), // verde
    (0x00, 0x00, 0xFF), // blu scuro
    (0x06, 0x9a, 0x75), // giallo
    (0x80, 0x00, 0xFF), // porpora
    (0xFF, 0x80, 0x00), // arancione
    (0x00, 0xFF, 0xFF), // ciano
    (0xFF, 0x00, 0x80), // rosa
    (0x00, 0x80, 0x80), // teal
    (0xFF, 0x40, 0x40), // rosso chiaro
];

fn color_for_idx(idx: DevId) -> (usize, usize) {
    let (r, g, b) = DEV_COLORS[idx.0 % DEV_COLORS.len()];
    let bg = ((r as usize) << 16) | ((g as usize) << 8) | (b as usize);
    let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let font = if lum < 128.0 { 0xFFFFFF } else { 0x000000 };
    (bg, font)
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Dev {
    name: String,
    #[serde(default)]
    bg_color: Option<usize>,
    #[serde(default)]
    font_color: Option<usize>,
    /// Escluso dai totali in basso a sinistra del footer (riga e somma
    /// «Totale»); resta visibile ovunque altrove. Omesso dal file quando falso.
    #[serde(default, skip_serializing_if = "is_false")]
    hide_in_totals: bool,
}

impl Dev {
    pub fn new(name: &str, idx: DevId) -> Self {
        let (bg, font) = color_for_idx(idx);
        Self {
            name: name.to_string(),
            bg_color: Some(bg),
            font_color: Some(font),
            hide_in_totals: false,
        }
    }

    pub fn is_hidden_in_totals(&self) -> bool {
        self.hide_in_totals
    }

    pub fn set_hidden_in_totals(&mut self, hide: bool) {
        self.hide_in_totals = hide;
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn bg_color(&self) -> usize {
        self.bg_color.unwrap_or(0x0099FF)
    }

    pub fn font_color(&self) -> usize {
        self.font_color.unwrap_or(0xFFFFFF)
    }
}
