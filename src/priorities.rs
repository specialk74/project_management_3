//! Priorità dei progetti (es. Must / Should / Could): elenco libero, gestito
//! come le categorie (aggiungi, rinomina, nascondi, elimina se non usata).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct PriorityId(pub usize);

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Priorities {
    last_id: PriorityId,
    priorities: BTreeMap<PriorityId, String>,
    /// Priorità nascoste (archiviate): restano sui progetti che le hanno, ma
    /// non sono più proposte nelle scelte. Omesso dal file quando vuoto.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    hidden: BTreeSet<PriorityId>,
}

impl Priorities {
    pub fn new() -> Self {
        Self {
            last_id: PriorityId(0),
            priorities: BTreeMap::new(),
            hidden: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, name: &str) -> PriorityId {
        let id = self.last_id;
        self.priorities.insert(id, name.to_string());
        self.last_id.0 += 1;
        id
    }

    pub fn list(&self) -> Vec<(PriorityId, String)> {
        let mut items: Vec<(PriorityId, String)> = self
            .priorities
            .iter()
            .map(|(&id, name)| (id, name.clone()))
            .collect();
        items.sort_by_key(|(id, _)| *id);
        items
    }

    pub fn get_name(&self, id: PriorityId) -> Option<&str> {
        self.priorities.get(&id).map(|s| s.as_str())
    }

    pub fn is_hidden(&self, id: PriorityId) -> bool {
        self.hidden.contains(&id)
    }

    /// Nasconde / rimostra la priorità. Ritorna `true` se è cambiato.
    pub fn set_hidden(&mut self, id: PriorityId, hidden: bool) -> bool {
        if !self.priorities.contains_key(&id) {
            return false;
        }
        if hidden {
            self.hidden.insert(id)
        } else {
            self.hidden.remove(&id)
        }
    }

    /// Rinomina la priorità (stesse regole di `check_new_name`); `Ok(false)`
    /// se il nome non cambia.
    pub fn rename(&mut self, id: PriorityId, name: &str) -> Result<bool, String> {
        let name = name.trim();
        let others = self.list().into_iter().filter(|(c, _)| *c != id).map(|(_, n)| n);
        crate::app::check_new_name(name, others, "priorità")?;
        match self.priorities.get_mut(&id) {
            Some(n) if n != name => {
                *n = name.to_string();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Toglie la priorità (il chiamante controlla che nessun progetto la
    /// usi). Ritorna `true` se c'era.
    pub fn del(&mut self, id: PriorityId) -> bool {
        self.hidden.remove(&id);
        self.priorities.remove(&id).is_some()
    }
}

impl Default for Priorities {
    fn default() -> Self {
        Self::new()
    }
}

/// Valori proposti in un file nuovo o scritto prima delle priorità (MoSCoW);
/// l'utente li rinomina, ne aggiunge o li elimina come le categorie.
pub const DEFAULT_PRIORITIES: [&str; 3] = ["Must", "Should", "Could"];

impl Priorities {
    /// Elenco iniziale con `DEFAULT_PRIORITIES`.
    pub fn with_defaults() -> Self {
        let mut p = Self::new();
        for name in DEFAULT_PRIORITIES {
            p.add(name);
        }
        p
    }
}
