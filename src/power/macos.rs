//! Battery state through pmset, and process CPU time through getrusage.

use super::Supply;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);

pub fn supply() -> Supply {
    let batt =
        crate::hwprobe::run_capture(Command::new("/usr/bin/pmset").args(["-g", "batt"]), TIMEOUT);
    let settings = crate::hwprobe::run_capture(Command::new("/usr/bin/pmset").arg("-g"), TIMEOUT);
    parse(
        batt.as_deref().unwrap_or(""),
        settings.as_deref().unwrap_or(""),
    )
}

// `pmset -g batt` names the source in quotes; `pmset -g` lists "lowpowermode 1" when Low Power Mode is on.
fn parse(batt: &str, settings: &str) -> Supply {
    let on_battery = if batt.contains("'Battery Power'") {
        Some(true)
    } else if batt.contains("'AC Power'") {
        Some(false)
    } else {
        None
    };
    let percent = batt
        .split('%')
        .next()
        .filter(|_| batt.contains('%'))
        .and_then(|head| head.rsplit(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok());
    let saver = settings
        .lines()
        .find(|l| l.trim_start().starts_with("lowpowermode"))
        .map(|l| l.trim_end().ends_with('1'));
    Supply {
        on_battery,
        percent,
        saver,
    }
}

#[repr(C)]
struct Timeval {
    sec: i64,
    usec: i32,
}

#[repr(C)]
struct Rusage {
    utime: Timeval,
    stime: Timeval,
    rest: [i64; 14],
}

extern "C" {
    fn getrusage(who: i32, usage: *mut Rusage) -> i32;
}

const RUSAGE_SELF: i32 = 0;

pub fn cpu_time() -> Option<Duration> {
    let mut usage = std::mem::MaybeUninit::<Rusage>::zeroed();
    // SAFETY: usage is writable and laid out as Darwin's struct rusage; getrusage only fills it.
    let rc = unsafe { getrusage(RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: zeroed is a valid Rusage and getrusage succeeded.
    let usage = unsafe { usage.assume_init() };
    let micros = |t: &Timeval| t.sec as u64 * 1_000_000 + t.usec.max(0) as u64;
    Some(Duration::from_micros(
        micros(&usage.utime) + micros(&usage.stime),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pmset_output() {
        let batt = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1234)\t54%; discharging; 3:10 remaining present: true\n";
        let s = parse(batt, " lowpowermode         1\n");
        assert_eq!(
            s,
            Supply {
                on_battery: Some(true),
                percent: Some(54),
                saver: Some(true)
            }
        );
        let desktop = parse("Now drawing from 'AC Power'\n", " sleep 1\n");
        assert_eq!(
            desktop,
            Supply {
                on_battery: Some(false),
                percent: None,
                saver: None
            }
        );
    }

    #[test]
    fn rusage_matches_darwin_size() {
        assert_eq!(std::mem::size_of::<Rusage>(), 144);
    }
}
