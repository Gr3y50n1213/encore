//! Time stamps in LRC and TTML, to milliseconds. Anything odd gives None, never a
//! panic or an overflow.

/// Up to three fraction digits as milliseconds: "5" is 500, "05" is 50, "1234" is 123.
fn fraction_ms(digits: &str) -> Option<i64> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut ms = 0i64;
    for (i, b) in digits.bytes().take(3).enumerate() {
        ms += i64::from(b - b'0') * [100, 10, 1][i];
    }
    Some(ms)
}

fn number(s: &str) -> Option<i64> {
    if s.is_empty() || s.len() > 9 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `seconds[.fraction]` with seconds under 60 unless `free` (the leading field).
fn seconds_ms(s: &str, free: bool) -> Option<i64> {
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    let secs = number(whole)?;
    if !free && secs >= 60 {
        return None;
    }
    Some(secs * 1000 + frac.map(fraction_ms).unwrap_or(Some(0))?)
}

/// LRC: `mm:ss`, `mm:ss.x`, `mm:ss.xx`, `mm:ss.xxx`, `mm:ss:xx` (some writers use a
/// colon before the hundredths) and `h:mm:ss.xx`. Minutes may run past 99.
pub(crate) fn lrc(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    match parts.as_slice() {
        [m, sec] => Some(number(m)? * 60_000 + seconds_ms(sec, false)?),
        // `mm:ss:xx` (fraction after a colon) or `h:mm:ss.xx`.
        [a, b, c] if !c.contains('.') && c.len() <= 3 && !b.contains('.') => {
            let m = number(a)?;
            let sec = number(b)?;
            if sec >= 60 {
                return None;
            }
            // A third field of two or three digits after mm:ss is a fraction.
            Some(m * 60_000 + sec * 1000 + fraction_ms(c)?)
        }
        [h, m, sec] => {
            let m = number(m)?;
            if m >= 60 {
                return None;
            }
            Some(number(h)? * 3_600_000 + m * 60_000 + seconds_ms(sec, false)?)
        }
        _ => None,
    }
}

/// TTML: clock times (`hh:mm:ss.fff`, `mm:ss.fff`, `ss.fff`) and offset times with
/// a unit (`12.3s`, `1500ms`, `1.5m`, `0.5h`). Frames and ticks are not used by
/// lyrics files and give None.
pub(crate) fn ttml(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        return match parts.as_slice() {
            [m, sec] => Some(number(m)? * 60_000 + seconds_ms(sec, false)?),
            [h, m, sec] => {
                let m = number(m)?;
                if m >= 60 {
                    return None;
                }
                Some(number(h)? * 3_600_000 + m * 60_000 + seconds_ms(sec, false)?)
            }
            _ => None,
        };
    }
    let (value, scale) = if let Some(v) = s.strip_suffix("ms") {
        (v, 1)
    } else if let Some(v) = s.strip_suffix('s') {
        (v, 1000)
    } else if let Some(v) = s.strip_suffix('m') {
        (v, 60_000)
    } else if let Some(v) = s.strip_suffix('h') {
        (v, 3_600_000)
    } else {
        (s, 1000) // a bare number is seconds
    };
    if value.is_empty() {
        return None;
    }
    let (whole, frac) = match value.split_once('.') {
        Some((w, f)) => (w, f),
        None => (value, ""),
    };
    let whole = if whole.is_empty() { 0 } else { number(whole)? };
    if !frac.is_empty() && !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // The fraction scaled exactly, without floating point: digits * scale / 10^n.
    let mut frac_ms = 0i64;
    let mut denom = 1i64;
    for b in frac.bytes().take(6) {
        frac_ms = frac_ms * 10 + i64::from(b - b'0');
        denom *= 10;
    }
    whole
        .checked_mul(scale)?
        .checked_add(frac_ms * scale / denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_times() {
        assert_eq!(lrc("00:12"), Some(12_000));
        assert_eq!(lrc("00:12.5"), Some(12_500));
        assert_eq!(lrc("00:12.34"), Some(12_340));
        assert_eq!(lrc("00:12.345"), Some(12_345));
        assert_eq!(lrc("01:02:03"), Some(62_030), "mm:ss:xx");
        assert_eq!(lrc("1:02:03.5"), Some(3_723_500), "h:mm:ss.x");
        assert_eq!(lrc("123:00.00"), Some(7_380_000), "minutes past 99");
        for bad in [
            "",
            "ar:Someone",
            "00:60",
            "aa:bb",
            "-1:00",
            "00:1x",
            "1:2:3:4",
            "99999999999:00",
        ] {
            assert_eq!(lrc(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn ttml_times() {
        assert_eq!(ttml("12.3s"), Some(12_300));
        assert_eq!(ttml("1500ms"), Some(1500));
        assert_eq!(ttml("1.5m"), Some(90_000));
        assert_eq!(ttml("0.5h"), Some(1_800_000));
        assert_eq!(ttml("12.345"), Some(12_345));
        assert_eq!(ttml("1:02.345"), Some(62_345));
        assert_eq!(ttml("00:01:02.345"), Some(62_345));
        assert_eq!(ttml(".5s"), Some(500));
        for bad in ["", "12f", "abc", "1:99.0", "s", "1.2.3s", "00:00:00:00"] {
            assert_eq!(ttml(bad), None, "{bad:?}");
        }
    }
}
