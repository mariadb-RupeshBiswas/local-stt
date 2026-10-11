//! Local clean-up of a transcript before it is pasted: spoken list markers, fillers, spacing.

const FILLERS: &[&str] = &["uh", "uhh", "um", "umm", "uhm", "er", "erm", "hmm"];

/// English output gets every rule; other languages only get spacing fixes, since "um" or "point" mean other things there.
pub fn tidy_for(text: &str, english: bool) -> String {
    if english {
        tidy(text)
    } else {
        fix_spacing(text).trim().to_string()
    }
}

/// Applies every rule; text with nothing to fix comes back unchanged apart from trimming.
pub fn tidy(text: &str) -> String {
    let without_fillers = remove_fillers(text);
    let spaced = fix_spacing(&without_fillers);
    let listed = numbered_points(&spaced);
    let trimmed = listed.trim();
    // Only re-capitalise when a leading filler was removed; "iPhone works" must stay as spoken.
    if starts_with_filler(text) {
        capitalize_first(trimmed)
    } else {
        trimmed.to_string()
    }
}

fn starts_with_filler(text: &str) -> bool {
    words(text)
        .first()
        .is_some_and(|w| FILLERS.contains(&text[w.start..w.end].to_lowercase().as_str()))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Word {
    start: usize,
    end: usize,
}

fn words(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        let part_of_word = c.is_alphanumeric() || c == '\'';
        match (part_of_word, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push(Word { start: s, end: i });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(Word {
            start: s,
            end: text.len(),
        });
    }
    out
}

fn remove_fillers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for w in words(text) {
        let word = text[w.start..w.end].to_lowercase();
        if !FILLERS.contains(&word.as_str()) {
            continue;
        }
        out.push_str(&text[last..w.start]);
        // Swallow the comma that usually follows a spoken filler.
        let mut end = w.end;
        if text[end..].starts_with(',') {
            end += 1;
        }
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

fn fix_spacing(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' && out.ends_with(' ') {
            continue;
        }
        // " ," and " ." left behind by removed words.
        if (c == ',' || c == '.') && out.ends_with(' ') {
            out.pop();
        }
        if c == ',' && out.ends_with(',') {
            continue;
        }
        out.push(c);
        // "here.So" becomes "here. So"; "1.5" and "U.S." are left alone.
        let next = chars.get(i + 1).copied();
        let prev = if i > 0 {
            chars.get(i - 1).copied()
        } else {
            None
        };
        if matches!(c, '.' | '!' | '?')
            && next.is_some_and(char::is_uppercase)
            && prev.is_some_and(char::is_lowercase)
        {
            out.push(' ');
        }
    }
    out
}

#[derive(Debug)]
struct Marker {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
enum Kind {
    /// "point one", "point number 2"
    Point,
    /// "number one"
    Number,
    /// "first,", "secondly", "the third point"
    Ordinal,
}

const ORDINALS: [&str; 10] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];
const LIST_NOUNS: &[&str] = &[
    "point",
    "points",
    "thing",
    "things",
    "step",
    "steps",
    "item",
    "items",
    "question",
    "questions",
    "reason",
    "reasons",
    "idea",
    "ideas",
    "option",
    "options",
    "issue",
    "issues",
];
// After these, "point one" and "number two" are ordinary words ("at this point one...", "my number one priority").
const NOT_A_LIST_AFTER: &[&str] = &[
    "a", "an", "the", "this", "that", "these", "those", "my", "your", "our", "their", "his", "her",
    "its", "at", "of", "on", "in", "to", "which", "what", "some", "any", "every", "each", "no",
    "per", "by", "from",
];

const LEAD_INS: &[&str] = &["so", "okay", "ok", "alright", "right", "well", "now"];

fn lower(text: &str, w: Word) -> String {
    text[w.start..w.end].to_lowercase()
}

fn exact_number(word: &str) -> Option<u32> {
    match word {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        _ => word.parse::<u32>().ok().filter(|n| (1..=99).contains(n)),
    }
}

// Speech recognisers hear "two" as "to" and "four" as "for"; trusted only where that number is due.
fn heard_as(word: &str, expected: u32) -> bool {
    matches!(
        (word, expected),
        ("to" | "too", 2) | ("for", 4) | ("won", 1)
    )
}

// A marker word is a list marker at a clause start, or anywhere not after "the", "this", "my"...
fn list_position(text: &str, ws: &[Word], i: usize) -> bool {
    clause_start(text, ws, i) || !NOT_A_LIST_AFTER.contains(&lower(text, ws[i - 1]).as_str())
}

