use crate::workers_utils::worker::{DEFAULT_MAX_HOURS, WORKER_ID_ZERO, WeekStatus, Worker, WorkerId};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

/// Ordine **alfabetico** dei nomi dei worker, unico per tutta l'app: senza
/// distinguere maiuscole/minuscole, a parità il nome esatto (ordine stabile).
pub fn name_order(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Workers {
    last_id: WorkerId,
    worker_id: HashMap<WorkerId, Worker>,
}

impl Workers {
    pub fn new() -> Self {
        Self {
            last_id: WorkerId(1),
            worker_id: HashMap::from([(WORKER_ID_ZERO, Worker::new(""))]),
        }
    }

    pub fn get_id_by_name(&self, name: &str) -> Option<WorkerId> {
        self.worker_id
            .iter()
            .find(|(_, w)| w.name == name)
            .map(|(id, _)| *id)
    }

    pub fn get_name_by_id(&self, worker_id: WorkerId) -> &str {
        self.worker_id
            .get(&worker_id)
            .map(|s| s.name.as_str())
            .unwrap_or("")
    }

    pub fn add(&mut self, name: &str) -> WorkerId {
        self.get_id_by_name(name).unwrap_or_else(|| {
            let id = self.last_id;
            self.worker_id.insert(self.last_id, Worker::new(name));
            self.last_id.0 += 1;
            id
        })
    }

    /// Toglie il worker dall'anagrafica (il chiamante controlla che non abbia
    /// celle). Il worker «zero» interno non si toglie. Ritorna `true` se c'era.
    pub fn del(&mut self, id: WorkerId) -> bool {
        id != WORKER_ID_ZERO && self.worker_id.remove(&id).is_some()
    }

    /// Rinomina il worker (stesse regole di `check_new_name`: nelle celle il
    /// nome è seguito da `|`, quindi non può contenerlo); `Ok(false)` se il
    /// nome non cambia.
    pub fn rename(&mut self, id: WorkerId, name: &str) -> Result<bool, String> {
        let name = name.trim();
        let others = self.list().into_iter().filter(|(w, _)| *w != id).map(|(_, n)| n);
        crate::app::check_new_name(name, others, "worker")?;
        match self.worker_id.get_mut(&id) {
            Some(w) if id != WORKER_ID_ZERO && w.name != name => {
                w.name = name.to_string();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Worker nascosto (archiviato): né nel footer né negli elenchi, cioè le
    /// due spunte di «Filtri ▸ Worker (visibilità e ghost)…» entrambe spente.
    pub fn is_archived(&self, id: WorkerId) -> bool {
        self.is_hidden_in_footer(id) && !self.is_shown_in_find(id)
    }

    pub fn get_max_hours(&self, id: WorkerId) -> u32 {
        self.worker_id
            .get(&id)
            .map_or(DEFAULT_MAX_HOURS, |w| w.get_max_hours())
    }

    pub fn set_max_hours(&mut self, id: WorkerId, hours: u32) {
        if let Some(w) = self.worker_id.get_mut(&id) {
            w.set_max_hours(hours);
        }
    }

    pub fn get_effective_max_hours(&self, id: WorkerId, week: usize) -> u32 {
        self.worker_id.get(&id).map_or(DEFAULT_MAX_HOURS, |w| {
            w.get_effective_max_hours_for_week(week)
        })
    }

    pub fn set_week_override(&mut self, id: WorkerId, week: usize, hours: u32) {
        if let Some(w) = self.worker_id.get_mut(&id) {
            w.set_week_override(week, hours);
        }
    }

    pub fn get_week_note(&self, id: WorkerId, week: usize) -> String {
        self.worker_id
            .get(&id)
            .and_then(|w| w.get_week_note(week))
            .unwrap_or("")
            .to_string()
    }

    pub fn has_week_note(&self, id: WorkerId, week: usize) -> bool {
        self.worker_id
            .get(&id)
            .is_some_and(|w| w.has_week_note(week))
    }

    pub fn set_week_note(&mut self, id: WorkerId, week: usize, note: &str) {
        if let Some(w) = self.worker_id.get_mut(&id) {
            w.set_week_note(week, note);
        }
    }

    pub fn get_week_status(&self, id: WorkerId, week: usize) -> Option<WeekStatus> {
        self.worker_id
            .get(&id)
            .and_then(|w| w.get_week_status(week))
    }

    pub fn set_week_status(&mut self, id: WorkerId, week: usize, status: Option<WeekStatus>) {
        if let Some(w) = self.worker_id.get_mut(&id) {
            w.set_week_status(week, status);
        }
    }

    pub fn is_hidden_in_footer(&self, id: WorkerId) -> bool {
        self.worker_id
            .get(&id)
            .is_some_and(|w| w.is_hidden_in_footer())
    }

    /// True se il worker va mostrato nel filtro (Ctrl+F). Default `true` (anche
    /// per un id sconosciuto).
    pub fn is_shown_in_find(&self, id: WorkerId) -> bool {
        self.worker_id.get(&id).is_none_or(|w| w.is_shown_in_find())
    }

    /// True se il worker è di tipo "ghost". Default `false` (anche per un id
    /// sconosciuto).
    pub fn is_ghost(&self, id: WorkerId) -> bool {
        self.worker_id.get(&id).is_some_and(|w| w.is_ghost())
    }

    /// Nasconde/mostra la riga del worker nel footer. Ritorna `true` se il
    /// valore è cambiato.
    pub fn set_hidden_in_footer(&mut self, id: WorkerId, hidden: bool) -> bool {
        match self.worker_id.get_mut(&id) {
            Some(w) if w.is_hidden_in_footer() != hidden => {
                w.set_hidden_in_footer(hidden);
                true
            }
            _ => false,
        }
    }

    /// Fa comparire o no il worker negli elenchi di scelta (filtri, matrice,
    /// inserimento multiplo, saturazione). Ritorna `true` se è cambiato.
    pub fn set_shown_in_find(&mut self, id: WorkerId, shown: bool) -> bool {
        match self.worker_id.get_mut(&id) {
            Some(w) if w.is_shown_in_find() != shown => {
                w.set_shown_in_find(shown);
                true
            }
            _ => false,
        }
    }

    pub fn set_ghost(&mut self, id: WorkerId, ghost: bool) {
        if let Some(w) = self.worker_id.get_mut(&id) {
            w.set_ghost(ghost);
        }
    }

    pub fn list(&self) -> Vec<(WorkerId, String)> {
        let mut items: Vec<(WorkerId, String)> = self
            .worker_id
            .iter()
            .filter(|(w_id, _)| w_id != &&WORKER_ID_ZERO)
            .map(|(&id, w)| (id, w.name.clone()))
            .collect();
        items.sort_by(|(a_id, a), (b_id, b)| name_order(a, b).then(a_id.0.cmp(&b_id.0)));
        items
    }

    /// Ordina degli id di worker in ordine alfabetico del nome (`name_order`).
    pub fn sort_by_name(&self, ids: &mut [WorkerId]) {
        ids.sort_by(|a, b| name_order(self.get_name_by_id(*a), self.get_name_by_id(*b)).then(a.0.cmp(&b.0)));
    }

    /// Ordine alfabetico fra due worker (per gli ordinamenti composti, es.
    /// settimana poi worker).
    pub fn cmp_by_name(&self, a: WorkerId, b: WorkerId) -> Ordering {
        name_order(self.get_name_by_id(a), self.get_name_by_id(b)).then(a.0.cmp(&b.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_with_zero_worker() {
        let ws = Workers::new();
        assert_eq!(ws.get_name_by_id(WORKER_ID_ZERO), "");
    }

    #[test]
    fn show_in_find_defaults_true_when_absent_from_ron() {
        // Un worker serializzato senza il campo (com'era prima della feature).
        let w: Worker = ron::from_str(r#"(name: "Alice")"#).expect("deserializza");
        assert!(w.is_shown_in_find(), "assente nel .ron ⇒ default true");
    }

    #[test]
    fn show_in_find_true_is_not_serialized() {
        let w = Worker::new("Alice"); // default: show_in_find = true
        let ron = ron::ser::to_string_pretty(&w, ron::ser::PrettyConfig::default()).unwrap();
        assert!(
            !ron.contains("show_in_find"),
            "col valore di default (true) il campo non va salvato: {ron}"
        );
    }

    #[test]
    fn show_in_find_false_round_trips_through_ron() {
        let mut w = Worker::new("Alice");
        w.set_shown_in_find(false);
        let ron = ron::ser::to_string_pretty(&w, ron::ser::PrettyConfig::default()).unwrap();
        assert!(ron.contains("show_in_find"), "false va salvato: {ron}");
        let back: Worker = ron::from_str(&ron).expect("deserializza");
        assert!(!back.is_shown_in_find(), "false deve persistere");
    }

    #[test]
    fn new_list_is_empty() {
        let ws = Workers::new();
        assert!(ws.list().is_empty());
    }

    #[test]
    fn add_returns_new_id() {
        let mut ws = Workers::new();
        let id = ws.add("Alice");
        assert_eq!(id, WorkerId(1));
    }

    #[test]
    fn add_increments_id() {
        let mut ws = Workers::new();
        let id1 = ws.add("Alice");
        let id2 = ws.add("Bob");
        assert_ne!(id1, id2);
        assert_eq!(id1, WorkerId(1));
        assert_eq!(id2, WorkerId(2));
    }

    #[test]
    fn add_duplicate_returns_existing_id() {
        let mut ws = Workers::new();
        let id1 = ws.add("Alice");
        let id2 = ws.add("Alice");
        assert_eq!(id1, id2);
        assert_eq!(ws.list().len(), 1);
    }

    #[test]
    fn get_id_by_name_found() {
        let mut ws = Workers::new();
        let id = ws.add("Alice");
        assert_eq!(ws.get_id_by_name("Alice"), Some(id));
    }

    #[test]
    fn get_id_by_name_not_found() {
        let ws = Workers::new();
        assert_eq!(ws.get_id_by_name("Ghost"), None);
    }

    #[test]
    fn get_name_by_id_found() {
        let mut ws = Workers::new();
        let id = ws.add("Alice");
        assert_eq!(ws.get_name_by_id(id), "Alice");
    }

    #[test]
    fn get_name_by_id_not_found_returns_empty() {
        let ws = Workers::new();
        assert_eq!(ws.get_name_by_id(WorkerId(99)), "");
    }

    #[test]
    fn del_removes_worker() {
        let mut ws = Workers::new();
        let id = ws.add("Alice");
        ws.del(id);
        assert_eq!(ws.get_id_by_name("Alice"), None);
        assert!(ws.list().is_empty());
    }

    #[test]
    fn del_nonexistent_does_not_panic() {
        let mut ws = Workers::new();
        ws.del(WorkerId(99)); // should not panic
    }

    #[test]
    fn list_excludes_worker_id_zero() {
        let ws = Workers::new();
        let list = ws.list();
        assert!(list.iter().all(|(id, _)| *id != WORKER_ID_ZERO));
    }

    #[test]
    fn list_is_sorted_alphabetically() {
        let mut ws = Workers::new();
        ws.add("Charlie");
        ws.add("Alice");
        ws.add("Bob");
        let list = ws.list();
        let names: Vec<&str> = list.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn alphabetical_order_ignores_case() {
        let mut ws = Workers::new();
        let zed = ws.add("Zed");
        let anna = ws.add("anna");
        let bob = ws.add("Bob");
        let names: Vec<String> = ws.list().into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, vec!["anna", "Bob", "Zed"]);
        let mut ids = vec![zed, bob, anna];
        ws.sort_by_name(&mut ids);
        assert_eq!(ids, vec![anna, bob, zed]);
    }

    #[test]
    fn list_contains_all_workers() {
        let mut ws = Workers::new();
        ws.add("Alice");
        ws.add("Bob");
        assert_eq!(ws.list().len(), 2);
    }
}
