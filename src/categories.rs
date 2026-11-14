use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct CategoryId(pub usize);

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Categories {
    last_id: CategoryId,
    categories: HashMap<CategoryId, String>,
    /// Categorie nascoste (archiviate): restano sui progetti che le hanno, ma
    /// non sono più proposte nelle scelte. Omesso dal file quando vuoto.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    hidden: BTreeSet<CategoryId>,
}

impl Categories {
    pub fn new() -> Self {
        Self {
            last_id: CategoryId(0),
            categories: HashMap::new(),
            hidden: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, name: &str) -> CategoryId {
        let id = self.last_id;
        self.categories.insert(id, name.to_string());
        self.last_id.0 += 1;
        id
    }

    pub fn list(&self) -> Vec<(CategoryId, String)> {
        let mut items: Vec<(CategoryId, String)> = self
            .categories
            .iter()
            .map(|(&id, name)| (id, name.clone()))
            .collect();
        items.sort_by_key(|(id, _)| *id);
        items
    }

    pub fn get_name(&self, id: CategoryId) -> Option<&str> {
        self.categories.get(&id).map(|s| s.as_str())
    }

    pub fn is_hidden(&self, id: CategoryId) -> bool {
        self.hidden.contains(&id)
    }

    /// Nasconde / rimostra la categoria. Ritorna `true` se è cambiato.
    pub fn set_hidden(&mut self, id: CategoryId, hidden: bool) -> bool {
        if !self.categories.contains_key(&id) {
            return false;
        }
        if hidden {
            self.hidden.insert(id)
        } else {
            self.hidden.remove(&id)
        }
    }

    /// Rinomina la categoria (stesse regole di `check_new_name`); `Ok(false)`
    /// se il nome non cambia.
    pub fn rename(&mut self, id: CategoryId, name: &str) -> Result<bool, String> {
        let name = name.trim();
        let others = self.list().into_iter().filter(|(c, _)| *c != id).map(|(_, n)| n);
        crate::app::check_new_name(name, others, "categoria")?;
        match self.categories.get_mut(&id) {
            Some(n) if n != name => {
                *n = name.to_string();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Toglie la categoria (il chiamante controlla che nessun progetto la
    /// usi). Ritorna `true` se c'era.
    pub fn del(&mut self, id: CategoryId) -> bool {
        self.hidden.remove(&id);
        self.categories.remove(&id).is_some()
    }
}

impl Default for Categories {
    fn default() -> Self {
        Self::new()
    }
}
