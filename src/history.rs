//! Dictation history as JSON Lines.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub ts_ms: u64,
    pub text: String,
    pub duration_ms: u64,
    pub model: String,
    pub ok: bool,
}

// About 2,000 typical dictations; older ones are dropped so the file never grows without bound.
const KEEP_ENTRIES: usize = 2_000;
const TRIM_ABOVE_BYTES: u64 = 1024 * 1024;

// Appends run on the worker while deletes come from the UI; a rewrite must not drop a fresh line.
static FILE: Mutex<()> = Mutex::new(());

fn file_lock() -> std::sync::MutexGuard<'static, ()> {
    FILE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn append(path: &Path, e: &Entry) -> std::io::Result<()> {
    let _guard = file_lock();
    let mut e = e.clone();
    // ts_ms is the id delete() goes by, so it stays unique even when the clock steps back.
    if let Some(newest) = read_newest_first(path, 1)?.first() {
        e.ts_ms = e.ts_ms.max(newest.ts_ms + 1);
    }
    append_line(path, &e)?;
    trim_if_large(path, TRIM_ABOVE_BYTES, KEEP_ENTRIES)
}

#[derive(Deserialize)]
struct Stamp {
    ts_ms: u64,
}

/// Removes the entries with these timestamps (their ids) and returns how many went.
pub fn delete(path: &Path, ids: &HashSet<u64>) -> std::io::Result<usize> {
    let _guard = file_lock();
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    // Works on raw lines so a line this build cannot read survives a delete untouched.
    let mut kept = Vec::with_capacity(bytes.len());
    let mut removed = 0;
    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let chosen = serde_json::from_slice::<Stamp>(line).is_ok_and(|s| ids.contains(&s.ts_ms));
        if chosen {
            removed += 1;
        } else {
            kept.extend_from_slice(line);
            kept.push(b'\n');
        }
    }
    if removed > 0 {
        replace(path, &kept)?;
    }
    Ok(removed)
}

fn trim_if_large(path: &Path, max_bytes: u64, keep: usize) -> std::io::Result<()> {
    if std::fs::metadata(path)?.len() <= max_bytes {
        return Ok(());
    }
    let mut newest = read_newest_first(path, keep)?;
    newest.reverse();
    let mut bytes = Vec::new();
    for entry in &newest {
        bytes.extend(serde_json::to_vec(entry).map_err(std::io::Error::other)?);
        bytes.push(b'\n');
    }
    replace(path, &bytes)
}

// Writes a private temp file, flushes it to disk, then swaps it in whole, so a power cut leaves old or new.
pub(crate) fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp_name);
    let _ = std::fs::remove_file(&tmp);
    let mut file = crate::config::private_options()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)?;
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        let _ = std::fs::File::open(dir).and_then(|d| d.sync_all());
    }
    Ok(())
}

fn append_line(path: &Path, e: &Entry) -> std::io::Result<()> {
    // serde_json escapes newlines, so one entry is always one line.
    let mut line = serde_json::to_vec(e).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut file = crate::config::private_options()
        .append(true)
        .create(true)
        .open(path)?;
    file.write_all(&line)
}

