//! ISO 8601 timestamps from Jellyfin ("2024-05-01T12:34:56.1234567Z") to UTC
//! milliseconds, without a date library.

/// Days from 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// UTC milliseconds, or None when the text is not a timestamp. A missing zone is
/// read as UTC, which is what Jellyfin means.
pub fn parse_utc_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || (b[10] != b'T' && b[10] != b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
        num(17..19)?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut rest = &s[19..];
    let mut millis = 0i64;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits: String = frac.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        let padded = format!("{:0<3}", &digits[..digits.len().min(3)]);
        millis = padded.parse().ok()?;
        rest = &frac[digits.len()..];
    }
    let offset_min = match rest {
        "" | "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hm = rest[1..].replace(':', "");
            if hm.len() != 4 {
                return None;
            }
            sign * (hm[..2].parse::<i64>().ok()? * 60 + hm[2..].parse::<i64>().ok()?)
        }
    };
    let days = days_from_civil(y, mo, d);
    Some(((days * 86_400 + h * 3600 + mi * 60 + se) - offset_min * 60) * 1000 + millis)
}

/// The HTTP `Date` header (RFC 9110 IMF-fixdate, "Thu, 06 Nov 2026 15:04:05 GMT")
/// to UTC milliseconds.
pub fn parse_http_date(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 6 || parts[5] != "GMT" {
        return None;
    }
    let day: i64 = parts[1].parse().ok()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == parts[2])? as i64
        + 1;
    let year: i64 = parts[3].parse().ok()?;
    let hms: Vec<i64> = parts[4]
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    if hms.len() != 3 || !(1..=31).contains(&day) || hms[0] > 23 || hms[1] > 59 || hms[2] > 60 {
        return None;
    }
    Some((days_from_civil(year, month, day) * 86_400 + hms[0] * 3600 + hms[1] * 60 + hms[2]) * 1000)
}

/// UTC milliseconds to "YYYY-MM-DDTHH:MM:SSZ", the form `minDateLastSaved` takes.
pub fn format_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_dates_and_back() {
        assert_eq!(
            parse_http_date("Thu, 06 Nov 2025 03:33:20 GMT"),
            Some(1_762_400_000_000)
        );
        assert_eq!(
            parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(784_111_777_000)
        );
        assert_eq!(parse_http_date("Thu, 06 Nov 2025 03:33:20 PST"), None);
        assert_eq!(parse_http_date("garbage"), None);
        assert_eq!(format_utc(1_762_400_000_000), "2025-11-06T03:33:20Z");
        assert_eq!(format_utc(951_825_600_500), "2000-02-29T12:00:00Z");
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        for ms in [0, 951_825_600_000, 1_762_400_000_000, 4_102_444_800_000] {
            assert_eq!(parse_utc_ms(&format_utc(ms)), Some(ms));
        }
    }

    #[test]
    fn jellyfin_timestamps() {
        assert_eq!(parse_utc_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_utc_ms("2025-11-06T03:33:20.0000000Z"),
            Some(1_762_400_000_000)
        );
        assert_eq!(
            parse_utc_ms("2025-11-06T03:33:20.1234567Z"),
            Some(1_762_400_000_123)
        );
        assert_eq!(
            parse_utc_ms("2025-11-05T22:33:20-05:00"),
            Some(1_762_400_000_000)
        );
        assert_eq!(parse_utc_ms("2025-11-06T03:33:20"), Some(1_762_400_000_000));
        assert_eq!(
            parse_utc_ms("2000-02-29T12:00:00.5Z"),
            Some(951_825_600_500)
        );
        assert_eq!(parse_utc_ms("1969-12-31T23:59:59Z"), Some(-1000));
    }

    #[test]
    fn rejects_what_is_not_a_timestamp() {
        for bad in [
            "",
            "yesterday",
            "2025-13-01T00:00:00Z",
            "2025-11-06",
            "2025-11-06T03:33:20.Z",
            "2025-11-06T03:33:20+5",
        ] {
            assert_eq!(parse_utc_ms(bad), None, "{bad}");
        }
    }
}
