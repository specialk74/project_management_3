//! Date and time utility functions for week-based calculations.

use chrono::{Datelike, NaiveDate};

const NUM_DEFAULT_WEEK: i64 = 52 * 2;

/// Converts a NaiveDate to days since Unix epoch (1970-01-01).
pub fn local_to_days(dt: &NaiveDate) -> i32 {
    dt.to_epoch_days()
}

/// Converts days since Unix epoch to a NaiveDate.
///
/// Panics if `days` is out of chrono's valid range.
pub fn days_to_local(days: i32) -> NaiveDate {
    NaiveDate::from_epoch_days(days).unwrap()
}

/// Returns the first day (Monday) of the week containing `data`.
pub fn primo_giorno_settimana_corrente(data: &chrono::NaiveDate) -> chrono::NaiveDate {
    let giorni_da_lunedi = data.weekday().num_days_from_monday();
    *data - chrono::Duration::days(giorni_da_lunedi as i64)
}

/// Today's date in **local** time: the single source of "today" for the whole
/// app (grid, footer, exports). UTC would still show the previous day — and on
/// Monday the previous week — in the first hours after midnight in Italy.
pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// Today as days since Unix epoch (same unit as `WeekId`).
pub fn today_days() -> i32 {
    local_to_days(&today())
}

/// Monday of the current week as days since Unix epoch (same unit as `WeekId`).
pub fn current_monday_days() -> i32 {
    local_to_days(&primo_giorno_settimana_corrente(&today()))
}

/// Parses a date string in "yy-mm-dd" or "yyyy-mm-dd" format and returns the
/// Monday of that week as days since Unix epoch. Returns None on parse failure.
pub fn parse_date_str(s: &str) -> Option<i32> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    let year = if year < 100 { year + 2000 } else { year };
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let monday = primo_giorno_settimana_corrente(&date);
    Some(local_to_days(&monday))
}

/// Default week range: from the current week's Monday (or `start`, if earlier)
/// to `NUM_DEFAULT_WEEK` weeks after today.
/// Returns `(number_of_weeks, start_week_days, end_week_days)`.
pub fn get_default_weeks(start: Option<i32>) -> (i32, i32, i32) {
    let today = today();
    let start_date = local_to_days(&primo_giorno_settimana_corrente(&today));
    let start_date = start.map_or(start_date, |s| start_date.min(s));
    let end_date = local_to_days(&primo_giorno_settimana_corrente(
        &(today + chrono::Duration::weeks(NUM_DEFAULT_WEEK)),
    ));

    ((end_date - start_date) / 7, start_date, end_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn today_and_current_monday_agree() {
        let t = today();
        assert_eq!(today_days(), local_to_days(&t));
        let monday = current_monday_days();
        assert_eq!(days_to_local(monday).weekday(), chrono::Weekday::Mon);
        assert!(monday <= today_days() && today_days() < monday + 7);
    }

    #[test]
    fn test_local_to_days_and_back() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let days = local_to_days(&date);
        let converted = days_to_local(days);
        assert_eq!(date, converted);
    }

    #[test]
    fn test_primo_giorno_settimana_monday() {
        // Already a Monday
        let monday = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let result = primo_giorno_settimana_corrente(&monday);
        assert_eq!(result, monday);
    }

    #[test]
    fn test_primo_giorno_settimana_wednesday() {
        // Wednesday, should return Monday
        let wednesday = NaiveDate::from_ymd_opt(2024, 1, 3).unwrap();
        let monday = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let result = primo_giorno_settimana_corrente(&wednesday);
        assert_eq!(result, monday);
    }

    #[test]
    fn test_primo_giorno_settimana_sunday() {
        // Sunday, should return Monday of that week
        let sunday = NaiveDate::from_ymd_opt(2024, 1, 7).unwrap();
        let monday = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let result = primo_giorno_settimana_corrente(&sunday);
        assert_eq!(result, monday);
    }
}