pub fn read_newest_first(path: &Path, limit: usize) -> std::io::Result<Vec<Entry>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    // ponytail: reads the whole file, fine for thousands of entries; seek from the end if it grows.
    let mut entries = Vec::new();
    for line in bytes.split(|b| *b == b'\n').rev() {
        if entries.len() >= limit {
            break;
        }
        if let Ok(entry) = serde_json::from_slice::<Entry>(line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub fn clear(path: &Path) -> std::io::Result<()> {
    let _guard = file_lock();
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("lstt-hist-{n}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join("history.jsonl")
    }
    fn e(ts: u64, t: &str) -> Entry {
        Entry {
            ts_ms: ts,
            text: t.into(),
            duration_ms: 1000,
            model: "small".into(),
            ok: true,
        }
    }
    #[test]
    fn newest_first_and_limit() {
        let p = tmp("order");
        for i in 1..=5 {
            append(&p, &e(i, &format!("t{i}"))).unwrap();
        }
        let r = read_newest_first(&p, 3).unwrap();
        assert_eq!(r.iter().map(|x| x.ts_ms).collect::<Vec<_>>(), vec![5, 4, 3]);
    }
    #[test]
    fn bad_lines_skipped() {
        let p = tmp("bad");
        append(&p, &e(1, "ok")).unwrap();
        writeln!(
            std::fs::OpenOptions::new().append(true).open(&p).unwrap(),
            "garbage"
        )
        .unwrap();
        append(&p, &e(2, "ok2")).unwrap();
        assert_eq!(read_newest_first(&p, 10).unwrap().len(), 2);
    }
    #[test]
    fn text_with_newlines_roundtrips() {
        let p = tmp("nl");
        append(&p, &e(1, "a\nb")).unwrap();
        assert_eq!(read_newest_first(&p, 1).unwrap()[0].text, "a\nb");
    }
    #[test]
    fn missing_file_is_empty() {
        assert!(read_newest_first(&tmp("none"), 10).unwrap().is_empty());
    }
    #[test]
    fn clear_empties() {
        let p = tmp("clr");
        append(&p, &e(1, "x")).unwrap();
        clear(&p).unwrap();
        assert!(read_newest_first(&p, 10).unwrap().is_empty());
    }
    #[test]
    fn large_file_is_trimmed_to_newest() {
        let p = tmp("trim");
        let _ = clear(&p);
        for i in 1..=50 {
            append_line(&p, &e(i, "some dictated text")).unwrap();
        }
        trim_if_large(&p, 100, 10).unwrap();
        let kept = read_newest_first(&p, 100).unwrap();
        assert_eq!(kept.len(), 10);
        assert_eq!(kept[0].ts_ms, 50);
        assert_eq!(kept[9].ts_ms, 41);
    }
    #[test]
    fn delete_removes_only_the_chosen_entries() {
        let p = tmp("del");
        let _ = clear(&p);
        for i in 1..=5 {
            append(&p, &e(i, &format!("t{i}"))).unwrap();
        }
        let gone = delete(&p, &HashSet::from([2, 4, 99])).unwrap();
        assert_eq!(gone, 2);
        let left = read_newest_first(&p, 10).unwrap();
        assert_eq!(
            left.iter().map(|x| x.ts_ms).collect::<Vec<_>>(),
            vec![5, 3, 1]
        );
        append(&p, &e(6, "after")).unwrap();
        assert_eq!(read_newest_first(&p, 1).unwrap()[0].text, "after");
    }
    #[test]
    fn delete_keeps_lines_it_cannot_read() {
        let p = tmp("del-keep");
        let _ = clear(&p);
        append(&p, &e(1, "a")).unwrap();
        writeln!(
            std::fs::OpenOptions::new().append(true).open(&p).unwrap(),
            "{{\"torn"
        )
        .unwrap();
        append(&p, &e(2, "b")).unwrap();
        assert_eq!(delete(&p, &HashSet::from([1])).unwrap(), 1);
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("{\"torn"), "{raw}");
        assert_eq!(read_newest_first(&p, 10).unwrap().len(), 1);
    }
    #[test]
    fn stamps_stay_unique_when_the_clock_repeats() {
        let p = tmp("uniq");
        let _ = clear(&p);
        append(&p, &e(500, "a")).unwrap();
        append(&p, &e(500, "b")).unwrap();
        append(&p, &e(0, "c")).unwrap();
        let ts: Vec<u64> = read_newest_first(&p, 10)
            .unwrap()
            .iter()
            .map(|x| x.ts_ms)
            .collect();
        assert_eq!(ts, vec![502, 501, 500]);
    }
    #[test]
    fn delete_on_missing_file_is_zero() {
        assert_eq!(delete(&tmp("del-none"), &HashSet::from([1])).unwrap(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn history_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let p = tmp("perm");
        let _ = clear(&p);
        append(&p, &e(1, "x")).unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
