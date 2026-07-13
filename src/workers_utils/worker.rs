#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const WORKER_ID_ZERO: WorkerId = WorkerId(0);
pub const DEFAULT_MAX_HOURS: u32 = 40;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct WorkerId(pub usize);

/// Stato di una settimana per un worker: ferie o malattia.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum WeekStatus {
    Ferie,
    Malattia,
}

/// Default per `show_in_find`: un worker è mostrato nel filtro (Ctrl+F) se non
/// specificato diversamente.
fn show_in_find_default() -> bool {
    true
}

/// Usata da `skip_serializing_if`: non salvare il campo quando è al valore di
/// default (`true`).
fn is_true(b: &bool) -> bool {
    *b
}

/// Usata da `skip_serializing_if`: non salvare il campo quando è `false`
/// (valore di default, es. per `ghost`).
fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Worker {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<usize>,
    /// Global max hours per week for this worker. None = 40h default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_weekly_hours: Option<u32>,
    /// Per-week overrides: WeekId.0 → max hours for that specific week.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub week_overrides: HashMap<usize, u32>,
    /// Per-week notes: WeekId.0 → testo nota per quella specifica settimana.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub week_notes: HashMap<usize, String>,
    /// Per-week status: WeekId.0 → ferie/malattia per quella specifica settimana.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub week_status: HashMap<usize, WeekStatus>,
    /// When true, the worker row is hidden in the right-footer (totals are still counted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_in_footer: Option<bool>,
    /// When true (default), the worker appears in the Ctrl+F filter list. Default
    /// is `true` even if the field is absent from the .ron; it is not serialized
    /// while it stays `true`.
    #[serde(default = "show_in_find_default", skip_serializing_if = "is_true")]
    pub show_in_find: bool,
    /// Worker "ghost": quando inserito negli effort di un dev viene evidenziato
    /// come anomalia — cella sempre rossa (max effort di fatto 0), nome del dev
    /// lampeggiante, barra/linea rossa negli export. Default `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ghost: bool,
}

