//! Conversioni per il calendario `egui_extras::DatePickerButton`, che lavora con
//! `jiff::civil::Date`, mentre il resto del programma usa `chrono` e i giorni
//! dall'epoca (`WeekId`). Tutte le conversioni chrono ⇄ jiff passano da qui.

use chrono::{Datelike, NaiveDate};

use super::dates::{days_to_local, local_to_days, parse_ymd, today};

/// chrono → jiff. `None` se la data esce dall'intervallo di jiff (anni ±9999).
fn naive_to_jiff(d: NaiveDate) -> Option<jiff::civil::Date> {
    let year = i16::try_from(d.year()).ok()?;
    jiff::civil::Date::new(year, d.month() as i8, d.day() as i8).ok()
}

/// jiff → chrono.
fn jiff_to_naive(d: jiff::civil::Date) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32)
}

/// Data odierna (locale) per il calendario; 1/1/2000 solo se fuori intervallo.
pub fn today_jiff() -> jiff::civil::Date {
    naive_to_jiff(today()).unwrap_or(jiff::civil::date(2000, 1, 1))
}

/// Giorni dall'epoca → data del calendario (oggi se fuori intervallo).
pub fn days_to_jiff(days: i32) -> jiff::civil::Date {
    naive_to_jiff(days_to_local(days)).unwrap_or_else(today_jiff)
}

/// Data del calendario → giorni dall'epoca.
pub fn jiff_to_days(d: jiff::civil::Date) -> Option<i32> {
    jiff_to_naive(d).map(|n| local_to_days(&n))
}

/// Stringa `yy-mm-dd` / `yyyy-mm-dd` → data del calendario (il giorno esatto,
/// non allineato al lunedì). `None` se vuota o non valida.
pub fn date_str_to_jiff(s: &str) -> Option<jiff::civil::Date> {
    naive_to_jiff(parse_ymd(s)?)
}

/// Data del calendario → `yyyy-mm-dd` (accettato da `parse_date_str`, che poi
/// allinea al lunedì della settimana).
pub fn jiff_to_date_str(d: &jiff::civil::Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip_and_parse_like_the_rest_of_the_app() {
        let days = local_to_days(&NaiveDate::from_ymd_opt(2026, 10, 24).unwrap());
        let j = days_to_jiff(days);
        assert_eq!(j, jiff::civil::date(2026, 10, 24));
        assert_eq!(jiff_to_days(j), Some(days));
        assert_eq!(jiff_to_date_str(&j), "2026-10-24");
        // stessa regola di `parse_date_str`: anno a due cifre = 20yy, giorno esatto
        assert_eq!(date_str_to_jiff("26-10-24"), Some(j));
        assert_eq!(date_str_to_jiff(" 2026-10-24 "), Some(j));
        assert_eq!(date_str_to_jiff(&jiff_to_date_str(&j)), Some(j));
        for bad in ["", "26-13-01", "26-02-30", "26-10", "x-1-1"] {
            assert_eq!(date_str_to_jiff(bad), None, "{bad:?}");
        }
        assert_eq!(naive_to_jiff(NaiveDate::from_ymd_opt(20000, 1, 1).unwrap()), None);
    }
}
