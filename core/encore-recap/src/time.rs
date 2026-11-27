//! Calendar arithmetic in integers only: no system clock, no time-zone database,
//! no locale. A play's local time is its UTC instant plus its own stored offset.

pub(crate) const MS_PER_MINUTE: i64 = 60_000;
pub(crate) const MS_PER_HOUR: i64 = 3_600_000;
pub(crate) const MS_PER_DAY: i64 = 86_400_000;

/// The play's local wall-clock instant, in ms since the epoch.
pub(crate) fn local_ms(utc_ms: i64, offset_minutes: i32) -> i64 {
    utc_ms.saturating_add(i64::from(offset_minutes) * MS_PER_MINUTE)
}

/// Days since 1970-01-01 of a local instant.
pub(crate) fn day_of(local_ms: i64) -> i64 {
    local_ms.div_euclid(MS_PER_DAY)
}

/// The local hour, 0 to 23.
pub(crate) fn hour_of(local_ms: i64) -> u32 {
    (local_ms.rem_euclid(MS_PER_DAY) / MS_PER_HOUR) as u32
}

/// 0 = Monday ... 6 = Sunday. 1970-01-01 was a Thursday.
pub(crate) fn weekday(day: i64) -> u32 {
    (day + 3).rem_euclid(7) as u32
}

/// Days since the epoch of a civil date (proleptic Gregorian; H. Hinnant's
/// algorithm).
pub(crate) fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from((month + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date (year, month 1-12, day 1-31) of a day number.
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// "YYYY-MM-DD" to a day number; None when it is not a real date.
pub(crate) fn parse_day(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = &s[r];
        if part.bytes().all(|c| c.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || d == 0 {
        return None;
    }
    let n = days_from_civil(i64::from(y), m, d);
    // Round-trips only when the day exists in that month.
    (civil_from_days(n) == (i64::from(y), m, d)).then_some(n)
}

/// A day number as "YYYY-MM-DD".
pub(crate) fn format_day(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(format_day(days_from_civil(2026, 3, 9)), "2026-03-09");
        assert_eq!(weekday(days_from_civil(2026, 3, 9)), 0); // a Monday
        assert_eq!(weekday(days_from_civil(2026, 11, 27)), 4); // a Friday
        for z in -800_000..800_000i64 {
            if z % 997 != 0 {
                continue;
            }
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        assert_eq!(parse_day("2028-02-29"), Some(days_from_civil(2028, 2, 29)));
        assert_eq!(parse_day("2026-02-29"), None);
        assert_eq!(parse_day("2026-8-03"), None);
        assert_eq!(parse_day("2026-13-01"), None);
    }

    #[test]
    fn a_play_lands_on_its_own_local_day() {
        // 23:30 on 31 Dec in UTC-5 is already 1 Jan in UTC.
        let utc = days_from_civil(2027, 1, 1) * MS_PER_DAY + 4 * MS_PER_HOUR + 30 * MS_PER_MINUTE;
        let local = local_ms(utc, -300);
        assert_eq!(format_day(day_of(local)), "2026-12-31");
        assert_eq!(hour_of(local), 23);
    }
}
