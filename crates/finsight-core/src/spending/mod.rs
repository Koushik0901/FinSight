//! Spending Analysis Engine — deterministic "what changed vs your normal".
pub mod stats;

/// A half-open date window `[start, end)` in `YYYY-MM-DD`, plus the number of
/// whole calendar months it spans (used to convert a window total into a
/// monthly-equivalent so a 1-month window and a 12-month baseline compare).
#[derive(Debug, Clone)]
pub struct Window {
    pub start: String,
    pub end: String,
    pub months: f64,
}

impl Window {
    /// The single calendar month `ym` (`YYYY-MM`) as a `[first, next-first)` window.
    pub fn for_month(ym: &str) -> Window {
        let (y, m) = parse_ym(ym);
        let start = format!("{y:04}-{m:02}-01");
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        Window { start, end: format!("{ny:04}-{nm:02}-01"), months: 1.0 }
    }
}

/// Parse `YYYY-MM` into `(year, month)`. Defaults to `(1970, 1)` on garbage so
/// callers never panic on user/LLM input.
pub fn parse_ym(ym: &str) -> (i32, u32) {
    let mut it = ym.split('-');
    let y = it.next().and_then(|s| s.parse().ok()).unwrap_or(1970);
    let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m.clamp(1, 12))
}

/// Count of whole calendar months in `[start_ym, end_ym)` (both `YYYY-MM`).
pub fn months_between(start_ym: &str, end_ym: &str) -> i64 {
    let (sy, sm) = parse_ym(start_ym);
    let (ey, em) = parse_ym(end_ym);
    ((ey * 12 + em as i32) - (sy * 12 + sm as i32)) as i64
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn for_month_builds_a_half_open_window() {
        let w = Window::for_month("2026-05");
        assert_eq!(w.start, "2026-05-01");
        assert_eq!(w.end, "2026-06-01");
        assert_eq!(w.months, 1.0);
        let w = Window::for_month("2026-12");
        assert_eq!(w.end, "2027-01-01");
    }

    #[test]
    fn months_between_counts_calendar_months() {
        assert_eq!(months_between("2025-04", "2026-04"), 12);
        assert_eq!(months_between("2026-05", "2026-06"), 1);
    }
}