/// The marker of `kind` at word `i` if it carries the `expected` number.
fn marker_at(text: &str, ws: &[Word], i: usize, kind: Kind, expected: u32) -> Option<Marker> {
    let word = lower(text, ws[i]);
    match kind {
        Kind::Point | Kind::Number => {
            let lead = if matches!(kind, Kind::Point) {
                "point"
            } else {
                "number"
            };
            if word != lead {
                return None;
            }
            let mut j = i + 1;
            if matches!(kind, Kind::Point) && ws.get(j).is_some_and(|w| lower(text, *w) == "number")
            {
                j += 1;
            }
            let n = lower(text, *ws.get(j)?);
            let at_clause = clause_start(text, ws, i);
            let fits = exact_number(&n) == Some(expected) || (at_clause && heard_as(&n, expected));
            (fits && list_position(text, ws, i)).then(|| Marker {
                start: ws[i].start,
                end: ws[j].end,
            })
        }
        Kind::Ordinal => {
            let want = ORDINALS.get(expected as usize - 1)?;
            let after = &text[ws[i].end..];
            // "the second point is ..." names its list noun, so it counts anywhere.
            if word == *want
                && ws
                    .get(i + 1)
                    .is_some_and(|w| LIST_NOUNS.contains(&lower(text, *w).as_str()))
            {
                let start = if i > 0 && lower(text, ws[i - 1]) == "the" {
                    ws[i - 1].start
                } else {
                    ws[i].start
                };
                return Some(Marker {
                    start,
                    end: ws[i + 1].end,
                });
            }
            if !clause_start(text, ws, i) {
                return None;
            }
            if word == format!("{want}ly") {
                return Some(Marker {
                    start: ws[i].start,
                    end: ws[i].end,
                });
            }
            // A bare "First," needs its comma or colon; "first of all" is a marker too.
            if word == *want {
                if after.starts_with([',', ':']) {
                    return Some(Marker {
                        start: ws[i].start,
                        end: ws[i].end,
                    });
                }
                let of_all = ws
                    .get(i + 1)
                    .zip(ws.get(i + 2))
                    .filter(|(a, b)| lower(text, **a) == "of" && lower(text, **b) == "all");
                if let Some((_, all)) = of_all {
                    return Some(Marker {
                        start: ws[i].start,
                        end: all.end,
                    });
                }
            }
            None
        }
    }
}

// People mix forms ("first point ... point two"), so each number may come in any of them.
fn find_markers(text: &str, ws: &[Word]) -> Vec<Marker> {
    let mut markers: Vec<Marker> = Vec::new();
    let mut i = 0;
    while i < ws.len() {
        let expected = markers.len() as u32 + 1;
        let found = [Kind::Point, Kind::Number, Kind::Ordinal]
            .into_iter()
            .find_map(|kind| marker_at(text, ws, i, kind, expected));
        match found {
            Some(m) => {
                i = ws.iter().position(|w| w.start >= m.end).unwrap_or(ws.len());
                markers.push(m);
            }
            None => i += 1,
        }
    }
    markers
}

/// Spoken lists ("point one ... point two", "number one ...", "first, ... second, ...") become a
/// numbered list, only when the markers count up from one.
fn numbered_points(text: &str) -> String {
    let ws = words(text);
    let markers = find_markers(text, &ws);
    if markers.len() < 2 {
        return text.to_string();
    }
    let mut out = String::new();
    let mut intro = count_word_fixed(trim_joiners(&text[..markers[0].start]), markers.len());
    // A spoken lead-in ("So", "Okay") is not a heading.
    let bare = intro.trim_end_matches([',', '.']).to_lowercase();
    if LEAD_INS.contains(&bare.as_str()) {
        intro.clear();
    }
    if !intro.is_empty() {
        out.push_str(&intro);
        if !intro.ends_with(['.', ':', '?', '!']) {
            out.push(':');
        }
        out.push('\n');
    }
    let mut closing = "";
    for (n, m) in markers.iter().enumerate() {
        let item_end = markers.get(n + 1).map_or(text.len(), |next| next.start);
        let mut item = trim_joiners(&text[m.end..item_end]);
        if n + 1 == markers.len() {
            (item, closing) = split_closing(item);
        }
        let item = item_body(item);
        out.push_str(&format!("{}. ", n + 1));
        out.push_str(&capitalize_first(item));
        if !item.ends_with(['.', '?', '!']) {
            out.push('.');
        }
        if n + 1 < markers.len() {
            out.push('\n');
        }
    }
    if !closing.is_empty() {
        out.push_str("\n\n");
        out.push_str(closing);
    }
    out
}

