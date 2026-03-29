#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Default)]
pub struct Effort(pub usize);

#[derive(Serialize, Deserialize)]
pub struct SingleEffort {
    effort: Effort,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl SingleEffort {
    pub fn new(effort: Effort) -> Self {
        Self { effort, note: None }
    }

    pub fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
    }

    pub fn set_note(&mut self, note: &str) {
        self.note = if note.is_empty() { None } else { Some(note.to_string()) };
    }

    pub fn get_note(&self) -> String {
        self.note.clone().unwrap_or(String::from(""))
    }

    pub fn get_effort(&self) -> Effort {
        self.effort
    }
}
