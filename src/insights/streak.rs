//! Workday streak rules: pure functions over a small state, no clock and no files.

use crate::localtime::Day;
use serde::{Deserialize, Serialize};

pub const START_FREEZES: u32 = 5;
pub const FREEZE_CAP: u32 = 10;
pub const WORKDAYS_PER_FREEZE: u32 = 10;
// Frozen dates only matter for the week row, so older ones are dropped.
const KEEP_FROZEN_DAYS: i64 = 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Streak {
    pub current: u32,
    pub longest: u32,
    pub freezes: u32,
    /// Lifetime active workdays; survives a reset so freezes keep being earned.
    pub active_workdays: u32,
    /// Last day the rules ran over; days up to and including it are settled.
    pub last_evaluated: Option<Day>,
    /// Day of the first dictation; earlier days are never shown as missed.
    pub first_day: Option<Day>,
    pub frozen: Vec<Day>,
}

impl Default for Streak {
    fn default() -> Self {
        Streak {
            current: 0,
            longest: 0,
            freezes: START_FREEZES,
            active_workdays: 0,
            last_evaluated: None,
            first_day: None,
            frozen: Vec::new(),
        }
    }
}

/// Settles every workday after `last_evaluated` up to yesterday; today is never missed.
pub fn advance(state: &mut Streak, active: &dyn Fn(Day) -> bool, today: Day) {
    let yesterday = today.prev();
    let Some(last) = state.last_evaluated else {
        // First use: nothing before today can be missed.
        state.last_evaluated = Some(yesterday);
        return;
    };
    if last >= yesterday {
        return;
    }
    let mut run: Vec<Day> = Vec::new();
    let mut day = last.next();
    while day <= yesterday {
        if day.is_workday() {
            if active(day) {
                settle_misses(state, &mut run);
                count_active_day(state);
            } else {
                run.push(day);
            }
        }
        day = day.next();
    }
    settle_misses(state, &mut run);
    state.last_evaluated = Some(yesterday);
    state
        .frozen
        .retain(|d| yesterday.days_since(*d) <= KEEP_FROZEN_DAYS);
}

fn count_active_day(state: &mut Streak) {
    state.current += 1;
    state.longest = state.longest.max(state.current);
    state.active_workdays += 1;
    if state.active_workdays % WORKDAYS_PER_FREEZE == 0 {
        state.freezes = (state.freezes + 1).min(FREEZE_CAP);
    }
}

// A run of missed workdays is covered whole or not at all, so a lost cause keeps its freezes.
fn settle_misses(state: &mut Streak, run: &mut Vec<Day>) {
    if run.is_empty() {
        return;
    }
    // With no streak running there is nothing to protect, so nothing is spent.
    if state.current > 0 {
        if state.freezes as usize >= run.len() {
            state.freezes -= run.len() as u32;
            state.frozen.append(run);
        } else {
            state.current = 0;
        }
    }
    run.clear();
}

