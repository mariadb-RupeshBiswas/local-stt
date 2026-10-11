//! Local time on Windows through GetLocalTime.

use super::{Day, LocalNow};
use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

pub fn now() -> LocalNow {
    // SAFETY: SYSTEMTIME is plain integers (zero is valid) and GetLocalTime only fills it.
    let st: SYSTEMTIME = unsafe {
        let mut st = std::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    let date = Day::from_ymd(st.wYear as i32, st.wMonth as u32, st.wDay as u32)
        .unwrap_or_else(|| Day::from_epoch_days(0));
    LocalNow {
        date,
        hour: st.wHour.min(23) as u8,
    }
}
