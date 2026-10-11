# Insights tab: design

Status: draft, 2026-10-11. Branch `feature/insights`, stacked on `feature/note-taker`.

## Goal

Help people build a dictation habit and see their progress, honestly and privately. Insights
reward showing up, never punish missing a day, and never leave the computer.

Principles, from the research ([Smashing Magazine on streak design](https://shop.smashingmagazine.com/2026/02/designing-streak-system-ux-psychology/),
self-determination theory, Duolingo's published streak work):

- Celebrate how far someone has come; never use loss language ("you will lose your streak").
- Forgiveness is built in and automatic: freezes spend themselves, the longest streak is kept.
- No notifications, no nagging, no leaderboards, nothing to buy.
- Autonomy: word insights have their own off switch, and everything can be reset.

## What the tab shows

Tabs become History, Notes, Insights, Model, Settings (Cmd/Ctrl + 1 to 5).

1. **Workday streak**: the current streak in workdays, the longest streak, freezes held
   ("4 of 10"), and this week as a Monday to Friday row: active, covered by a freeze, missed,
   today, or still to come. Saturday and Sunday show as rest days; dictating on them shows a
   small bonus mark but adds nothing and breaks nothing.
2. **Words**: today, this week, all time, and dictations so far.
3. **Speed and time saved**: average words per minute over the last 100 dictations, and an
   estimate of time saved against typing at 40 words a minute, labelled as an estimate.
4. **When you dictate**: words by hour of the day over the last 30 days.
5. **Your words** (English output only, while Word insights is on): the 15 words used most
   (common words like "the" and "and" left out) and the mix of nouns, verbs, adjectives and
   adverbs, labelled approximate.
6. **Milestones**: 1,000, 10,000, 50,000, 100,000, 250,000 and 1,000,000 words, each with the
   date it was reached.
7. Footer: "Everything here stays on this Mac." Word insights switch, Reset Insights (asks).

Empty state: "Dictate once to start your first streak."

## Streak and freeze rules

| Rule | Value |
|---|---|
| Active day | At least one successful dictation (text pasted or copied) on that local date |
| Days that count | Monday to Friday, local date at the moment of dictating |
| Weekends | Never count, never break; shown as rest days |
| Streak | Consecutive active workdays, with missed workdays covered by freezes |
| Freezes at start | 5 |
| Earning | +1 for every 10 active workdays (a lifetime count, so it survives a reset) |
| Cap | 10 held |
| Spending | Automatic: each missed workday spends one, applied when the app next records or shows insights |
| Not enough freezes | The streak ends (back to 0) and the freezes are kept, not spent on a lost cause |
| Longest streak | Kept forever |
| Today | Never counted as missed until it is over |
| Clock moves back | Past days are never evaluated again |

The streak is evaluated by one pure function, `advance(state, active_days, today)`, which walks
the workdays after the last evaluated day up to yesterday. It is unit tested for weekends,
freezes, earning, the cap, a gap longer than the freezes held, and a clock that moves back.

## Storage

`<data>/insights.json`, owner-only (0600), written with the same temp-file, fsync and rename
helper as history. It holds aggregates only:

- one row per local date (words, seconds spoken, dictations, words per hour), kept for 400
  days; older rows fold into all-time totals;
- the streak state (current, longest, freezes, lifetime active workdays, last evaluated day,
  the dates a freeze covered);
- the last 100 words-per-minute values;
- reached milestones and their dates;
- with Word insights on: counts for the 200 words used most (dropping the least used beyond
  that), and part-of-speech totals.

Never stored: sentences, word sequences, or any word's history over time. Words with digits,
words under three letters, common stop words and capitalised words in mid-sentence (often
names) are not counted. Turning Word insights off deletes the word counts and part-of-speech
totals. Reset Insights deletes the file.

Insights do not read history, so clearing or turning off history leaves the streak intact.

## Local dates

The local date, weekday and hour come from the operating system at the moment a dictation is
recorded: `localtime_r` on macOS, `GetLocalTime` on Windows, each in a platform file. No new
dependency.

## Parts of speech (v1)

No Rust crate passes the dependency rule (every English tagger found is published by a person,
or stale, or needs licensed data). v1 is our own rule-based tagger, a few hundred lines and no
data file:

- closed-class word lists written by hand (pronouns, determiners, prepositions, conjunctions,
  auxiliaries, modals);
- suffix rules (`-ly` adverb; `-tion`, `-ment`, `-ness`, `-ity` noun; `-ous`, `-ful`,
  `-able`, `-ive` adjective; `-ing`, `-ed` verb after an auxiliary);
- context (after a determiner: noun or adjective; after "to", a modal or a pronoun: verb).

It is English only, approximate, and labelled so. Phase 2, if it proves too rough: the Moby
part-of-speech list (public domain) as a lexicon.

## Commands and events

| Command | Does |
|---|---|
| `get_insights` | The view: streak, week row, totals, speed, hours, top words, part-of-speech mix, milestones |
| `reset_insights` | Deletes the insights file |
| `set_config` `word_insights` | Turning it off deletes the word counts |

Event `insights-changed` after each recorded dictation, so an open tab refreshes.

## Testing

- Pure: streak advance (every rule in the table), word counter (stop words, capitals, digits,
  top-N), tagger on a hand-tagged sample with an accuracy floor, time saved and WPM math,
  milestone dates, 400-day folding.
- UI checks: tab and keyboard shortcut, week row states, freezes text, reset dialog, Word
  insights off hides the word cards, empty state, both themes.

## Decisions for the owner

1. Word insights when history is off: own switch, on by default (recommended), or off whenever
   history is off.
2. Whether meeting notes (your Me lines) count toward the streak and words.
3. The freeze economy: 1 per 10 active workdays, cap 10.