/// The streak as the tab shows it: today's dictation counts now, not tomorrow.
pub fn with_today(state: &Streak, active_today: bool, today: Day) -> Streak {
    let mut shown = state.clone();
    let settled = state.last_evaluated.is_some_and(|l| l >= today);
    if active_today && today.is_workday() && !settled {
        count_active_day(&mut shown);
    }
    shown
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WeekState {
    Active,
    Frozen,
    Missed,
    Today,
    Upcoming,
    Rest,
    Bonus,
}

/// Monday to Sunday of the week holding `today`.
pub fn week(state: &Streak, active: &dyn Fn(Day) -> bool, today: Day) -> Vec<(Day, WeekState)> {
    let monday = today.plus_days(-i64::from(today.weekday()));
    (0..7)
        .map(|i| {
            let day = monday.plus_days(i);
            (day, week_state(state, active, today, day))
        })
        .collect()
}

fn week_state(state: &Streak, active: &dyn Fn(Day) -> bool, today: Day, day: Day) -> WeekState {
    if !day.is_workday() {
        return if active(day) {
            WeekState::Bonus
        } else {
            WeekState::Rest
        };
    }
    if day == today {
        return if active(day) {
            WeekState::Active
        } else {
            WeekState::Today
        };
    }
    if day > today {
        return WeekState::Upcoming;
    }
    if active(day) {
        WeekState::Active
    } else if state.frozen.contains(&day) {
        WeekState::Frozen
    } else if state.first_day.is_some_and(|first| day >= first) {
        WeekState::Missed
    } else {
        WeekState::Rest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // 2026-10-12 is a Monday.
    fn mon(week: i64) -> Day {
        Day::from_ymd(2026, 10, 12).unwrap().plus_days(week * 7)
    }

    fn run(state: &mut Streak, days: &[Day], today: Day) {
        let set: HashSet<Day> = days.iter().copied().collect();
        advance(state, &|d| set.contains(&d), today);
    }

    fn started(day: Day) -> Streak {
        Streak {
            last_evaluated: Some(day.prev()),
            first_day: Some(day),
            ..Streak::default()
        }
    }

    #[test]
    fn first_use_settles_nothing() {
        let mut s = Streak::default();
        run(&mut s, &[], mon(0).plus_days(2));
        assert_eq!(s.last_evaluated, Some(mon(0).plus_days(1)));
        assert_eq!((s.current, s.freezes), (0, START_FREEZES));
    }

    #[test]
    fn weekend_does_not_break_and_friday_to_monday_is_consecutive() {
        let mut s = started(mon(0));
        let days: Vec<Day> = (0..5).map(|i| mon(0).plus_days(i)).collect();
        run(&mut s, &days, mon(1));
        assert_eq!(s.current, 5);
        run(
            &mut s,
            &[days.clone(), vec![mon(1)]].concat(),
            mon(1).plus_days(1),
        );
        assert_eq!(s.current, 6);
        assert_eq!(s.freezes, START_FREEZES);
        assert!(s.frozen.is_empty());
    }

    #[test]
    fn missed_wednesday_spends_a_freeze() {
        let mut s = started(mon(0));
        let days = [mon(0), mon(0).plus_days(1), mon(0).plus_days(3)];
        run(&mut s, &days, mon(0).plus_days(4));
        assert_eq!((s.current, s.freezes), (3, START_FREEZES - 1));
        assert_eq!(s.frozen, vec![mon(0).plus_days(2)]);
    }

    #[test]
    fn gap_longer_than_freezes_resets_and_keeps_them() {
        let mut s = started(mon(0));
        s.freezes = 1;
        run(&mut s, &[mon(0), mon(0).plus_days(1)], mon(0).plus_days(2));
        assert_eq!(s.current, 2);
        // Six workdays missed; one freeze cannot cover them.
        run(
            &mut s,
            &[mon(0), mon(0).plus_days(1), mon(1).plus_days(3)],
            mon(1).plus_days(4),
        );
        assert_eq!((s.current, s.freezes), (1, 1));
        assert_eq!(s.longest, 2);
        assert!(s.frozen.is_empty());
    }

    #[test]
    fn long_gap_with_no_streak_spends_nothing() {
        let mut s = started(mon(0));
        run(&mut s, &[], mon(3));
        assert_eq!((s.current, s.freezes), (0, START_FREEZES));
    }

    #[test]
    fn tenth_active_workday_earns_a_freeze() {
        let mut s = started(mon(0));
        let days: Vec<Day> = (0..14)
            .map(|i| mon(0).plus_days(i))
            .filter(|d| d.is_workday())
            .collect();
        run(&mut s, &days[..9], mon(1).plus_days(4));
        assert_eq!((s.active_workdays, s.freezes), (9, START_FREEZES));
        run(&mut s, &days, mon(2));
        assert_eq!((s.active_workdays, s.freezes), (10, START_FREEZES + 1));
    }

    #[test]
    fn freezes_cap_at_ten() {
        let mut s = started(mon(0));
        s.freezes = FREEZE_CAP;
        s.active_workdays = 9;
        run(&mut s, &[mon(0)], mon(0).next());
        assert_eq!((s.active_workdays, s.freezes), (10, FREEZE_CAP));
    }

    #[test]
    fn today_is_never_missed() {
        let mut s = started(mon(0));
        run(&mut s, &[mon(0)], mon(0).next());
        assert_eq!(s.last_evaluated, Some(mon(0)));
        assert_eq!((s.current, s.freezes), (1, START_FREEZES));
    }

    #[test]
    fn clock_moving_back_never_reevaluates() {
        let mut s = started(mon(0));
        run(&mut s, &[mon(0), mon(0).plus_days(1)], mon(0).plus_days(2));
        let before = s.clone();
        run(&mut s, &[], mon(0).plus_days(2));
        assert_eq!(s, before);
    }

    #[test]
    fn with_today_counts_todays_dictation_once() {
        let mut s = started(mon(0));
        run(&mut s, &[mon(0)], mon(0).next());
        let shown = with_today(&s, true, mon(0).next());
        assert_eq!(shown.current, 2);
        assert_eq!(with_today(&s, false, mon(0).next()).current, 1);
        // Saturday and settled days add nothing.
        assert_eq!(with_today(&s, true, mon(0).plus_days(5)).current, 1);
        assert_eq!(with_today(&s, true, mon(0)).current, 1);
    }

    #[test]
    fn week_row_states() {
        let mut s = started(mon(0));
        s.frozen.push(mon(0).plus_days(1));
        // Mon active, Tue frozen, Wed missed, Thu today (active), Sun bonus.
        let on: HashSet<Day> = [mon(0), mon(0).plus_days(3), mon(0).plus_days(6)].into();
        let row = week(&s, &|d| on.contains(&d), mon(0).plus_days(3));
        let states: Vec<WeekState> = row.iter().map(|(_, s)| *s).collect();
        use WeekState::*;
        assert_eq!(
            states,
            [Active, Frozen, Missed, Active, Upcoming, Rest, Bonus]
        );
        assert_eq!(row[0].0, mon(0));
        let quiet = week(&s, &|_| false, mon(0).plus_days(3));
        assert_eq!(quiet[3].1, Today);
        let before_start = week(
            &started(mon(0).plus_days(3)),
            &|_| false,
            mon(0).plus_days(4),
        );
        assert_eq!(before_start[0].1, Rest);
    }
}
