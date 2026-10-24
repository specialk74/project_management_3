use crate::dev_utils::dev::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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
            .map(|(&id, d)| (id, d.get_name()))
            .collect();
        items.sort_by_key(|(id, _)| id.0);
        items
    }

    /// Dev mostrati nei totali del footer (esclusi quelli con `hide_in_totals`),
    /// ordinati per id come `list`.
    pub fn list_in_totals(&self) -> Vec<(DevId, String)> {
        self.list()
            .into_iter()
            .filter(|(id, _)| !self.is_hidden_in_totals(*id))
            .collect()
    }

    pub fn is_hidden_in_totals(&self, id: DevId) -> bool {
        self.dev_id.get(&id).is_some_and(|d| d.is_hidden_in_totals())
    }

    /// Ritorna `true` se il valore è cambiato.
    pub fn set_hidden_in_totals(&mut self, id: DevId, hide: bool) -> bool {
        match self.dev_id.get_mut(&id) {
            Some(d) if d.is_hidden_in_totals() != hide => {
                d.set_hidden_in_totals(hide);
                true
            }
            _ => false,
        }
    }

    /// Returns (id, name, bg_color_packed, font_color_packed)
    pub fn list_full(&self) -> Vec<(DevId, String, usize, usize)> {
        let mut items: Vec<(DevId, String, usize, usize)> = self
            .dev_id
            .iter()
            .map(|(&id, d)| (id, d.get_name(), d.bg_color(), d.font_color()))
            .collect();
        items.sort_by_key(|(id, _, _, _)| id.0);
        items
    }
}
