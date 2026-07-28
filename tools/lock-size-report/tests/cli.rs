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
    v4_json_out: PathBuf,
    v4_markdown_out: PathBuf,
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
        let v4_json_out = temporary.path().join("v4-report.json");
        let v4_markdown_out = temporary.path().join("v4-report.md");
        Self {
            _temporary: temporary,
            manifest,
            cache,
            privacy,
            json_out,
            markdown_out,
            v4_json_out,
            v4_markdown_out,
        }
    }

    fn run(&self, check: bool, include_v4: bool) -> Output {
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
        if include_v4 {
            command.args([
                "--v4-json-out",
                path_text(&self.v4_json_out),
                "--v4-markdown-out",
                path_text(&self.v4_markdown_out),
            ]);
        }
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
    let phase_1 = fixture.run(false, false);
    assert!(
        phase_1.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&phase_1.stderr)
    );
    let json = fs::read_to_string(&fixture.json_out).unwrap();
    let markdown = fs::read_to_string(&fixture.markdown_out).unwrap();
    let preserved_json = format!("{json}\n");
    let preserved_markdown = format!("{markdown}\n");
    fs::write(&fixture.json_out, &preserved_json).unwrap();
    fs::write(&fixture.markdown_out, &preserved_markdown).unwrap();

    let phase_2 = fixture.run(false, true);
    assert!(
        phase_2.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&phase_2.stderr)
    );
    assert_eq!(
        fs::read_to_string(&fixture.json_out).unwrap(),
        preserved_json
    );
    assert_eq!(
        fs::read_to_string(&fixture.markdown_out).unwrap(),
        preserved_markdown
    );
    let v4_json = fs::read_to_string(&fixture.v4_json_out).unwrap();
    let v4_markdown = fs::read_to_string(&fixture.v4_markdown_out).unwrap();
    assert!(!json.contains(path_text(fixture.cache.parent().unwrap())));
    assert!(!markdown.contains(path_text(fixture.cache.parent().unwrap())));
    assert!(!v4_json.contains(path_text(fixture.cache.parent().unwrap())));
    assert!(!v4_markdown.contains(path_text(fixture.cache.parent().unwrap())));
    let v4_report: serde_json::Value = serde_json::from_str(&v4_json).unwrap();
    let v4_contract_bytes = v4_report["corpus"][0]["v4_contract_bytes"]
        .as_u64()
        .expect("passing corpus row should include v4 payload bytes");
    assert!(v4_contract_bytes > 0);
    assert!(v4_contract_bytes <= 5_242_880);
    assert_eq!(v4_report["corpus"][0]["within_ceiling"], true);
    assert!(v4_markdown.contains("# APIWatch Phase 2 v4 Lock-Size Report"));
    assert!(v4_markdown.contains(&format!("{v4_contract_bytes} (fits)")));

    assert!(fixture.run(true, true).status.success());
    fs::write(&fixture.json_out, json.replace('\n', "\r\n")).unwrap();
    fs::write(&fixture.markdown_out, markdown.replace('\n', "\r\n")).unwrap();
    assert!(
        fixture.run(true, true).status.success(),
        "report checks should canonicalize platform line endings"
    );
    fs::write(&fixture.json_out, "changed\n").unwrap();
    assert_eq!(fixture.run(true, false).status.code(), Some(1));

    fs::write(&fixture.json_out, &json).unwrap();
    fs::write(&fixture.v4_json_out, "changed\n").unwrap();
    assert_eq!(fixture.run(true, true).status.code(), Some(1));
}

#[test]
fn input_failure_preserves_existing_reports() {
    let fixture = Fixture::new();
    fs::write(&fixture.json_out, "preserve-me").unwrap();
    fs::write(&fixture.markdown_out, "preserve-me").unwrap();
    fs::write(&fixture.v4_json_out, "preserve-me").unwrap();
    fs::write(&fixture.v4_markdown_out, "preserve-me").unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
    manifest["specs"][0]["sha256"] = json!("0".repeat(64));
    fs::write(
        &fixture.manifest,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let output = fixture.run(false, true);
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
    assert_eq!(
        fs::read_to_string(&fixture.v4_json_out).unwrap(),
        "preserve-me"
    );
    assert_eq!(
        fs::read_to_string(&fixture.v4_markdown_out).unwrap(),
        "preserve-me"
    );
}
