//! Date and time utility functions for week-based calculations.

use chrono::{Datelike, NaiveDate, Utc};

use crate::date_utils::day::DayDto;

/// Converts a NaiveDate to days since Unix epoch.
///
/// # Arguments
/// * `dt` - A reference to a NaiveDate
///
/// # Returns
/// Number of days since Unix epoch (1970-01-01)
///
/// # Examples
/// ```
/// # use chrono::NaiveDate;
/// # use project_app::date_utils::local_to_days;
/// let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
/// let days = local_to_days(&date);
/// assert!(days > 19700); // Days since 1970
/// ```
pub fn local_to_days(dt: &NaiveDate) -> i32 {
    dt.to_epoch_days()
}

/// Converts days since Unix epoch to a NaiveDate.
///
/// # Arguments
/// * `days` - Number of days since Unix epoch
///
/// # Returns
/// A NaiveDate corresponding to the given days
///
/// # Panics
/// Panics if the days value is out of valid range
///
/// # Examples
/// ```
/// # use project_app::date_utils::{local_to_days, days_to_local};
/// # use chrono::NaiveDate;
/// let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
/// let days = local_to_days(&date);
/// let converted = days_to_local(days);
/// assert_eq!(date, converted);
/// ```
pub fn days_to_local(days: i32) -> NaiveDate {
    NaiveDate::from_epoch_days(days).unwrap()
}

/// Returns the first day (Monday) of the week for a given date.
///
/// # Arguments
/// * `data` - A reference to a NaiveDate
///
/// # Returns
/// The Monday of the week containing the given date
///
/// # Examples
/// ```
/// # use chrono::NaiveDate;
/// # use project_app::date_utils::primo_giorno_settimana_corrente;
/// // Wednesday, January 3, 2024
/// let date = NaiveDate::from_ymd_opt(2024, 1, 3).unwrap();
/// let monday = primo_giorno_settimana_corrente(&date);
/// // Should return Monday, January 1, 2024
/// assert_eq!(monday, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
/// ```
pub fn primo_giorno_settimana_corrente(data: &chrono::NaiveDate) -> chrono::NaiveDate {
    let giorni_da_lunedi = data.weekday().num_days_from_monday();
    *data - chrono::Duration::days(giorni_da_lunedi as i64)
}

/// Generates a list of weeks between two dates.
///
/// # Arguments
/// * `start_date` - Start date for the week list
/// * `end_date` - End date for the week list
///
/// # Returns
/// A vector of DayDto representing each Monday between the dates
///
/// # Examples
/// ```
/// # use chrono::NaiveDate;
/// # use project_app::date_utils::weeks_list;
/// let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
/// let end = NaiveDate::from_ymd_opt(2024, 1, 29).unwrap();
/// let weeks = weeks_list(&start, &end);
/// assert_eq!(weeks.len(), 4); // 4 Mondays in this range
/// ```
pub fn weeks_list(start_date: &chrono::NaiveDate, end_date: &chrono::NaiveDate) -> Vec<DayDto> {
    let mut weeks: Vec<DayDto> = Vec::new();

    let mut start_week = primo_giorno_settimana_corrente(start_date);
    let end_week = primo_giorno_settimana_corrente(end_date);

    while start_week < end_week {
        weeks.push(DayDto::new(local_to_days(&start_week)));
        start_week += chrono::Duration::days(7);
        start_week = primo_giorno_settimana_corrente(&start_week);
    }

    weeks
}

/// Returns default week range (52 weeks from current week).
///
/// # Returns
/// A tuple of (number_of_weeks, start_week_days, end_week_days)
///
/// # Examples
/// ```
/// # use project_app::date_utils::get_default_weeks;
/// let (num_weeks, start, end) = get_default_weeks();
/// assert_eq!(num_weeks, 52);
/// assert!(end > start);
/// ```
pub fn get_default_weeks(start: Option<i32>) -> (i32, i32, i32) {
    let today = Utc::now().date_naive();
    let start_date = local_to_days(&primo_giorno_settimana_corrente(&today));
    let start_date = start.map_or(start_date, |s| start_date.min(s));
    let end_date = local_to_days(&primo_giorno_settimana_corrente(
        &(today + chrono::Duration::weeks(52)),
    ));

    ((end_date - start_date) / 7, start_date, end_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

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

    #[test]
    fn test_weeks_list() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 29).unwrap();
        let weeks = weeks_list(&start, &end);
        assert_eq!(weeks.len(), 4);
    }
}
