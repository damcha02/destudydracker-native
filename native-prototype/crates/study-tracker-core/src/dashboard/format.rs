//! Number/date formatting the Dashboard shows, ported from production (`metrics.ts`,
//! `grades.ts`, `App.tsx` helpers). Kept in the core crate because these are *semantics*
//! (e.g. `formatMinutes(0) == "0m"`, JavaScript's `toFixed` rounding), not presentation styling:
//! the golden tests pin them against what production's WebView actually prints.

use super::civil::CivilDate;

const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// `en-GB` (ICU/CLDR as shipped in Chromium 154) abbreviates September as "Sept", unlike `en`.
const MONTHS_SHORT_GB: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec",
];
const WEEKDAYS_LONG: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const WEEKDAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// `formatMinutes`: `0 -> "0m"`, `45 -> "45m"`, `60 -> "1h"`, `95 -> "1h 35m"`.
pub fn format_minutes(minutes: u64) -> String {
    if minutes == 0 {
        return "0m".to_string();
    }
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let (hours, mins) = (minutes / 60, minutes % 60);
    if mins > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{hours}h")
    }
}

/// `Math.round`: halves round toward +infinity (Rust's `f64::round` rounds halves away from zero).
pub fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// `Number.prototype.toFixed(digits)`: exact decimal expansion of the double, ties rounded *up*
/// (Rust's `{:.N}` rounds exact ties to even, so `0.25` would print `0.2` where JS prints `0.3`).
pub fn to_fixed(value: f64, digits: usize) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".into()
        } else if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let negative = value < 0.0;
    // 60 fractional digits is exact for every double this app produces (they are all ratios of
    // small integers), so the digit after `digits` decides rounding with no binary-float error.
    let expanded = format!("{:.60}", value.abs());
    let (int_part, frac_part) = expanded.split_once('.').unwrap_or((&expanded, ""));
    let mut digits_vec: Vec<u8> = int_part
        .bytes()
        .chain(frac_part.bytes().take(digits))
        .map(|b| b - b'0')
        .collect();
    let round_up = frac_part.as_bytes().get(digits).is_some_and(|b| *b >= b'5');
    if round_up {
        let mut i = digits_vec.len();
        loop {
            if i == 0 {
                digits_vec.insert(0, 1);
                break;
            }
            i -= 1;
            if digits_vec[i] == 9 {
                digits_vec[i] = 0;
            } else {
                digits_vec[i] += 1;
                break;
            }
        }
    }
    let split = digits_vec.len() - digits;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    for (index, digit) in digits_vec.iter().enumerate() {
        if index == split && digits > 0 {
            out.push('.');
        }
        out.push((b'0' + digit) as char);
    }
    out
}

/// JavaScript's default `Number -> string`, for the values this app prints: integers without a
/// decimal point, other finite values with their shortest round-trip form.
pub fn js_number(value: f64) -> String {
    if value.is_finite() && value == value.trunc() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// `formatUnitAmount`: `1 -> "1 unit"`, `0.5 -> "1/2 unit"`, `0.25 -> "1/4 unit"`, else `N units`
/// (`toFixed(2)` for non-integers).
pub fn format_unit_amount(amount: f64) -> String {
    if amount == 1.0 {
        "1 unit".into()
    } else if amount == 0.5 {
        "1/2 unit".into()
    } else if amount == 0.25 {
        "1/4 unit".into()
    } else if amount == amount.trunc() {
        format!("{} units", js_number(amount))
    } else {
        format!("{} units", to_fixed(amount, 2))
    }
}

/// `formatUnitLabel(label, count)`: lower-cased, pluralized with a trailing `s` unless the label
/// already ends in `s` or the count is 1; an empty label reads "task".
pub fn format_unit_label(label: &str, count: u32) -> String {
    let clean = if label.trim().is_empty() {
        "Task"
    } else {
        label.trim()
    };
    if count == 1 || clean.ends_with('s') {
        clean.to_lowercase()
    } else {
        format!("{}s", clean.to_lowercase())
    }
}

/// `formatSwissGrade`: `5 -> "5.0"`, `5.5 -> "5.5"`, `4.25 -> "4.25"`.
pub fn format_swiss_grade(grade: f64) -> String {
    let fixed = to_fixed(grade, 2);
    if fixed.ends_with("00") || fixed.ends_with('0') {
        fixed[..fixed.len() - 1].to_string()
    } else {
        fixed
    }
}

/// `Intl.DateTimeFormat("en", { month: "short", day: "numeric" })`: `Oct 9`.
pub fn format_month_day(date: CivilDate) -> String {
    let (_, m, d) = date.ymd();
    format!("{} {}", MONTHS_SHORT[(m - 1) as usize], d)
}

/// `Intl.DateTimeFormat("en", { weekday: "long", day: "2-digit", month: "short", year:
/// "numeric" })`: `Wednesday, Sep 30, 2026`.
pub fn format_field_today_label(date: CivilDate) -> String {
    let (y, m, d) = date.ymd();
    format!(
        "{}, {} {:02}, {}",
        WEEKDAYS_LONG[date.weekday() as usize],
        MONTHS_SHORT[(m - 1) as usize],
        d,
        y
    )
}

/// `fossilDateLabel` = `Intl.DateTimeFormat("en-GB", { weekday: "short", day: "numeric", month:
/// "short" })`: `Wed 7 Oct`.
pub fn format_fossil_date_label(date: CivilDate) -> String {
    let (_, m, d) = date.ymd();
    format!(
        "{} {} {}",
        WEEKDAYS_SHORT[date.weekday() as usize],
        d,
        MONTHS_SHORT_GB[(m - 1) as usize]
    )
}

/// `Intl.DateTimeFormat("en", { weekday: "short" })`.
pub fn weekday_short(date: CivilDate) -> &'static str {
    WEEKDAYS_SHORT[date.weekday() as usize]
}

