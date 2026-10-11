//! Local calendar date and hour from the operating system, plus pure date math.

use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// A calendar date, stored as days since 1970-01-01 and written as "YYYY-MM-DD".
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Day(i64);

#[derive(Clone, Copy, Debug)]
pub struct LocalNow {
    pub date: Day,
    pub hour: u8,
}

pub fn now() -> LocalNow {
    #[cfg(target_os = "macos")]
    return macos::now();
    #[cfg(windows)]
    return windows::now();
    #[cfg(not(any(target_os = "macos", windows)))]
    return utc_now();
}

// No local-time API without libc bindings here, so other systems get UTC.
#[cfg(not(any(target_os = "macos", windows)))]
fn utc_now() -> LocalNow {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    LocalNow {
        date: Day::from_epoch_days(secs.div_euclid(86_400)),
        hour: (secs.rem_euclid(86_400) / 3_600) as u8,
    }
}

impl Day {
    pub fn from_epoch_days(days: i64) -> Day {
        Day(days)
    }

    /// None when the month or day does not exist (February 30).
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Day> {
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        let days = days_from_civil(year as i64, month as i64, day as i64);
        let back = civil_from_days(days);
        (back == (year as i64, month as i64, day as i64)).then_some(Day(days))
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        let (y, m, d) = civil_from_days(self.0);
        (y as i32, m as u32, d as u32)
    }

    /// 0 = Monday .. 6 = Sunday (1970-01-01 was a Thursday).
    pub fn weekday(self) -> u8 {
        (self.0 + 3).rem_euclid(7) as u8
    }

    pub fn is_workday(self) -> bool {
        self.weekday() < 5
    }

    pub fn plus_days(self, n: i64) -> Day {
        Day(self.0 + n)
    }

    pub fn next(self) -> Day {
        self.plus_days(1)
    }

    pub fn prev(self) -> Day {
        self.plus_days(-1)
    }

    /// Whole days from `earlier` to self (negative when self is before it).
    pub fn days_since(self, earlier: Day) -> i64 {
        self.0 - earlier.0
    }

    pub fn parse(text: &str) -> Option<Day> {
        let mut parts = text.split('-');
        let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return None;
        }
        Day::from_ymd(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
    }
}

// Howard Hinnant's days-from-civil: exact for every proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl From<Day> for String {
    fn from(day: Day) -> String {
        day.to_string()
    }
}

impl TryFrom<String> for Day {
    type Error = String;
    fn try_from(text: String) -> Result<Day, String> {
        Day::parse(&text).ok_or_else(|| format!("not a date: {text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Day {
        Day::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn epoch_and_known_weekdays() {
        assert_eq!(day(1970, 1, 1), Day(0));
        assert_eq!(day(1970, 1, 1).weekday(), 3);
        assert_eq!(day(2026, 10, 12).weekday(), 0);
        assert_eq!(day(2026, 10, 11).weekday(), 6);
        assert_eq!(day(2000, 2, 29).weekday(), 1);
    }

    #[test]
    fn next_prev_cross_month_and_year() {
        assert_eq!(day(2026, 12, 31).next(), day(2027, 1, 1));
        assert_eq!(day(2024, 2, 28).next(), day(2024, 2, 29));
        assert_eq!(day(2025, 3, 1).prev(), day(2025, 2, 28));
        assert_eq!(day(2026, 10, 12).days_since(day(2026, 10, 5)), 7);
    }

    #[test]
    fn workdays_are_monday_to_friday() {
        let monday = day(2026, 10, 12);
        let flags: Vec<bool> = (0..7).map(|i| monday.plus_days(i).is_workday()).collect();
        assert_eq!(flags, [true, true, true, true, true, false, false]);
    }

    #[test]
    fn parse_and_format_round_trip() {
        let d = day(2026, 1, 9);
        assert_eq!(d.to_string(), "2026-01-09");
        assert_eq!(Day::parse("2026-01-09"), Some(d));
        for bad in [
            "2026-02-30",
            "2026-13-01",
            "2026-1-09",
            "x",
            "",
            "2026-01-09-1",
        ] {
            assert_eq!(Day::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn every_day_over_a_century_round_trips() {
        let start = day(1990, 1, 1).0;
        for n in start..start + 40_000 {
            let (y, m, d) = Day(n).ymd();
            assert_eq!(Day::from_ymd(y, m, d), Some(Day(n)));
        }
    }

    #[test]
    fn json_is_a_date_string() {
        let d = day(2026, 10, 12);
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"2026-10-12\"");
        assert_eq!(serde_json::from_str::<Day>("\"2026-10-12\"").unwrap(), d);
        assert!(serde_json::from_str::<Day>("\"nope\"").is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn now_matches_the_date_command() {
        let out = |fmt: &str| {
            let o = std::process::Command::new("/bin/date")
                .arg(fmt)
                .output()
                .unwrap();
            String::from_utf8(o.stdout).unwrap().trim().to_string()
        };
        // Retry once so a midnight or top-of-hour tick between the two reads cannot flake.
        for attempt in 0..2 {
            let before = out("+%Y-%m-%d %H");
            let local = now();
            let after = out("+%Y-%m-%d %H");
            let got = format!("{} {:02}", local.date, local.hour);
            if got == before || got == after {
                return;
            }
            assert!(attempt == 0, "now() {got} vs date {before}");
        }
    }
}
