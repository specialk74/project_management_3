#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const WORKER_ID_ZERO: WorkerId = WorkerId(0);

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
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