impl Worker {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bg_color: None,
            font_color: None,
            max_weekly_hours: None,
            week_overrides: HashMap::new(),
            week_notes: HashMap::new(),
            week_status: HashMap::new(),
            hide_in_footer: None,
            show_in_find: true,
            ghost: false,
        }
    }

    pub fn is_ghost(&self) -> bool {
        self.ghost
    }

    pub fn set_ghost(&mut self, ghost: bool) {
        self.ghost = ghost;
    }

    pub fn get_week_note(&self, week: usize) -> Option<&str> {
        self.week_notes.get(&week).map(|s| s.as_str())
    }

    pub fn has_week_note(&self, week: usize) -> bool {
        self.week_notes.get(&week).is_some_and(|s| !s.is_empty())
    }

    /// Imposta (o rimuove, se vuota) la nota della settimana indicata.
    pub fn set_week_note(&mut self, week: usize, note: &str) {
        if note.is_empty() {
            self.week_notes.remove(&week);
        } else {
            self.week_notes.insert(week, note.to_string());
        }
    }

    pub fn get_week_status(&self, week: usize) -> Option<WeekStatus> {
        self.week_status.get(&week).copied()
    }

    /// Imposta (o rimuove, se `None`) lo stato della settimana indicata.
    pub fn set_week_status(&mut self, week: usize, status: Option<WeekStatus>) {
        match status {
            Some(s) => {
                self.week_status.insert(week, s);
            }
            None => {
                self.week_status.remove(&week);
            }
        }
    }

    pub fn is_hidden_in_footer(&self) -> bool {
        self.hide_in_footer.unwrap_or(false)
    }

    /// True se il worker va mostrato nella lista del filtro (Ctrl+F).
    pub fn is_shown_in_find(&self) -> bool {
        self.show_in_find
    }

    pub fn set_shown_in_find(&mut self, shown: bool) {
        self.show_in_find = shown;
    }

    pub fn get_max_hours(&self) -> u32 {
        self.max_weekly_hours.unwrap_or(DEFAULT_MAX_HOURS)
    }

    pub fn set_max_hours(&mut self, hours: u32) {
        self.max_weekly_hours = if hours == DEFAULT_MAX_HOURS { None } else { Some(hours) };
    }

    /// Returns effective max hours for the given week (override → global → default).
    pub fn get_effective_max_hours_for_week(&self, week: usize) -> u32 {
        self.week_overrides
            .get(&week)
            .copied()
            .unwrap_or_else(|| self.get_max_hours())
    }

    /// Sets a per-week override. Removes it if equal to the global max (no-op override).
    pub fn set_week_override(&mut self, week: usize, hours: u32) {
        if hours == self.get_max_hours() {
            self.week_overrides.remove(&week);
        } else {
            self.week_overrides.insert(week, hours);
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_bg_color(&mut self, color: Option<usize>) {
        self.bg_color = color;
    }
    pub fn set_font_color(&mut self, color: Option<usize>) {
        self.font_color = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_id_zero_is_zero() {
        assert_eq!(WORKER_ID_ZERO.0, 0);
    }

    #[test]
    fn worker_id_equality() {
        assert_eq!(WorkerId(1), WorkerId(1));
        assert_ne!(WorkerId(1), WorkerId(2));
    }

    #[test]
    fn worker_id_copy() {
        let id = WorkerId(5);
        let id2 = id;
        assert_eq!(id, id2);
    }

    #[test]
    fn worker_new_sets_name_and_no_colors() {
        let w = Worker::new("Alice");
        assert_eq!(w.name, "Alice");
        assert!(w.bg_color.is_none());
        assert!(w.font_color.is_none());
    }

    #[test]
    fn worker_new_empty_name() {
        let w = Worker::new("");
        assert_eq!(w.name, "");
    }

    #[test]
    fn set_name_updates_name() {
        let mut w = Worker::new("Alice");
        w.set_name("Bob".to_string());
        assert_eq!(w.name, "Bob");
    }

    #[test]
    fn set_bg_color_some() {
        let mut w = Worker::new("Alice");
        w.set_bg_color(Some(0xFF0000));
        assert_eq!(w.bg_color, Some(0xFF0000));
    }

    #[test]
    fn set_bg_color_none() {
        let mut w = Worker::new("Alice");
        w.set_bg_color(Some(42));
        w.set_bg_color(None);
        assert!(w.bg_color.is_none());
    }

    #[test]
    fn set_font_color_some() {
        let mut w = Worker::new("Alice");
        w.set_font_color(Some(0x00FF00));
        assert_eq!(w.font_color, Some(0x00FF00));
    }

    #[test]
    fn set_font_color_none() {
        let mut w = Worker::new("Alice");
        w.set_font_color(Some(99));
        w.set_font_color(None);
        assert!(w.font_color.is_none());
    }

    #[test]
    fn ghost_defaults_false_and_is_not_serialized() {
        let w = Worker::new("Alice");
        assert!(!w.is_ghost());
        let ron = ron::ser::to_string_pretty(&w, ron::ser::PrettyConfig::default()).unwrap();
        assert!(!ron.contains("ghost"), "col default (false) non va salvato: {ron}");
    }

    #[test]
    fn ghost_true_round_trips_through_ron() {
        let mut w = Worker::new("Alice");
        w.set_ghost(true);
        let ron = ron::ser::to_string_pretty(&w, ron::ser::PrettyConfig::default()).unwrap();
        assert!(ron.contains("ghost"), "true va salvato: {ron}");
        let back: Worker = ron::from_str(&ron).expect("deserializza");
        assert!(back.is_ghost(), "true deve persistere");
    }

    #[test]
    fn ghost_absent_from_old_ron_defaults_false() {
        let w: Worker = ron::from_str(r#"(name: "Alice")"#).expect("deserializza");
        assert!(!w.is_ghost(), "assente nel .ron ⇒ default false");
    }
}
