//! Verified model download through curl. Owned by Task 6.

use std::path::Path;

pub fn download(
    _url: &str,
    _sha256_hex: &str,
    _dest: &Path,
    _progress: &dyn Fn(u64),
) -> Result<(), String> {
    todo!("Task 6")
}

pub fn sha256_file(_path: &Path) -> std::io::Result<String> {
    todo!("Task 6")
}
