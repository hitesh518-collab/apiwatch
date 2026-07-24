use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

struct Fixture {
    _temporary: TempDir,
    manifest: PathBuf,
    cache: PathBuf,
    privacy: PathBuf,
    json_out: PathBuf,
    markdown_out: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = root.join("testdata/openapi/verify_matching.yaml");
        let privacy = root.join("testdata/openapi/privacy_sentinels.yaml");
        let temporary = tempfile::tempdir().unwrap();
        let cache = temporary.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let payload = fs::read(&source).unwrap();
        fs::write(cache.join("simple.yaml"), &payload).unwrap();
        let sha256 = format!("{:x}", Sha256::digest(&payload));
        let manifest = temporary.path().join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec_pretty(&json!({
                "version": 1,
                "max_total_bytes": 1_048_576,
                "specs": [{
                    "name": "simple",
                    "file": "simple.yaml",
                    "url": concat!(
                        "https://raw.githubusercontent.com/example/api/",
                        "0123456789abcdef0123456789abcdef01234567/openapi.yaml"
                    ),
                    "sha256": sha256,
                    "max_bytes": 1_048_576,
                    "status": "passing"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let json_out = temporary.path().join("report.json");
        let markdown_out = temporary.path().join("report.md");
        Self {
            _temporary: temporary,
            manifest,
            cache,
            privacy,
            json_out,
            markdown_out,
        }
    }

    fn run(&self, check: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_apiwatch-lock-size-report"));
        command.args([
            "--manifest",
            path_text(&self.manifest),
            "--compat-dir",
            path_text(&self.cache),
            "--privacy-fixture",
            path_text(&self.privacy),
            "--max-lock-bytes",
            "5242880",
            "--json-out",
            path_text(&self.json_out),
            "--markdown-out",
            path_text(&self.markdown_out),
        ]);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn writes_deterministic_reports_and_checks_existing_bytes() {
    let fixture = Fixture::new();
    let first = fixture.run(false);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let json = fs::read_to_string(&fixture.json_out).unwrap();
    let markdown = fs::read_to_string(&fixture.markdown_out).unwrap();
    assert!(!json.contains(path_text(fixture.cache.parent().unwrap())));
    assert!(!markdown.contains(path_text(fixture.cache.parent().unwrap())));

    assert!(fixture.run(true).status.success());
    fs::write(&fixture.json_out, "changed\n").unwrap();
    assert_eq!(fixture.run(true).status.code(), Some(1));
}

#[test]
fn input_failure_preserves_existing_reports() {
    let fixture = Fixture::new();
    fs::write(&fixture.json_out, "preserve-me").unwrap();
    fs::write(&fixture.markdown_out, "preserve-me").unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
    manifest["specs"][0]["sha256"] = json!("0".repeat(64));
    fs::write(
        &fixture.manifest,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let output = fixture.run(false);
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&fixture.json_out).unwrap(),
        "preserve-me"
    );
    assert_eq!(
        fs::read_to_string(&fixture.markdown_out).unwrap(),
        "preserve-me"
    );
}
