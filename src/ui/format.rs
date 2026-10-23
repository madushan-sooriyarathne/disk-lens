//! Size formatting (owner: A1). Port of `legacy-web/src/lib/format.ts`.

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn format_size_prefixes_estimates() {
        assert_eq!(format_size(4_100_000_000, true), "up to 4.1 GB");
        assert_eq!(format_size(4_100_000_000, false), "4.1 GB");
    }
}
