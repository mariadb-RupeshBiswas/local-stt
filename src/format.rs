//! Local clean-up of a transcript before it is pasted: spoken list markers, fillers, spacing.

const FILLERS: &[&str] = &["uh", "uhh", "um", "umm", "uhm", "erm", "hmm"];

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

fn number_word(word: &str, expected: u32) -> Option<u32> {
    let lower = word.to_lowercase();
    let exact = match lower.as_str() {
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
        _ => lower.parse::<u32>().ok(),
    };
    if exact.is_some() {
        return exact;
    }
    // Speech recognisers hear "two" as "to" and "four" as "for"; accept them only where that number is due.
    match (lower.as_str(), expected) {
        ("to" | "too", 2) | ("for", 4) | ("won", 1) => Some(expected),
        _ => None,
    }
}

#[derive(Debug)]
struct Marker {
    start: usize,
    end: usize,
}

/// "point one ... point two ..." becomes a numbered list, only when the points count up from one.
fn numbered_points(text: &str) -> String {
    let ws = words(text);
    let mut markers: Vec<Marker> = Vec::new();
    let mut i = 0;
    while i < ws.len() {
        let expected = markers.len() as u32 + 1;
        let word = &text[ws[i].start..ws[i].end];
        if !word.eq_ignore_ascii_case("point") {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        if ws
            .get(j)
            .is_some_and(|w| text[w.start..w.end].eq_ignore_ascii_case("number"))
        {
            j += 1;
        }
        let number = ws
            .get(j)
            .and_then(|w| number_word(&text[w.start..w.end], expected));
        if number == Some(expected) {
            markers.push(Marker {
                start: ws[i].start,
                end: ws[j].end,
            });
            i = j + 1;
        } else {
            i += 1;
        }
    }
    if markers.len() < 2 {
        return text.to_string();
    }
    let mut out = String::new();
    let intro = trim_joiners(&text[..markers[0].start]);
    if !intro.is_empty() {
        out.push_str(intro);
        if !intro.ends_with(['.', ':', '?', '!']) {
            out.push(':');
        }
        out.push('\n');
    }
    for (n, m) in markers.iter().enumerate() {
        let item_end = markers.get(n + 1).map_or(text.len(), |next| next.start);
        let item = trim_joiners(&text[m.end..item_end]);
        out.push_str(&format!("{}. ", n + 1));
        out.push_str(&capitalize_first(item));
        if !item.ends_with(['.', '?', '!']) {
            out.push('.');
        }
        if n + 1 < markers.len() {
            out.push('\n');
        }
    }
    out
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
        let input = "Agenda point number 1 budget point number 2 hiring point number 3 travel";
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
        let input = "I want to go point one eat point to sleep";
        assert_eq!(tidy(input), "I want to go:\n1. Eat.\n2. Sleep.");
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
    fn empty_and_plain_text() {
        assert_eq!(tidy(""), "");
        assert_eq!(tidy("  hello world  "), "hello world");
        assert_eq!(tidy("iPhone works"), "iPhone works");
    }
}
