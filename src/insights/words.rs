//! Word counting for Your words: what to count, and a rule-based English part-of-speech tagger.

use serde::{Deserialize, Serialize};

pub const MAX_WORDS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tag {
    Noun,
    Verb,
    Adjective,
    Adverb,
    Other,
}

const PRONOUNS: &[&str] = &[
    "i",
    "me",
    "my",
    "mine",
    "myself",
    "you",
    "your",
    "yours",
    "yourself",
    "he",
    "him",
    "his",
    "himself",
    "she",
    "her",
    "hers",
    "herself",
    "it",
    "its",
    "itself",
    "we",
    "us",
    "our",
    "ours",
    "ourselves",
    "they",
    "them",
    "their",
    "theirs",
    "themselves",
    "who",
    "whom",
    "whose",
    "which",
    "what",
    "that",
    "this",
    "these",
    "those",
    "someone",
    "something",
    "anyone",
    "anything",
    "everyone",
    "everything",
    "nothing",
    "nobody",
];
const DETERMINERS: &[&str] = &[
    "the", "a", "an", "some", "any", "each", "every", "no", "all", "both", "few", "many", "much",
    "more", "most", "other", "another", "such", "several", "own",
];
const PREPOSITIONS: &[&str] = &[
    "of", "in", "on", "at", "by", "for", "with", "about", "against", "between", "into", "through",
    "during", "before", "after", "above", "below", "from", "up", "down", "out", "off", "over",
    "under", "again", "once", "than", "via", "per", "around", "across", "behind", "beyond",
    "within", "without", "upon", "toward", "towards", "until", "while", "like",
];
const CONJUNCTIONS: &[&str] = &[
    "and", "but", "or", "nor", "so", "yet", "because", "although", "though", "if", "unless",
    "whether", "when", "where", "why", "how", "since", "as", "then", "also",
];
const AUXILIARIES: &[&str] = &[
    "am", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "having", "do",
    "does", "did", "doing", "done",
];
const MODALS: &[&str] = &[
    "can", "could", "will", "would", "shall", "should", "may", "might", "must", "cannot", "wont",
];
// Frequent words that carry no insight but belong to no closed class.
const EXTRA_STOP: &[&str] = &[
    "not", "just", "very", "too", "only", "really", "even", "still", "now", "here", "there", "get",
    "got", "going", "gonna", "let", "lets", "one", "two", "yeah", "yes", "okay", "ok", "well",
    "thing", "things", "way", "lot", "kind", "sort", "actually", "maybe", "right", "know", "think",
    "mean", "said", "say", "see", "want", "make", "made", "take", "come", "go", "back", "same",
    "need", "use", "used",
];

fn is_stop(word: &str) -> bool {
    closed_class(word) || EXTRA_STOP.contains(&word)
}

fn closed_class(word: &str) -> bool {
    [
        PRONOUNS,
        DETERMINERS,
        PREPOSITIONS,
        CONJUNCTIONS,
        AUXILIARIES,
        MODALS,
    ]
    .iter()
    .any(|list| list.contains(&word))
}

const NOUN_SUFFIXES: &[&str] = &[
    "tion", "sion", "ment", "ness", "ity", "ship", "ance", "ence",
];
const ADJECTIVE_SUFFIXES: &[&str] = &["ous", "ful", "able", "ible", "ive", "al", "less", "ic"];

fn has_suffix(word: &str, suffixes: &[&str]) -> bool {
    // Needs a three-letter stem, so "ship" and "ice" are not read as suffixes.
    suffixes
        .iter()
        .any(|s| word.len() >= s.len() + 3 && word.ends_with(s))
}

/// Guesses a word's tag from its spelling and the word before it.
fn tag_word(word: &str, prev: Option<(&str, Tag)>) -> Tag {
    if closed_class(word) {
        return Tag::Other;
    }
    if word.len() > 4 && word.ends_with("ly") {
        return Tag::Adverb;
    }
    if has_suffix(word, NOUN_SUFFIXES) {
        return Tag::Noun;
    }
    if has_suffix(word, ADJECTIVE_SUFFIXES) {
        return Tag::Adjective;
    }
    let (before, before_tag) = prev.unwrap_or(("", Tag::Other));
    let after_determiner = DETERMINERS.contains(&before) || before_tag == Tag::Adjective;
    let wants_verb = before == "to"
        || MODALS.contains(&before)
        || AUXILIARIES.contains(&before)
        || PRONOUNS.contains(&before);
    let verb_ending = word.len() > 4 && (word.ends_with("ing") || word.ends_with("ed"));
    if verb_ending && wants_verb {
        return Tag::Verb;
    }
    // A past-tense ending after anything but a determiner reads as a verb ("the customer asked").
    if word.len() > 4 && word.ends_with("ed") && !after_determiner {
        return Tag::Verb;
    }
    if after_determiner {
        return Tag::Noun;
    }
    if wants_verb {
        return Tag::Verb;
    }
    Tag::Noun
}

struct Token {
    text: String,
    capitalised: bool,
    sentence_start: bool,
}

