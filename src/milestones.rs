#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub struct MilestoneId(pub usize);

/// Palette di colori distinti (0xRRGGBB) da cui pescare per le nuove milestone.
/// Alla creazione si sceglie a caso un colore non ancora usato; se sono tutti
/// occupati si genera un colore pseudo-casuale.
const PALETTE: [u32; 16] = [
    0xE53935, // rosso
    0x8E24AA, // viola
    0x3949AB, // indaco
    0x1E88E5, // blu
    0x00ACC1, // ciano
    0x00897B, // teal
    0x43A047, // verde
    0x7CB342, // lime
    0xC0CA33, // giallo-verde
    0xFDD835, // giallo
    0xFFB300, // ambra
    0xFB8C00, // arancione
    0xF4511E, // arancione scuro
    0x6D4C41, // marrone
    0x757575, // grigio
    0xD81B60, // rosa
];

#[derive(Serialize, Deserialize, Clone)]
pub struct Milestone {
    pub name: String,
    /// Colore associato, formato 0xRRGGBB.
    pub color: u32,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Milestones {
    last_id: MilestoneId,
    milestones: HashMap<MilestoneId, Milestone>,
}

impl Default for MilestoneId {
    fn default() -> Self {
        MilestoneId(0)
    }
}

impl Milestones {
    pub fn new() -> Self {
        Self {
            last_id: MilestoneId(0),
            milestones: HashMap::new(),
        }
    }

    /// Colori attualmente in uso.
    fn used_colors(&self) -> Vec<u32> {
        self.milestones.values().map(|m| m.color).collect()
    }

    /// Sceglie un colore distinto: uno a caso tra quelli della palette non
    /// ancora usati; se sono tutti occupati, ne genera uno pseudo-casuale.
    fn pick_color(&self) -> u32 {
        let used = self.used_colors();
        let free: Vec<u32> = PALETTE.iter().copied().filter(|c| !used.contains(c)).collect();
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        if !free.is_empty() {
            free[(seed as usize) % free.len()]
        } else {
            // fallback: colore pseudo-casuale dai bit del seed
            (seed as u32) & 0x00FF_FFFF
        }
    }

    /// Crea una nuova milestone con un colore distinto assegnato in automatico.
    pub fn add(&mut self, name: &str) -> MilestoneId {
        let color = self.pick_color();
        let id = self.last_id;
        self.milestones.insert(id, Milestone { name: name.to_string(), color });
        self.last_id.0 += 1;
        id
    }

    pub fn del(&mut self, id: MilestoneId) {
        self.milestones.remove(&id);
    }

    pub fn get(&self, id: MilestoneId) -> Option<&Milestone> {
        self.milestones.get(&id)
    }

    pub fn get_name(&self, id: MilestoneId) -> Option<&str> {
        self.milestones.get(&id).map(|m| m.name.as_str())
    }

    pub fn get_color(&self, id: MilestoneId) -> Option<u32> {
        self.milestones.get(&id).map(|m| m.color)
    }

    pub fn set_color(&mut self, id: MilestoneId, color: u32) {
        if let Some(m) = self.milestones.get_mut(&id) {
            m.color = color;
        }
    }

    pub fn set_name(&mut self, id: MilestoneId, name: &str) {
        if let Some(m) = self.milestones.get_mut(&id) {
            m.name = name.to_string();
        }
    }

    /// Elenco (id, nome, colore) ordinato per id di creazione.
    pub fn list(&self) -> Vec<(MilestoneId, String, u32)> {
        let mut items: Vec<(MilestoneId, String, u32)> = self
            .milestones
            .iter()
            .map(|(&id, m)| (id, m.name.clone(), m.color))
            .collect();
        items.sort_by_key(|(id, _, _)| *id);
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_returns_incrementing_ids() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        let b = ms.add("Beta");
        assert_eq!(a, MilestoneId(0));
        assert_eq!(b, MilestoneId(1));
    }

    #[test]
    fn add_assigns_distinct_colors() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        let b = ms.add("Beta");
        assert_ne!(ms.get_color(a), ms.get_color(b));
    }

    #[test]
    fn del_removes_milestone() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        ms.del(a);
        assert!(ms.get(a).is_none());
    }

    #[test]
    fn set_color_updates() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        ms.set_color(a, 0x123456);
        assert_eq!(ms.get_color(a), Some(0x123456));
    }

    #[test]
    fn list_sorted_by_id() {
        let mut ms = Milestones::new();
        ms.add("Alpha");
        ms.add("Beta");
        ms.add("Gamma");
        let ids: Vec<MilestoneId> = ms.list().into_iter().map(|(id, _, _)| id).collect();
        assert_eq!(ids, vec![MilestoneId(0), MilestoneId(1), MilestoneId(2)]);
    }
}
