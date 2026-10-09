//! Recording pill placement: multi-monitor default, saved position, edge snapping, spring motion.

use crate::config::OverlayPos;

pub const MARGIN: f64 = 24.0;
pub const SNAP: f64 = 16.0;
pub const SPRING_RESPONSE_S: f64 = 0.4;

/// A monitor's usable work area in desktop units (points on macOS, pixels on Windows).
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Screen {
    fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }

    fn bottom_right(&self, size: (f64, f64)) -> (f64, f64) {
        (
            self.x + self.w - MARGIN - size.0,
            self.y + self.h - MARGIN - size.1,
        )
    }
}

pub fn screen_for_point(screens: &[Screen], p: (f64, f64)) -> Option<&Screen> {
    screens.iter().find(|s| s.contains(p)).or(screens.first())
}

pub fn default_pos(screens: &[Screen], pointer: (f64, f64), size: (f64, f64)) -> (f64, f64) {
    match screen_for_point(screens, pointer) {
        Some(s) => s.bottom_right(size),
        None => (MARGIN, MARGIN),
    }
}

pub fn resolve(
    screens: &[Screen],
    saved: Option<&OverlayPos>,
    pointer: (f64, f64),
    size: (f64, f64),
) -> (f64, f64) {
    let Some(saved) = saved else {
        return default_pos(screens, pointer, size);
    };
    match screens.iter().find(|s| s.name == saved.monitor) {
        Some(screen) => snap(screen, (saved.x, saved.y), size),
        None => default_pos(screens, pointer, size),
    }
}

/// Pulls the pill to the margin when it is near or past an edge; free positions are kept.
pub fn snap(screen: &Screen, pos: (f64, f64), size: (f64, f64)) -> (f64, f64) {
    let x = snap_axis(pos.0, size.0, screen.x, screen.w);
    let y = snap_axis(pos.1, size.1, screen.y, screen.h);
    (x, y)
}

fn snap_axis(pos: f64, len: f64, start: f64, extent: f64) -> f64 {
    let low = start + MARGIN;
    let high = start + extent - MARGIN - len;
    if high <= low {
        return low;
    }
    let from_start = pos - start;
    let from_end = start + extent - (pos + len);
    if from_start <= SNAP {
        return low;
    }
    if from_end <= SNAP {
        return high;
    }
    pos
}

/// Critically damped spring (no overshoot), starting at rest, after `t` seconds.
pub fn spring(from: f64, to: f64, t: f64) -> f64 {
    let omega = 2.0 * std::f64::consts::PI / SPRING_RESPONSE_S;
    let decay = (1.0 + omega * t) * (-omega * t).exp();
    to + (from - to) * decay
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, x: f64, y: f64, w: f64, h: f64) -> Screen {
        Screen { name: name.into(), x, y, w, h }
    }

    #[test]
    fn default_is_bottom_right_of_pointer_screen() {
        let screens = [mon("A", 0.0, 0.0, 1440.0, 900.0), mon("B", 1440.0, 0.0, 1920.0, 1080.0)];
        assert_eq!(
            default_pos(&screens, (2000.0, 500.0), (220.0, 44.0)),
            (1440.0 + 1920.0 - 24.0 - 220.0, 1080.0 - 24.0 - 44.0)
        );
    }

    #[test]
    fn pointer_off_every_screen_uses_first() {
        let screens = [mon("A", 0.0, 0.0, 1440.0, 900.0)];
        assert_eq!(default_pos(&screens, (-500.0, -500.0), (220.0, 44.0)), (1196.0, 832.0));
    }

    #[test]
    fn no_screens_does_not_panic() {
        assert_eq!(default_pos(&[], (0.0, 0.0), (220.0, 44.0)), (MARGIN, MARGIN));
    }

    #[test]
    fn position_on_missing_monitor_falls_back() {
        let screens = [mon("A", 0.0, 0.0, 1440.0, 900.0)];
        let saved = OverlayPos { monitor: "Gone".into(), x: 3000.0, y: 100.0 };
        assert_eq!(
            resolve(&screens, Some(&saved), (10.0, 10.0), (220.0, 44.0)),
            (1440.0 - 244.0, 900.0 - 68.0)
        );
    }

    #[test]
    fn saved_position_on_second_monitor_is_kept() {
        let screens = [mon("A", 0.0, 0.0, 1440.0, 900.0), mon("B", 1440.0, -200.0, 1920.0, 1080.0)];
        let saved = OverlayPos { monitor: "B".into(), x: 2000.0, y: 300.0 };
        assert_eq!(resolve(&screens, Some(&saved), (0.0, 0.0), (220.0, 44.0)), (2000.0, 300.0));
    }

    #[test]
    fn saved_position_outside_shrunken_monitor_is_pulled_back() {
        let screens = [mon("A", 0.0, 0.0, 1280.0, 800.0)];
        let saved = OverlayPos { monitor: "A".into(), x: 1700.0, y: 1000.0 };
        assert_eq!(resolve(&screens, Some(&saved), (0.0, 0.0), (220.0, 44.0)), (1036.0, 732.0));
    }

    #[test]
    fn snaps_near_corner() {
        let s = mon("A", 0.0, 0.0, 1440.0, 900.0);
        assert_eq!(
            snap(&s, (1440.0 - 220.0 - 10.0, 900.0 - 44.0 - 5.0), (220.0, 44.0)),
            (1440.0 - 244.0, 900.0 - 68.0)
        );
    }

    #[test]
    fn free_position_kept_away_from_edges() {
        let s = mon("A", 0.0, 0.0, 1440.0, 900.0);
        assert_eq!(snap(&s, (600.0, 400.0), (220.0, 44.0)), (600.0, 400.0));
    }

    #[test]
    fn clamped_inside_work_area() {
        let s = mon("A", 0.0, 25.0, 1440.0, 875.0);
        assert_eq!(snap(&s, (-50.0, 0.0), (220.0, 44.0)), (24.0, 49.0));
    }

    #[test]
    fn spring_settles_without_overshoot() {
        let mut last = 0.0;
        for i in 0..=60 {
            let v = spring(0.0, 100.0, i as f64 / 60.0);
            assert!(v >= last - 1e-9 && v <= 100.0 + 1e-9, "overshoot or reversal at frame {i}: {v}");
            last = v;
        }
        assert!((spring(0.0, 100.0, SPRING_RESPONSE_S) - 100.0).abs() < 2.0);
    }
}