// "point one is how we get there" lists "How we get there".
fn item_body(item: &str) -> &str {
    let mut t = item.trim();
    for lead in ["is that ", "is "] {
        if t.len() > lead.len() && t[..lead.len()].eq_ignore_ascii_case(lead) {
            t = t[lead.len()..].trim_start();
            break;
        }
    }
    t
}

// The last item ends at its first sentence when more follows ("... book the room. Thanks everyone.").
fn split_closing(item: &str) -> (&str, &str) {
    let bytes = item.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if matches!(b, b'.' | b'!' | b'?') && bytes.get(i + 1) == Some(&b' ') {
            let rest = item[i + 1..].trim();
            if rest.chars().next().is_some_and(char::is_uppercase) {
                return (&item[..=i], rest);
            }
        }
    }
    (item, "")
}

// "I need to points" before a two-item list is "I need two points".
fn count_word_fixed(intro: &str, count: usize) -> String {
    let ws = words(intro);
    if ws.len() >= 2 {
        let (prev, last) = (ws[ws.len() - 2], ws[ws.len() - 1]);
        let heard = lower(intro, prev);
        if LIST_NOUNS.contains(&lower(intro, last).as_str()) && heard_as(&heard, count as u32) {
            let spoken = if count == 2 {
                "two"
            } else if count == 4 {
                "four"
            } else {
                "one"
            };
            return format!("{}{}{}", &intro[..prev.start], spoken, &intro[prev.end..]);
        }
    }
    intro.to_string()
}

// A spoken list marker opens a clause: text start, after punctuation, or after "and"/"then"/"so".
fn clause_start(text: &str, ws: &[Word], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let gap = &text[ws[i - 1].end..ws[i].start];
    if gap.contains(['.', ',', ';', ':', '!', '?', '\n']) {
        return true;
    }
    let prev = text[ws[i - 1].start..ws[i - 1].end].to_lowercase();
    matches!(prev.as_str(), "and" | "then" | "so" | "also" | "next")
}

