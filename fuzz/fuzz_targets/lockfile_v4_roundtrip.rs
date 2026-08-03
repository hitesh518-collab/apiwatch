#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 5_000_000 {
        return;
    }

    let mut file = tempfile::NamedTempFile::new().ok();
    let (path, mut file) = match file {
        Some(f) => {
            let p = f.path().to_path_buf();
            (p, f)
        }
        None => return,
    };

    if file.write_all(data).is_err() {
        return;
    }
    let _ = file.flush();

    let lock = match apiwatch::lockfile::load(&path) {
        Ok(l) => l,
        Err(_) => return,
    };

    let rendered = match apiwatch::lockfile::render(&lock) {
        Ok(r) => r,
        Err(_) => return,
    };

    let reparse_path = match tempfile::NamedTempFile::new() {
        Ok(f) => f.path().to_path_buf(),
        Err(_) => return,
    };
    if std::fs::write(&reparse_path, &rendered).is_err() {
        return;
    }

    let roundtripped = match apiwatch::lockfile::load(&reparse_path) {
        Ok(l) => l,
        Err(_) => return,
    };

    let rerendered = match apiwatch::lockfile::render(&roundtripped) {
        Ok(r) => r,
        Err(_) => return,
    };

    assert_eq!(rendered, rerendered, "v4 roundtrip mismatch");
});
