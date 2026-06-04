use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct CategoryId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct Categories {
    last_id: CategoryId,
    categories: HashMap<CategoryId, String>,
}

impl Categories {
    pub fn new() -> Self {
        Self {
            last_id: CategoryId(0),
            categories: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: &str) -> CategoryId {
        let id = self.last_id;
        self.categories.insert(id, name.to_string());
        self.last_id.0 += 1;
        id
    }

    pub fn remove(&mut self, id: CategoryId) {
        self.categories.remove(&id);
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
}

impl Default for Categories {
    fn default() -> Self {
        Self::new()
    }
}
