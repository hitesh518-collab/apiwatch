#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.len() > 10_000_000 {
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

    let _ = apiwatch::openapi::load_contract(&path);
});