// Splits like format.rs: letters, digits and apostrophes make a word.
fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut sentence_start = true;
    let mut push = |range: &str, sentence_start: bool| {
        if range.chars().any(char::is_alphanumeric) {
            out.push(Token {
                text: range.to_lowercase(),
                capitalised: range.chars().next().is_some_and(char::is_uppercase),
                sentence_start,
            });
            true
        } else {
            false
        }
    };
    for (i, c) in text.char_indices() {
        let part = c.is_alphanumeric() || c == '\'';
        match (part, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if push(&text[s..i], sentence_start) {
                    sentence_start = false;
                }
                start = None;
            }
            _ => {}
        }
        if matches!(c, '.' | '!' | '?') {
            sentence_start = true;
        }
    }
    if let Some(s) = start {
        push(&text[s..], sentence_start);
    }
    out
}

/// How many words the text holds, for totals and speed.
pub fn count(text: &str) -> u64 {
    tokens(text).len() as u64
}

/// The words worth counting, each with its tag; names, stop words, digits and short words are left out.
pub fn extract(text: &str) -> Vec<(String, Tag)> {
    let mut out = Vec::new();
    let mut prev: Option<(String, Tag)> = None;
    for token in tokens(text) {
        let tag = tag_word(&token.text, prev.as_ref().map(|(w, t)| (w.as_str(), *t)));
        // An adverb is transparent: "they quickly finished" still reads as pronoun then verb.
        if tag != Tag::Adverb {
            prev = Some((token.text.clone(), tag));
        }
        let name = token.capitalised && !token.sentence_start;
        let letters = token.text.chars().count();
        let skip = name
            || letters < 3
            || token.text.chars().any(|c| c.is_ascii_digit() || c == '\'')
            || is_stop(&token.text);
        if !skip {
            out.push((token.text, tag));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words_of(text: &str) -> Vec<String> {
        extract(text).into_iter().map(|(w, _)| w).collect()
    }

    #[test]
    fn stop_words_digits_and_short_words_are_skipped() {
        assert_eq!(
            words_of("The pricing and the 3 plans of it"),
            ["pricing", "plans"]
        );
        assert_eq!(
            words_of("room101 is at b2b and we go"),
            Vec::<String>::new()
        );
        assert_eq!(words_of("an ox is"), Vec::<String>::new());
        assert_eq!(words_of("don't stop"), ["stop"]);
    }

    #[test]
    fn mid_sentence_capitals_are_names_but_sentence_starts_count() {
        assert_eq!(
            words_of("Pricing matters. Ask Priya about budgets"),
            ["pricing", "matters", "ask", "budgets"]
        );
        assert_eq!(
            words_of("Roadmap review! Budget talk"),
            ["roadmap", "review", "budget", "talk"]
        );
    }

    #[test]
    fn count_matches_word_splitting() {
        assert_eq!(count("Hello, world. It's 5 pm!"), 5);
        assert_eq!(count(" ... "), 0);
        assert_eq!(count(""), 0);
    }

    #[test]
    fn tagger_accuracy_floor() {
        use Tag::*;
        let sentences: &[(&str, &[(&str, Tag)])] = &[
            (
                "We need to ship the pricing review quickly.",
                &[
                    ("ship", Verb),
                    ("pricing", Noun),
                    ("review", Noun),
                    ("quickly", Adverb),
                ],
            ),
            (
                "She will deliver a useful report tomorrow.",
                &[("deliver", Verb), ("useful", Adjective), ("report", Noun)],
            ),
            (
                "They quickly finished the valuable meeting.",
                &[
                    ("quickly", Adverb),
                    ("finished", Verb),
                    ("valuable", Adjective),
                    ("meeting", Noun),
                ],
            ),
            (
                "I can write a helpful script to build the feature.",
                &[
                    ("write", Verb),
                    ("helpful", Adjective),
                    ("script", Noun),
                    ("build", Verb),
                    ("feature", Noun),
                ],
            ),
            (
                "The customer asked for a quote.",
                &[("customer", Noun), ("asked", Verb), ("quote", Noun)],
            ),
            (
                "Our manager wants to improve the decision process.",
                &[
                    ("manager", Noun),
                    ("improve", Verb),
                    ("decision", Noun),
                    ("process", Noun),
                ],
            ),
            (
                "You should test the dangerous change carefully.",
                &[
                    ("test", Verb),
                    ("dangerous", Adjective),
                    ("change", Noun),
                    ("carefully", Adverb),
                ],
            ),
            (
                "The team will launch a creative campaign.",
                &[
                    ("team", Noun),
                    ("launch", Verb),
                    ("creative", Adjective),
                    ("campaign", Noun),
                ],
            ),
        ];
        let (mut right, mut total) = (0, 0);
        for (text, expected) in sentences {
            let got = extract(text);
            for (word, tag) in *expected {
                total += 1;
                match got.iter().find(|(w, _)| w == word) {
                    Some((_, t)) if t == tag => right += 1,
                    other => eprintln!("{word}: wanted {tag:?}, got {other:?}"),
                }
            }
        }
        assert!(total >= 30, "{total}");
        assert!(right * 100 >= total * 75, "{right} of {total}");
    }

    #[test]
    fn closed_class_words_are_never_content() {
        for w in ["the", "would", "they", "with", "because", "been"] {
            assert_eq!(tag_word(w, None), Tag::Other);
            assert!(is_stop(w));
        }
    }
}
