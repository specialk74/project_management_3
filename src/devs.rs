use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const DEV_COLORS: &[(u8, u8, u8)] = &[
    (0x00, 0x99, 0xFF), // blu
    (0x80, 0x00, 0x80), // viola
    (0x80, 0x40, 0x00), // marrone
    (0x00, 0x80, 0x00), // verde
    (0x00, 0x00, 0xFF), // blu scuro
    (0xFF, 0xFF, 0x00), // giallo
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

#[derive(Serialize, Deserialize)]
pub struct Dev {
    name: String,
    #[serde(default)]
    bg_color: Option<usize>,
    #[serde(default)]
    font_color: Option<usize>,
}

impl Dev {
    pub fn new(name: &str, idx: DevId) -> Self {
        let (bg, font) = color_for_idx(idx);
        Self {
            name: name.to_string(),
            bg_color: Some(bg),
            font_color: Some(font),
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    pub fn bg_color(&self) -> usize {
        self.bg_color.unwrap_or(0x0099FF)
    }

    pub fn font_color(&self) -> usize {
        self.font_color.unwrap_or(0xFFFFFF)
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn set_bg_color(&mut self, color: Option<usize>) {
        self.bg_color = color;
    }

    pub fn set_font_color(&mut self, color: Option<usize>) {
        self.font_color = color;
    }
}

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct DevId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct Devs {
    last_id: DevId,
    dev_id: HashMap<DevId, Dev>,
}

impl Devs {
    pub fn new() -> Self {
        Self {
            last_id: DevId(0),
            dev_id: HashMap::new(),
        }
    }

    pub fn del(&mut self, id: DevId) {
        self.dev_id.remove(&id);
    }

    pub fn add(&mut self, name: &str) -> DevId {
        if let Some((&id, _)) = self.dev_id.iter().find(|(_, dev)| dev.get_name() == name) {
            return id;
        }
        let id = self.last_id;
        self.dev_id.insert(id, Dev::new(name, id));
        self.last_id.0 += 1;
        id
    }

    pub fn list(&self) -> Vec<(DevId, String)> {
        let mut items: Vec<(DevId, String)> = self
            .dev_id
            .iter()
            .map(|(&id, d)| (id, d.name.clone()))
            .collect();
        items.sort_by_key(|(id, _)| id.0);
        items
    }

    /// Returns (id, name, bg_color_packed, font_color_packed)
    pub fn list_full(&self) -> Vec<(DevId, String, usize, usize)> {
        let mut items: Vec<(DevId, String, usize, usize)> = self
            .dev_id
            .iter()
            .map(|(&id, d)| (id, d.name.clone(), d.bg_color(), d.font_color()))
            .collect();
        items.sort_by_key(|(id, _, _, _)| id.0);
        items
    }
}
