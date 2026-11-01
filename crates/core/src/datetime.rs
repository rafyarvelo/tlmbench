//! Text ↔ nanosecond times: ISO 8601 / RFC 3339 dates and times, and decimal seconds parsed exactly.

use crate::Timestamp;

const NS_PER_S: i64 = 1_000_000_000;

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Formats nanoseconds since the epoch as RFC 3339 in UTC, with as many fraction digits as needed (0, 3, 6 or 9).
pub fn format_utc(t: Timestamp) -> String {
    let (secs, ns) = (t.div_euclid(NS_PER_S), t.rem_euclid(NS_PER_S));
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (sod / 3600, sod / 60 % 60, sod % 60);
    let frac = match ns {
        0 => String::new(),
        _ if ns % 1_000_000 == 0 => format!(".{:03}", ns / 1_000_000),
        _ if ns % 1_000 == 0 => format!(".{:06}", ns / 1_000),
        _ => format!(".{ns:09}"),
    };
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}{frac}Z")
}

/// Times before 2000 are taken as relative to the start of the recording (seconds from 0), not dates.
pub const RELATIVE_BEFORE: Timestamp = 946_684_800 * NS_PER_S;

/// A time for display: RFC 3339 for dates, `+12.5 s` / `+01:02:03` for relative times.
pub fn format_time(t: Timestamp) -> String {
    if t.abs() < RELATIVE_BEFORE { format!("+{}", format_duration(t)) } else { format_utc(t) }
}

/// Formats a duration in nanoseconds as `1d 02:03:04.5`, `02:03:04` or `3.25 s`.
pub fn format_duration(ns: i64) -> String {
    let s = ns as f64 / 1e9;
    if s.abs() < 60.0 {
        return format!("{} s", trim_float(s, 6));
    }
    let total = ns / NS_PER_S;
    let (d, h, m, sec) = (total / 86_400, total / 3600 % 24, total / 60 % 60, total % 60);
    // milliseconds, truncated (rounding could carry into the seconds)
    let ms = ns % NS_PER_S / 1_000_000;
    let frac = if ms == 0 { String::new() } else { format!(".{ms:03}").trim_end_matches('0').to_owned() };
    let day = if d > 0 { format!("{d}d ") } else { String::new() };
    format!("{day}{h:02}:{m:02}:{sec:02}{frac}")
}

fn trim_float(v: f64, digits: usize) -> String {
    let s = format!("{v:.digits$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s }
}

/// Parses an ISO 8601 date or date-time: `2023-01-03`, `2023-01-03T10:20:30`, `2023-01-03 10:20:30.123456789`,
/// with an optional `Z` or `±hh:mm` offset. No offset means UTC.
pub fn parse_iso(s: &str) -> Option<Timestamp> {
    let b = s.trim().as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = b.get(r)?;
        part.iter().all(u8::is_ascii_digit).then(|| part.iter().fold(0u32, |a, c| a * 10 + (c - b'0') as u32))
    };
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(0..4)? as i64, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let mut t = days_from_civil(y, mo, d) * 86_400 * NS_PER_S;
    let mut i = 10;
    if i < b.len() && (b[i] == b'T' || b[i] == b' ') {
        if b.len() < i + 9 || b[i + 3] != b':' || b[i + 6] != b':' {
            return None;
        }
        let (hh, mm, ss) = (num(i + 1..i + 3)?, num(i + 4..i + 6)?, num(i + 7..i + 9)?);
        if hh > 23 || mm > 59 || ss > 60 {
            return None;
        }
        t += (hh as i64 * 3600 + mm as i64 * 60 + ss as i64) * NS_PER_S;
        i += 9;
        if i < b.len() && (b[i] == b'.' || b[i] == b',') {
            let start = i + 1;
            i = start;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                return None;
            }
            t += fraction_ns(&b[start..i]);
        }
    }
    match b.get(i) {
        None => Some(t),
        Some(b'Z' | b'z') if i + 1 == b.len() => Some(t),
        Some(&sign @ (b'+' | b'-')) if b.len() == i + 6 && b[i + 3] == b':' => {
            let off = (num(i + 1..i + 3)? as i64 * 3600 + num(i + 4..i + 6)? as i64 * 60) * NS_PER_S;
            Some(if sign == b'+' { t - off } else { t + off })
        }
        _ => None,
    }
}

