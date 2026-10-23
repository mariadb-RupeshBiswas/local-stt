//! Call detection: notices when a call app starts using the microphone and offers to take a note.

#[cfg(target_os = "macos")]
mod macos;
pub mod toast;
#[cfg(windows)]
mod windows;

use std::collections::BTreeMap;

/// Seconds between looks at who holds the microphone.
pub const EVERY_SECS: u64 = 3;
/// Looks a call may be missing before it counts as over, so a short mic drop does not offer again.
const GONE_AFTER_LOOKS: u32 = 5;

// Call apps by bundle id (Mac) or executable or package name (Windows), lowercase.
const APPS: [(&str, &str); 16] = [
    ("us.zoom.xos", "Zoom"),
    ("zoom.exe", "Zoom"),
    ("cpthost.exe", "Zoom"),
    ("com.microsoft.teams", "Microsoft Teams"),
    ("com.microsoft.teams2", "Microsoft Teams"),
    ("ms-teams.exe", "Microsoft Teams"),
    ("teams.exe", "Microsoft Teams"),
    ("msteams", "Microsoft Teams"),
    ("com.tinyspeck.slackmacgap", "Slack"),
    ("slack.exe", "Slack"),
    ("com.cisco.webexmeetingsapp", "Webex"),
    ("cisco-systems.spark", "Webex"),
    ("webex.exe", "Webex"),
    ("ciscocollabhost.exe", "Webex"),
    ("com.apple.facetime", "FaceTime"),
    ("com.apple.avconferenced", "FaceTime"),
];

// Browsers, whose open pages tell which call is running.
const BROWSERS: [&str; 12] = [
    "com.google.chrome",
    "com.microsoft.edgemac",
    "com.brave.browser",
    "company.thebrowser.browser",
    "com.apple.safari",
    "com.apple.webkit",
    "org.mozilla.firefox",
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "firefox.exe",
    "arc.exe",
];

/// What a microphone user is: a call app, a browser, or neither.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Holder {
    Call(&'static str),
    Browser,
}

/// Names a microphone user from its bundle id, executable name or package name.
pub fn holder(id: &str) -> Option<Holder> {
    let id = id.trim().to_lowercase();
    // Helpers carry the app's id plus a suffix ("com.microsoft.teams2.modulehost", "msteams_8wekyb3d8bbwe").
    let same_app = |app: &str| {
        id == app || id.starts_with(&format!("{app}.")) || id.starts_with(&format!("{app}_"))
    };
    for (app, call) in APPS {
        if same_app(app) {
            return Some(Holder::Call(call));
        }
    }
    if BROWSERS.iter().any(|b| same_app(b)) {
        return Some(Holder::Browser);
    }
    None
}

/// The call a browser page title shows: Google Meet ("Meet - abc-defg-hij") or Teams on the web.
pub fn page_call(title: &str) -> Option<&'static str> {
    let title = title.trim();
    if title.starts_with("Meet - ") || title.starts_with("Meet \u{2013} ") {
        return Some("Google Meet");
    }
    if title.contains("Microsoft Teams") {
        return Some("Microsoft Teams");
    }
    None
}

/// Apps recording from a microphone now: bundle ids on a Mac, executable or package names on Windows.
pub fn mic_users() -> Vec<String> {
    #[cfg(target_os = "macos")]
    return macos::mic_users();
    #[cfg(windows)]
    return windows::mic_users();
    #[cfg(not(any(target_os = "macos", windows)))]
    Vec::new()
}

/// Whether Windows privacy settings stop this app from hearing the microphone; always false elsewhere.
pub fn mic_blocked() -> bool {
    #[cfg(windows)]
    return windows::mic_blocked();
    #[cfg(not(windows))]
    false
}

// A missing value means the switch was never touched, which Windows treats as allowed.
#[cfg_attr(not(windows), allow(dead_code))]
fn any_deny(values: &[Option<String>]) -> bool {
    values.iter().any(|v| v.as_deref() == Some("Deny"))
}

fn browser_titles() -> Vec<String> {
    #[cfg(target_os = "macos")]
    return macos::browser_titles();
    #[cfg(windows)]
    return windows::browser_titles();
    #[cfg(not(any(target_os = "macos", windows)))]
    Vec::new()
}

/// The calls using the microphone now, from the microphone's users and, for browsers, their page titles.
pub fn live_calls() -> Vec<&'static str> {
    calls_from(&mic_users(), browser_titles)
}

