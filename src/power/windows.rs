//! Power throttling opt-out, battery state and process CPU time through Win32.

use super::Supply;
use std::time::Duration;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, ProcessPowerThrottling, SetProcessInformation,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE,
};

pub fn stay_fast() -> Result<(), String> {
    // Control the speed bit and leave it clear: Windows must not throttle this process.
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    // SAFETY: the pseudo handle needs no closing; the pointer and size describe `state`, which outlives the call.
    let ok = unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            std::ptr::from_ref(&state).cast(),
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

pub fn supply() -> Supply {
    // SAFETY: plain integers, so zero is valid; the call only fills the struct.
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: status is a live SYSTEM_POWER_STATUS for the call to fill.
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return Supply::default();
    }
    from_status(
        status.ACLineStatus,
        status.BatteryLifePercent,
        status.SystemStatusFlag,
    )
}

// ACLineStatus 0 = battery, 1 = mains, 255 = unknown; percent 255 = unknown; flag 1 = battery saver on.
fn from_status(ac_line: u8, percent: u8, flag: u8) -> Supply {
    Supply {
        on_battery: match ac_line {
            0 => Some(true),
            1 => Some(false),
            _ => None,
        },
        percent: (percent <= 100).then_some(percent),
        saver: Some(flag == 1),
    }
}

pub fn cpu_time() -> Option<Duration> {
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: the pseudo handle needs no closing; all four FILETIMEs are live for the call to fill.
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    if ok == 0 {
        return None;
    }
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    // FILETIME counts 100-nanosecond ticks.
    Some(Duration::from_nanos((ticks(kernel) + ticks(user)) * 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_status_codes_map_to_words() {
        assert_eq!(
            from_status(0, 54, 1).describe(),
            "battery 54%, power saving on"
        );
        assert_eq!(from_status(1, 255, 0).describe(), "AC power");
        assert_eq!(from_status(255, 255, 0).describe(), "-");
    }
}
