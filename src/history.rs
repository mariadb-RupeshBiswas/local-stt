//! Dictation history as JSON Lines.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub ts_ms: u64,
    pub text: String,
    pub duration_ms: u64,
    pub model: String,
    pub ok: bool,
}

pub fn append(path: &Path, e: &Entry) -> std::io::Result<()> {
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
