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

    /// Il dev con questo id, senza costruire l'elenco.
    pub fn get(&self, id: DevId) -> Option<&Dev> {
        self.dev_id.get(&id)
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

    /// Dev non nascosti (proposti nelle scelte), ordinati per id come `list`.
    pub fn list_visible(&self) -> Vec<(DevId, String)> {
        self.list()
            .into_iter()
            .filter(|(id, _)| !self.is_hidden(*id))
            .collect()
    }

    pub fn is_hidden(&self, id: DevId) -> bool {
        self.dev_id.get(&id).is_some_and(|d| d.is_hidden())
    }

    /// Nasconde / rimostra il dev. Ritorna `true` se il valore è cambiato.
    pub fn set_hidden(&mut self, id: DevId, hidden: bool) -> bool {
        match self.dev_id.get_mut(&id) {
            Some(d) if d.is_hidden() != hidden => {
                d.set_hidden(hidden);
                true
            }
            _ => false,
        }
    }

    /// Rinomina il dev: `Err` con il motivo se il nome è vuoto, contiene `|`
    /// o è già di un altro dev (vedi `check_new_name`). `Ok(false)` se il nome
    /// non cambia.
    pub fn rename(&mut self, id: DevId, name: &str) -> Result<bool, String> {
        let name = name.trim();
        let others = self.list().into_iter().filter(|(d, _)| *d != id).map(|(_, n)| n);
        crate::app::check_new_name(name, others, "dev")?;
        match self.dev_id.get_mut(&id) {
            Some(d) if d.get_name() != name => {
                d.set_name(name);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Toglie il dev dall'anagrafica (il chiamante controlla che non sia in
    /// nessun progetto). Ritorna `true` se c'era.
    pub fn del(&mut self, id: DevId) -> bool {
        self.dev_id.remove(&id).is_some()
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
