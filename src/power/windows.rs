//! Power throttling opt-out through SetProcessInformation.

use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, ProcessPowerThrottling, SetProcessInformation,
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
