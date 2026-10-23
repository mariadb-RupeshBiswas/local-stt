//! Power and CPU use: keeps Windows from throttling the app, and reports what slows transcription.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::time::Duration;

/// How the computer is powered; a field that could not be read stays None.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Supply {
    pub on_battery: Option<bool>,
    pub percent: Option<u8>,
    pub saver: Option<bool>,
}

impl Supply {
    /// "AC power", "battery 54%, power saving on", or "-".
    pub fn describe(&self) -> String {
        let mut text = match self.on_battery {
            Some(false) => "AC power".to_string(),
            Some(true) => match self.percent {
                Some(p) => format!("battery {p}%"),
                None => "battery".to_string(),
            },
            None => return "-".into(),
        };
        if self.saver == Some(true) {
            text.push_str(", power saving on");
        }
        text
    }
}

/// Opts the process out of Windows power throttling (EcoQoS), which parks a background app on slow cores.
pub fn stay_fast() -> Result<(), String> {
    #[cfg(windows)]
    return windows::stay_fast();
    #[cfg(not(windows))]
    return Ok(());
}

pub fn supply() -> Supply {
    #[cfg(windows)]
    return windows::supply();
    #[cfg(target_os = "macos")]
    return macos::supply();
    #[cfg(not(any(windows, target_os = "macos")))]
    return Supply::default();
}

/// CPU time this process has used so far, across all its threads.
pub fn cpu_time() -> Option<Duration> {
    #[cfg(windows)]
    return windows::cpu_time();
    #[cfg(target_os = "macos")]
    return macos::cpu_time();
    #[cfg(not(any(windows, target_os = "macos")))]
    return None;
}

/// Average cores kept busy over a stretch: 8 threads on a starved CPU show as well under 8.
pub fn cores_busy(cpu: Duration, wall: Duration) -> Option<f64> {
    if wall.is_zero() {
        return None;
    }
    Some(cpu.as_secs_f64() / wall.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stay_fast_succeeds() {
        assert_eq!(stay_fast(), Ok(()));
    }

    #[test]
    fn supply_reads_as_words() {
        let ac = Supply {
            on_battery: Some(false),
            percent: Some(80),
            saver: Some(false),
        };
        assert_eq!(ac.describe(), "AC power");
        let low = Supply {
            on_battery: Some(true),
            percent: Some(12),
            saver: Some(true),
        };
        assert_eq!(low.describe(), "battery 12%, power saving on");
        assert_eq!(Supply::default().describe(), "-");
    }

    #[test]
    fn cores_busy_divides_cpu_by_wall() {
        let busy = cores_busy(Duration::from_secs(6), Duration::from_secs(4)).unwrap();
        assert!((busy - 1.5).abs() < 1e-9);
        assert_eq!(cores_busy(Duration::from_secs(1), Duration::ZERO), None);
    }

    #[test]
    fn cpu_time_grows_while_working() {
        let before = cpu_time().expect("cpu time readable");
        let mut x = 0u64;
        for i in 0..20_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        std::hint::black_box(x);
        assert!(cpu_time().unwrap() > before);
    }
}
