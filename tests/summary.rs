//! A real summary through the linked llama.cpp. Set LOCAL_STT_TEST_SUMMARY_MODEL to a catalog model file;
//! LOCAL_STT_TEST_SUMMARY_NOTE to a saved note adds a run on a real call.

use local_stt::engine::Llm;
use local_stt::notes::{Note, Segment, Status};
use local_stt::summary::{self, write};
use std::path::PathBuf;

fn model() -> Option<(Llm, summary::SummaryModelId)> {
    let path = PathBuf::from(std::env::var("LOCAL_STT_TEST_SUMMARY_MODEL").ok()?);
    let name = path.file_name()?.to_str()?.to_string();
    let id = summary::CATALOG.iter().find(|m| m.file == name)?.id;
    Some((Llm::load(&path).expect("the summary model loads"), id))
}

fn run(llm: &Llm, id: summary::SummaryModelId, note: &Note) -> write::Summary {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get() as u32);
    let started = std::time::Instant::now();
    let mut generate = |prompt: &str, root: &str, max: u32| {
        let reply = llm.generate(prompt, write::GRAMMAR, root, write::N_CTX, max, threads);
        eprintln!("raw {root}: {reply:?}");
        reply
    };
    let mut progress =
        |done: usize, total: usize| eprintln!("part {done} of {total} at {:?}", started.elapsed());
    let s = write::summarize(note, Some("Ada"), id, 1, &mut generate, &mut progress)
        .expect("a summary");
    eprintln!(
        "{} in {:?}",
        serde_json::to_string_pretty(&s).unwrap(),
        started.elapsed()
    );
    s
}

#[test]
fn a_short_call_gets_an_overview_and_its_action_item() {
    let Some((llm, id)) = model() else {
        eprintln!("skipped: LOCAL_STT_TEST_SUMMARY_MODEL is not set");
        return;
    };
    let said = [
        ("me", "Let's review the September numbers before Friday."),
        ("s1", "Revenue is up eight percent, churn is flat."),
        (
            "me",
            "Good. Priya, can you send the updated deck to the whole team by Thursday?",
        ),
        ("s1", "Yes, I will send the deck by Thursday."),
    ];
    let note = Note {
        id: "t".into(),
        title: String::new(),
        started_ms: 0,
        ended_ms: Some(1),
        source: None,
        status: Status::Done,
        speakers: [("s1".to_string(), "Priya".to_string())]
            .into_iter()
            .collect(),
        segments: said
            .iter()
            .enumerate()
            .map(|(i, (who, text))| Segment {
                start_ms: i as u64 * 6_000,
                end_ms: i as u64 * 6_000 + 5_000,
                speaker: who.to_string(),
                text: text.to_string(),
            })
            .collect(),
        summary: None,
    };
    let s = run(&llm, id, &note);
    assert!(!s.overview.is_empty());
    assert!(
        s.action_items
            .iter()
            .any(|a| a.owner.as_deref() == Some("s1") && a.task.to_lowercase().contains("deck")),
        "{:?}",
        s.action_items
    );
}

#[test]
fn a_recorded_call_gets_a_checked_summary() {
    let (Some((llm, id)), Ok(file)) = (model(), std::env::var("LOCAL_STT_TEST_SUMMARY_NOTE"))
    else {
        eprintln!(
            "skipped: LOCAL_STT_TEST_SUMMARY_MODEL or LOCAL_STT_TEST_SUMMARY_NOTE is not set"
        );
        return;
    };
    let note: Note = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    let s = run(&llm, id, &note);
    assert!(!s.overview.is_empty() && !s.by_speaker.is_empty());
    let starts: std::collections::HashSet<u64> = note.segments.iter().map(|x| x.start_ms).collect();
    assert!(s.action_items.iter().all(|a| starts.contains(&a.at_ms)));
}