/// Up to 9 fraction digits as nanoseconds (more digits are truncated).
fn fraction_ns(digits: &[u8]) -> i64 {
    let mut ns = 0i64;
    for k in 0..9 {
        ns = ns * 10 + digits.get(k).map_or(0, |c| (c - b'0') as i64);
    }
    ns
}

/// Parses decimal seconds (`12`, `-0.25`, `1793491200.0001`) to nanoseconds exactly, with no float rounding.
/// `scale_ns` is the unit in nanoseconds (1 for ns, 1_000 for µs, 1e9 for s). Exponent notation falls back to
/// `f64`. Returns None for anything else.
pub fn parse_decimal(s: &str, scale_ns: i64) -> Option<Timestamp> {
    let t = s.trim().as_bytes();
    let (neg, body) = match t.first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let dot = body.iter().position(|&c| c == b'.').unwrap_or(body.len());
    let (int, frac) = (&body[..dot], body.get(dot + 1..).unwrap_or(&[]));
    let digits = |p: &[u8]| p.iter().all(u8::is_ascii_digit);
    if (int.is_empty() && frac.is_empty()) || !digits(int) || !digits(frac) {
        let v: f64 = s.trim().parse().ok()?;
        let ns = v * scale_ns as f64;
        return (ns.is_finite() && ns.abs() < i64::MAX as f64).then(|| ns.round() as i64);
    }
    let whole = int.iter().try_fold(0i64, |a, &c| a.checked_mul(10)?.checked_add((c - b'0') as i64))?;
    let mut ns = whole.checked_mul(scale_ns)?;
    // the fraction, scaled to the unit: take as many digits as the unit has below one
    let mut unit = scale_ns;
    for &c in frac {
        if unit < 10 {
            break;
        }
        unit /= 10;
        ns = ns.checked_add((c - b'0') as i64 * unit)?;
    }
    Some(if neg { -ns } else { ns })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trip() {
        let t = parse_iso("2026-11-01T12:34:56.789Z").unwrap();
        assert_eq!(format_utc(t), "2026-11-01T12:34:56.789Z");
        assert_eq!(parse_iso("1970-01-01"), Some(0));
        assert_eq!(parse_iso("2023-01-03"), Some(1_672_704_000 * NS_PER_S));
        assert_eq!(parse_iso("2026-11-01 01:00:00+01:00"), parse_iso("2026-11-01T00:00:00Z"));
        assert_eq!(parse_iso("1969-12-31T23:59:59.5Z"), Some(-NS_PER_S / 2));
        for bad in ["", "2023", "2023-13-01", "2023-01-01T25:00:00", "2023-01-01T10:00", "N/A", "2023-01-01x"] {
            assert_eq!(parse_iso(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn decimal_seconds_are_exact() {
        assert_eq!(parse_decimal("1793491200.0001", NS_PER_S), Some(1_793_491_200_000_100_000));
        assert_eq!(parse_decimal("-0.25", NS_PER_S), Some(-250_000_000));
        assert_eq!(parse_decimal("12", 1_000), Some(12_000));
        assert_eq!(parse_decimal(".5", NS_PER_S), Some(500_000_000));
        assert_eq!(parse_decimal("1.5e3", NS_PER_S), Some(1_500 * NS_PER_S));
        assert_eq!(parse_decimal("0.1234567891", NS_PER_S), Some(123_456_789));
        assert_eq!(parse_decimal("abc", NS_PER_S), None);
        assert_eq!(parse_decimal("", NS_PER_S), None);
    }

    #[test]
    fn relative_times() {
        assert_eq!(format_time(19_990_000_000), "+19.99 s");
        assert_eq!(format_time(1_767_225_600 * NS_PER_S), "2026-01-01T00:00:00Z");
    }

    #[test]
    fn durations() {
        assert_eq!(format_duration(3_250_000_000), "3.25 s");
        assert_eq!(format_duration(10_000 * NS_PER_S), "02:46:40");
        assert_eq!(format_duration((90_061 * NS_PER_S) + 500_000_000), "1d 01:01:01.5");
        assert_eq!(format_duration(99_999_900_000), "00:01:39.999");
    }
}