// Drops the commas, spaces and "and"/"then" that glue spoken points together.
fn trim_joiners(s: &str) -> &str {
    let mut t = s.trim_matches(|c: char| c.is_whitespace() || c == ',' || c == ':');
    loop {
        let lower = t.to_lowercase();
        let mut changed = false;
        for joiner in [" and", " then", " and then"] {
            if lower.ends_with(joiner) {
                t = t[..t.len() - joiner.len()].trim_end_matches([' ', ',']);
                changed = true;
                break;
            }
        }
        if !changed {
            return t;
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    // "iPhone" and "eBay" are spelled that way on purpose.
    if chars.clone().nth(1).is_some_and(char::is_uppercase) {
        return s.to_string();
    }
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_points_become_a_numbered_list() {
        let input = "So learn the template repo and improve it here. So you need to do two things, point one, improve the chart and point to check if any tables can also be improved or not.";
        let want = "So learn the template repo and improve it here. So you need to do two things:\n1. Improve the chart.\n2. Check if any tables can also be improved or not.";
        assert_eq!(tidy(input), want);
    }

    #[test]
    fn point_number_and_digits_work() {
        let input = "Agenda: point number 1 budget, point number 2 hiring, point number 3 travel";
        assert_eq!(tidy(input), "Agenda:\n1. Budget.\n2. Hiring.\n3. Travel.");
    }

    #[test]
    fn a_single_point_is_left_alone() {
        let input = "The point one should remember is simple.";
        assert_eq!(tidy(input), input);
    }

    #[test]
    fn points_out_of_order_are_left_alone() {
        let input = "point two first then point one later";
        assert_eq!(tidy(input), input);
    }

    #[test]
    fn to_is_only_two_when_two_is_due() {
        let input = "I want to go, point one eat, point to sleep";
        assert_eq!(tidy(input), "I want to go:\n1. Eat.\n2. Sleep.");
    }

    #[test]
    fn list_items_keep_intentional_lowercase_brands() {
        assert_eq!(
            tidy("Buy: point one iPhone case, point two eBay voucher"),
            "Buy:\n1. iPhone case.\n2. eBay voucher."
        );
    }

    #[test]
    fn point_inside_a_sentence_is_not_a_marker() {
        let input = "At this point one thing is clear and at that point to be fair we agree.";
        assert_eq!(tidy(input), input);
    }

    #[test]
    fn other_languages_only_get_spacing_fixes() {
        assert_eq!(
            tidy_for("Eu tenho um carro e um cão.", false),
            "Eu tenho um carro e um cão."
        );
        assert_eq!(
            tidy_for("Wir treffen uns um zehn Uhr, um zu reden.", false),
            "Wir treffen uns um zehn Uhr, um zu reden."
        );
        assert_eq!(tidy_for("Hola.Mundo", false), "Hola. Mundo");
        assert_eq!(
            tidy_for("uh, point one a, point two b", true),
            "1. A.\n2. B."
        );
    }

    #[test]
    fn fillers_and_their_commas_are_removed() {
        assert_eq!(
            tidy("Uh, make sure uh, the build, um, passes."),
            "Make sure the build, passes."
        );
    }

    #[test]
    fn only_fillers_become_empty() {
        assert_eq!(tidy("uh, um"), "");
        assert_eq!(tidy("Uh um."), ".");
    }

    #[test]
    fn words_containing_fillers_survive() {
        assert_eq!(tidy("Umbrella and hummus."), "Umbrella and hummus.");
    }

    #[test]
    fn missing_space_after_a_sentence_is_added() {
        assert_eq!(
            tidy("It works here.So try it."),
            "It works here. So try it."
        );
        assert_eq!(
            tidy("Version 1.5 of the U.S. plan"),
            "Version 1.5 of the U.S. plan"
        );
    }

    #[test]
    fn owner_sample_points_after_points_become_a_list() {
        let input = "Hello, are you listening? So I need to points point one is how we are going to go there point two is how we are going to s";
        let want = "Hello, are you listening? So I need two points:\n1. How we are going to go there.\n2. How we are going to s.";
        assert_eq!(tidy(input), want);
    }

    #[test]
    fn owner_sample_mixed_markers_become_a_list() {
        let input = "So first point we need to check how the work is going on. Point two like this should be worked on and we should be working on it.";
        let want = "1. We need to check how the work is going on.\n2. Like this should be worked on and we should be working on it.";
        assert_eq!(tidy(input), want);
    }

    #[test]
    fn mixed_markers_count_up_in_any_form() {
        let input = "Point one, book the room. The second point is the agenda. Number three, send the invite.";
        let want = "1. Book the room.\n2. The agenda.\n3. Send the invite.";
        assert_eq!(tidy(input), want);
    }

    #[test]
    fn whisper_style_capitalised_points_with_is() {
        let input =
            "So I need to points. Point 1 is how we get there. Point 2 is how we pay for it.";
        let want = "So I need two points.\n1. How we get there.\n2. How we pay for it.";
        assert_eq!(tidy(input), want);
    }

    #[test]
    fn number_one_number_two_become_a_list() {
        assert_eq!(
            tidy("Number one, buy milk. Number two, call mom."),
            "1. Buy milk.\n2. Call mom."
        );
    }

    #[test]
    fn ordinals_with_commas_become_a_list() {
        assert_eq!(
            tidy(
                "Before we start. First, check the logs. Second, restart the app. Third, tell me."
            ),
            "Before we start.\n1. Check the logs.\n2. Restart the app.\n3. Tell me."
        );
        assert_eq!(
            tidy("Firstly we test, secondly we ship."),
            "1. We test.\n2. We ship."
        );
        assert_eq!(
            tidy("The first point is speed and the second point is cost."),
            "1. Speed.\n2. Cost."
        );
    }

    #[test]
    fn a_closing_remark_stays_out_of_the_last_item() {
        assert_eq!(
            tidy("Two things: point one call Sam, point two book the room. Thanks everyone."),
            "Two things:\n1. Call Sam.\n2. Book the room.\n\nThanks everyone."
        );
    }

    #[test]
    fn prose_with_numbers_and_ordinals_stays_prose() {
        for input in [
            "My number one priority is speed and my number two is cost.",
            "The first time I saw it, the second guess was right.",
            "We grew point one percent this quarter.",
            "At first it failed, then second time lucky.",
            "He finished first, second was Ana.",
            "Let me make a point. One thing is clear.",
        ] {
            assert_eq!(tidy(input), input);
        }
    }

    #[test]
    fn four_heard_as_for_in_the_lead_in() {
        assert_eq!(
            tidy("I have for points point one apples point two pears point three plums point four figs"),
            "I have four points:\n1. Apples.\n2. Pears.\n3. Plums.\n4. Figs."
        );
    }

    #[test]
    fn er_is_a_filler_too() {
        assert_eq!(tidy("Er, send it today."), "Send it today.");
    }

    #[test]
    fn empty_and_plain_text() {
        assert_eq!(tidy(""), "");
        assert_eq!(tidy("  hello world  "), "hello world");
        assert_eq!(tidy("iPhone works"), "iPhone works");
    }
}