/// `Intl.DateTimeFormat("en", { month: "short" })`.
pub fn month_short(date: CivilDate) -> &'static str {
    MONTHS_SHORT[(date.month() - 1) as usize]
}

/// `getTimeGreeting` (only used by the non-default Modern dashboard; ported for completeness).
pub fn time_greeting(hour: u32) -> &'static str {
    match hour {
        5..=10 => "Good morning",
        11..=12 => "Good day",
        13..=16 => "Good afternoon",
        17..=21 => "Good evening",
        _ => "Good night",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_format_matches_production() {
        assert_eq!(format_minutes(0), "0m");
        assert_eq!(format_minutes(1), "1m");
        assert_eq!(format_minutes(59), "59m");
        assert_eq!(format_minutes(60), "1h");
        assert_eq!(format_minutes(95), "1h 35m");
        assert_eq!(format_minutes(5006), "83h 26m");
    }

    #[test]
    fn to_fixed_rounds_ties_up_like_javascript() {
        assert_eq!(
            to_fixed(0.25, 1),
            "0.3",
            "JS (0.25).toFixed(1) === '0.3'; Rust {{:.1}} would give 0.2"
        );
        assert_eq!(
            to_fixed(4.65, 1),
            "4.7",
            "4.65 is 4.6500000000000003553 in binary"
        );
        assert_eq!(
            to_fixed(1.005, 2),
            "1.00",
            "1.005 is 1.00499999999999989... in binary"
        );
        assert_eq!(to_fixed(4.7, 1), "4.7");
        assert_eq!(to_fixed(0.0, 1), "0.0");
        assert_eq!(to_fixed(9.96, 1), "10.0");
        assert_eq!(to_fixed(2.0, 2), "2.00");
        assert_eq!(to_fixed(2.5, 0), "3", "tie rounds up");
        assert_eq!(
            to_fixed(-0.04, 1),
            "-0.0",
            "JS keeps the sign for x < 0: (-0.04).toFixed(1) === '-0.0'"
        );
    }

    #[test]
    fn js_round_rounds_halves_up() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(61.5), 62.0);
        assert_eq!(js_round(61.49), 61.0);
    }

    #[test]
    fn unit_amounts_and_labels() {
        assert_eq!(format_unit_amount(1.0), "1 unit");
        assert_eq!(format_unit_amount(0.5), "1/2 unit");
        assert_eq!(format_unit_amount(0.25), "1/4 unit");
        assert_eq!(format_unit_amount(45.0), "45 units");
        assert_eq!(format_unit_amount(0.0), "0 units");
        assert_eq!(format_unit_amount(1.75), "1.75 units");
        assert_eq!(format_unit_label("Lecture", 2), "lectures");
        assert_eq!(format_unit_label("Lecture", 1), "lecture");
        assert_eq!(format_unit_label("Lectures", 2), "lectures");
        assert_eq!(format_unit_label("  ", 3), "tasks");
    }

    #[test]
    fn grades_and_numbers() {
        assert_eq!(format_swiss_grade(5.0), "5.0");
        assert_eq!(format_swiss_grade(5.5), "5.5");
        assert_eq!(format_swiss_grade(4.25), "4.25");
        assert_eq!(format_swiss_grade(6.0), "6.0");
        assert_eq!(js_number(40.0), "40");
        assert_eq!(js_number(0.4), "0.4");
        assert_eq!(js_number(62.5), "62.5");
    }

    #[test]
    fn dates_match_the_intl_output_captured_from_chromium() {
        // Captured from the real production WebView engine (probe-intl.js), not from memory.
        let wed = CivilDate::from_ymd(2026, 9, 9).unwrap();
        assert_eq!(format_fossil_date_label(wed), "Wed 9 Sept");
        assert_eq!(format_month_day(wed), "Sep 9");
        assert_eq!(format_field_today_label(wed), "Wednesday, Sep 09, 2026");
        let oct = CivilDate::from_ymd(2026, 10, 7).unwrap();
        assert_eq!(format_fossil_date_label(oct), "Wed 7 Oct");
        assert_eq!(
            format_field_today_label(CivilDate::from_ymd(2026, 1, 7).unwrap()),
            "Wednesday, Jan 07, 2026"
        );
        assert_eq!(
            weekday_short(CivilDate::from_ymd(2026, 2, 8).unwrap()),
            "Sun"
        );
    }

    #[test]
    fn greeting_boundaries() {
        assert_eq!(time_greeting(4), "Good night");
        assert_eq!(time_greeting(5), "Good morning");
        assert_eq!(time_greeting(11), "Good day");
        assert_eq!(time_greeting(13), "Good afternoon");
        assert_eq!(time_greeting(17), "Good evening");
        assert_eq!(time_greeting(22), "Good night");
    }
}
