#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const WORKER_ID_ZERO: WorkerId = WorkerId(0);

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct WorkerId(pub usize);

#[derive(Serialize, Deserialize)]
pub struct Worker {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<usize>,
}

impl Worker {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bg_color: None,
            font_color: None,
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
}