// Pure part of live_calls: page titles are only read when a browser holds the microphone.
fn calls_from(ids: &[String], titles: impl FnOnce() -> Vec<String>) -> Vec<&'static str> {
    let mut calls: Vec<&'static str> = Vec::new();
    let mut browser = false;
    for id in ids {
        match holder(id) {
            Some(Holder::Call(call)) if !calls.contains(&call) => calls.push(call),
            Some(Holder::Browser) => browser = true,
            _ => {}
        }
    }
    if browser {
        for title in titles() {
            if let Some(call) = page_call(&title) {
                if !calls.contains(&call) {
                    calls.push(call);
                }
            }
        }
    }
    calls
}

/// Offers a note once per call: a call seen again after a short drop is the same call.
#[derive(Default)]
pub struct Detector {
    // Each call seen, with how many looks in a row it has been missing.
    seen: BTreeMap<&'static str, u32>,
}

impl Detector {
    /// Takes the calls live now; returns the call to offer a note for, if a new one started.
    /// While `busy` (a note is recording) new calls are marked seen and never offered.
    pub fn update(&mut self, live: &[&'static str], busy: bool) -> Option<&'static str> {
        for (call, missing) in self.seen.iter_mut() {
            if live.contains(call) {
                *missing = 0;
            } else {
                *missing += 1;
            }
        }
        self.seen.retain(|_, missing| *missing <= GONE_AFTER_LOOKS);
        let mut offer = None;
        for call in live {
            if self.seen.contains_key(call) {
                continue;
            }
            self.seen.insert(call, 0);
            if !busy && offer.is_none() {
                offer = Some(*call);
            }
        }
        offer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_deny_blocks_and_missing_allows() {
        assert!(!any_deny(&[None, None]));
        assert!(!any_deny(&[Some("Allow".into()), None]));
        assert!(any_deny(&[Some("Allow".into()), Some("Deny".into())]));
    }

    #[test]
    fn call_apps_and_their_helpers_are_named() {
        assert_eq!(holder("us.zoom.xos"), Some(Holder::Call("Zoom")));
        assert_eq!(holder("Zoom.exe"), Some(Holder::Call("Zoom")));
        assert_eq!(
            holder("com.microsoft.teams2.modulehost"),
            Some(Holder::Call("Microsoft Teams"))
        );
        assert_eq!(
            holder("MSTeams_8wekyb3d8bbwe"),
            Some(Holder::Call("Microsoft Teams"))
        );
        assert_eq!(holder("com.google.Chrome.helper"), Some(Holder::Browser));
        assert_eq!(holder("chrome.exe"), Some(Holder::Browser));
        assert_eq!(holder("com.apple.Music"), None);
        assert_eq!(
            holder("us.zoom.xosfake"),
            None,
            "a prefix alone is not the app"
        );
    }

    #[test]
    fn browser_pages_name_meet_and_teams() {
        assert_eq!(page_call("Meet - abc-defg-hij"), Some("Google Meet"));
        assert_eq!(page_call("Meet \u{2013} Weekly sync"), Some("Google Meet"));
        assert_eq!(page_call("Chat | Microsoft Teams"), Some("Microsoft Teams"));
        assert_eq!(page_call("Meeting notes - Google Docs"), None);
    }

    #[test]
    fn page_titles_count_only_when_a_browser_holds_the_microphone() {
        let ids = vec!["us.zoom.xos".to_string()];
        let read = || vec!["Meet - abc-defg-hij".to_string()];
        assert_eq!(calls_from(&ids, read), vec!["Zoom"]);
        let ids = vec!["com.google.Chrome.helper".to_string()];
        assert_eq!(calls_from(&ids, read), vec!["Google Meet"]);
        assert!(
            calls_from(&ids, Vec::new).is_empty(),
            "a browser on the mic without a call page is not a call"
        );
    }

    #[test]
    fn a_call_is_offered_once_and_again_only_after_it_ends() {
        let mut d = Detector::default();
        assert_eq!(d.update(&["Zoom"], false), Some("Zoom"));
        assert_eq!(d.update(&["Zoom"], false), None);
        // A short drop is the same call.
        assert_eq!(d.update(&[], false), None);
        assert_eq!(d.update(&["Zoom"], false), None);
        for _ in 0..=GONE_AFTER_LOOKS {
            d.update(&[], false);
        }
        assert_eq!(
            d.update(&["Zoom"], false),
            Some("Zoom"),
            "a new call after the last one ended"
        );
    }

    #[test]
    fn a_call_that_starts_during_a_note_is_not_offered() {
        let mut d = Detector::default();
        assert_eq!(d.update(&["Zoom"], true), None);
        assert_eq!(d.update(&["Zoom"], false), None);
        assert_eq!(d.update(&["Zoom", "Slack"], false), Some("Slack"));
    }
}
