use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn distill_bin() -> &'static str {
    env!("CARGO_BIN_EXE_distill")
}

fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("distill-test-{unique}-{name}"));
    fs::write(&path, contents).expect("write temp file");
    path
}

#[test]
fn stdin_dry_run_prints_markdown_without_writing() {
    let mut child = Command::new(distill_bin())
        .args(["--stdin", "--dry-run", "--no-frontmatter"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn distill");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"<html><body><article><h1>Hello</h1><p>World</p></article></body></html>")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for distill");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("Hello"));
    assert!(stdout.contains("World"));
    assert!(stderr.contains("DRY RUN MODE"));
}

#[test]
fn fast_flag_errors_cleanly_without_feature() {
    let mut child = Command::new(distill_bin())
        .args(["--stdin", "--dry-run", "--fast"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn distill");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"<html><body><p>Hello</p></body></html>")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for distill");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--features fast"));
}

#[test]
fn batch_dry_run_lists_urls_from_file() {
    let path = temp_file(
        "urls.txt",
        "https://example.com/one\n# comment\n\nhttps://example.com/two\n",
    );

    let output = Command::new(distill_bin())
        .args(["--batch", path.to_str().expect("utf8 path"), "--dry-run"])
        .output()
        .expect("run distill");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would process: https://example.com/one"));
    assert!(stdout.contains("Would process: https://example.com/two"));

    let _ = fs::remove_file(path);
}

#[test]
fn local_html_file_is_accepted_as_single_input() {
    let path = temp_file(
        "page.html",
        "<html><head><title>Local</title></head><body><article><h1>Saved Page</h1><p>Offline body</p></article></body></html>",
    );

    let output = Command::new(distill_bin())
        .args([
            path.to_str().expect("utf8 path"),
            "--dry-run",
            "--no-frontmatter",
        ])
        .output()
        .expect("run distill");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Saved Page"));
    assert!(stdout.contains("Offline body"));

    let _ = fs::remove_file(path);
}
