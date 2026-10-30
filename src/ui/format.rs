//! Size, age and count formatting.

const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

/// JS `toFixed`: Rust rounds exact ties to even ("1.25" -> "1.2"), JS rounds up.
fn to_fixed(value: f64, digits: usize) -> String {
    // A value in [1, 1000) has at most 52 fractional digits, so 60 is exact.
    let exact = format!("{value:.60}");
    let tail = exact
        .split_once('.')
        .map_or("", |(_, frac)| &frac[digits..]);
    let tie = tail.starts_with('5') && tail[1..].bytes().all(|b| b == b'0');
    let value = if tie { value.next_up() } else { value };
    format!("{value:.digits$}")
}

/// Decimal (1000-based) sizes, matching Finder: "52.5 GB", "120 MB", "0 B".
pub fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let digits = if unit == 0 || value >= 100.0 { 0 } else { 1 };
    format!("{} {}", to_fixed(value, digits), UNITS[unit])
}

/// Estimates (command rules) read "up to 4.1 GB".
pub fn format_size(bytes: u64, estimate: bool) -> String {
    if estimate {
        format!("up to {}", format_bytes(bytes))
    } else {
        format_bytes(bytes)
    }
}

/// How long ago `then` was, both in Unix seconds: "just now", "5 min ago",
/// "2 h ago", "3 days ago". A time in the future reads "just now".
pub fn format_age(now: i64, then: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let age = now.saturating_sub(then);
    if age < MINUTE {
        "just now".into()
    } else if age < HOUR {
        format!("{} min ago", age / MINUTE)
    } else if age < DAY {
        format!("{} h ago", age / HOUR)
    } else if age < 2 * DAY {
        "1 day ago".into()
    } else {
        format!("{} days ago", age / DAY)
    }
}

/// `Number.toLocaleString()` for en-US: `1,234,567`.
pub fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1000), "1,000");
        assert_eq!(group_digits(1_234_567), "1,234,567");
    }

    #[test]
    fn format_bytes_across_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(52_500_000_000), "52.5 GB");
        assert_eq!(format_bytes(288_000_000_000), "288 GB");
    }

    #[test]
    fn format_bytes_rounds_ties_up_like_js() {
        assert_eq!(format_bytes(1_250), "1.3 KB");
        assert_eq!(format_bytes(100_500), "101 KB");
        // 1.15 is below the tie in binary, so JS gives 1.1 too.
        assert_eq!(format_bytes(1_150), "1.1 KB");
    }

    #[test]
    fn age_under_a_minute_is_just_now() {
        assert_eq!(format_age(1_000, 941), "just now");
    }

    #[test]
    fn age_in_the_future_is_just_now() {
        assert_eq!(format_age(1_000, 5_000), "just now");
    }

    #[test]
    fn age_in_minutes() {
        assert_eq!(format_age(10_000, 10_000 - 5 * 60 - 30), "5 min ago");
    }

    #[test]
    fn age_in_hours() {
        assert_eq!(format_age(100_000, 100_000 - 2 * 3_600 - 59), "2 h ago");
    }

    #[test]
    fn age_of_one_day_is_singular() {
        assert_eq!(format_age(200_000, 200_000 - 86_400), "1 day ago");
    }

    #[test]
    fn age_in_days() {
        assert_eq!(format_age(1_000_000, 1_000_000 - 3 * 86_400), "3 days ago");
    }

    #[test]
    fn format_size_prefixes_estimates() {
        assert_eq!(format_size(4_100_000_000, true), "up to 4.1 GB");
        assert_eq!(format_size(4_100_000_000, false), "4.1 GB");
    }
}
