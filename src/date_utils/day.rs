use serde::{Deserialize, Serialize};

use crate::date_utils::dates::days_to_local;

#[derive(Clone, Serialize, Deserialize)]
pub struct DayDto {
    pub week: i32,
    pub text: String,
}

impl DayDto {
    pub fn new(week: i32) -> Self {
        Self {
            week,
            text: days_to_local(week).format("%y-%m-%d").to_string(),
        }
    }
}

// Conversion implementations for DayData (from Slint)
impl From<crate::DayData> for DayDto {
    fn from(d: crate::DayData) -> Self {
        Self {
            week: d.week,
            text: d.text.to_string(),
        }
    }
}

impl From<DayDto> for crate::DayData {
    fn from(d: DayDto) -> Self {
        Self {
            week: d.week,
            text: d.text.into(),
        }
    }
}
