//! Local time on macOS through localtime_r.

use super::{Day, LocalNow};
use std::os::raw::{c_char, c_int, c_long};

// The full struct tm: glibc-style trailing fields included, or localtime_r writes past our buffer.
#[repr(C)]
struct Tm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
    tm_gmtoff: c_long,
    tm_zone: *const c_char,
}

extern "C" {
    fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
}

pub fn now() -> LocalNow {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 1,
        tm_mon: 0,
        tm_year: 70,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    // SAFETY: both pointers are valid for the call, and Tm matches the C struct tm layout.
    let ok = unsafe { !localtime_r(&secs, &mut tm).is_null() };
    if !ok {
        return LocalNow {
            date: Day::from_epoch_days(secs.div_euclid(86_400)),
            hour: (secs.rem_euclid(86_400) / 3_600) as u8,
        };
    }
    let date = Day::from_ymd(1900 + tm.tm_year, (tm.tm_mon + 1) as u32, tm.tm_mday as u32)
        .unwrap_or_else(|| Day::from_epoch_days(secs.div_euclid(86_400)));
    LocalNow {
        date,
        hour: tm.tm_hour.clamp(0, 23) as u8,
    }
}
