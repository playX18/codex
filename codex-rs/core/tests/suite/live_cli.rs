//! Optional smoke tests that hit the real OpenAI /v1/responses endpoint. They are `#[ignore]` by
//! default so CI stays deterministic and free. Developers can run them locally with
//! `just test -p codex-core --test all --run-ignored only live_cli` provided they set a valid
//! `OPENAI_API_KEY`.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde_json::Value;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;

fn require_api_key() -> String {
    std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY env var not set — skip running live tests")
}

/// Helper that spawns the binary inside a TempDir with minimal flags. Returns (Assert, TempDir).
fn run_live(prompt: &str) -> (assert_cmd::assert::Assert, TempDir) {
    #![expect(clippy::unwrap_used)]
    use std::io::Read;
    use std::io::Write;
    use std::thread;

    let dir = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let codex_home = home.path().join(".codex");
    std::fs::create_dir_all(&codex_home).unwrap();

    // Build a plain `std::process::Command` so we have full control over the underlying stdio
    // handles. `assert_cmd`’s own `Command` wrapper always forces stdout/stderr to be piped
    // internally which prevents us from streaming them live to the terminal (see its `spawn`
    // implementation). Instead we configure the std `Command` ourselves, then later hand the
    // resulting `Output` to `assert_cmd` for the familiar assertions.

    let mut cmd = Command::new(codex_utils_cargo_bin::cargo_bin("codexium").unwrap());
    cmd.current_dir(dir.path());
    cmd.env("OPENAI_API_KEY", require_api_key());
    cmd.env("HOME", home.path());
    cmd.env("CODEX_HOME", &codex_home);

    // We want three things at once:
    //   1. live streaming of the child’s stdout/stderr while the test is running
    //   2. captured output so we can keep using assert_cmd’s `Assert` helpers
    //   3. cross‑platform behavior (best effort)
    //
    // To get that we:
    //   • set both stdout and stderr to `piped()` so we can read them programmatically
    //   • spawn a thread for each stream that copies bytes into two sinks:
    //       – the parent process’ stdout/stderr for live visibility
    //       – an in‑memory buffer so we can pass it to `assert_cmd` later

    // Pass the prompt through the `--` separator so the CLI knows when user input ends.
    cmd.arg("--allow-no-git-exec")
        .arg("-v")
        .arg("--")
        .arg(prompt);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn codex-rs");

    // Send the terminating newline so Session::run exits after the first turn.
    child
        .stdin
        .as_mut()
        .expect("child stdin unavailable")
        .write_all(b"\n")
        .expect("failed to write to child stdin");

    // Helper that tees a ChildStdout/ChildStderr into both the parent’s stdio and a Vec<u8>.
    fn tee<R: Read + Send + 'static>(
        mut reader: R,
        mut writer: impl Write + Send + 'static,
    ) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        writer.write_all(&chunk[..n]).ok();
                        writer.flush().ok();
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    Err(_) => break,
                }
            }
            buf
        })
    }

    let stdout_handle = tee(
        child.stdout.take().expect("child stdout"),
        std::io::stdout(),
    );
    let stderr_handle = tee(
        child.stderr.take().expect("child stderr"),
        std::io::stderr(),
    );

    let status = child.wait().expect("failed to wait on child");
    let stdout = stdout_handle.join().expect("stdout thread panicked");
    let stderr = stderr_handle.join().expect("stderr thread panicked");

    let output = std::process::Output {
        status,
        stdout,
        stderr,
    };

    (output.assert(), dir)
}

#[ignore]
#[test]
fn live_create_file_hello_txt() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping live_create_file_hello_txt – OPENAI_API_KEY not set");
        return;
    }

    let (assert, dir) = run_live(
        "Use the shell tool with the apply_patch command to create a file named hello.txt containing the text 'hello'.",
    );

    assert.success();

    let path = dir.path().join("hello.txt");
    assert!(path.exists(), "hello.txt was not created by the model");

    let contents = std::fs::read_to_string(path).unwrap();

    assert_eq!(contents.trim(), "hello");
}

#[ignore]
#[test]
fn live_print_working_directory() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping live_print_working_directory – OPENAI_API_KEY not set");
        return;
    }

    let (assert, dir) = run_live("Print the current working directory using the shell function.");

    assert
        .success()
        .stdout(predicate::str::contains(dir.path().to_string_lossy()));
}

fn codex_new_home() -> std::path::PathBuf {
    std::env::var_os("CODEX_LIVE_TEST_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".codex-new"))
        })
        .expect("set CODEX_LIVE_TEST_HOME or HOME")
}

fn build_large_probe_repo() -> TempDir {
    let dir = TempDir::new().expect("temp repo");
    let source_repo = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(source_repo.status.success(), "git rev-parse should succeed");
    let source_repo =
        String::from_utf8(source_repo.stdout).expect("source repo path should be utf-8");
    let source_repo = source_repo.trim();

    Command::new("git")
        .args(["clone", "--no-hardlinks", "--local", source_repo])
        .arg(dir.path())
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir.path())
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir.path())
        .assert()
        .success();
    let readme_path = dir.path().join("README.md");
    let mut readme = std::fs::read_to_string(&readme_path).expect("README from cloned repo");
    readme.push_str("\n## Live Probe Result\n\nPending live inspection.\n");
    std::fs::write(readme_path, readme).expect("README update");
    dir
}

fn build_recent_changes_probe_repo() -> TempDir {
    let dir = TempDir::new().expect("temp repo");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir.path())
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir.path())
        .assert()
        .success();

    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(
        dir.path().join("README.md"),
        "# Recent Changes Probe\n\nInitial content.\n",
    )
    .expect("README");
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn value() -> i32 { 1 }\n",
    )
    .expect("lib.rs");
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-q", "-m", "chore: initial"])
        .current_dir(dir.path())
        .assert()
        .success();

    std::fs::write(
        dir.path().join("README.md"),
        "# Recent Changes Probe\n\nInitial content.\n\nDocument the changed return value.\n",
    )
    .expect("README update");
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn value() -> i32 { 2 }\n",
    )
    .expect("lib.rs update");
    dir
}

fn json_events(output: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn item_type(event: &Value) -> Option<&str> {
    event
        .get("item")
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
}

#[ignore]
#[test]
fn live_codex_new_inspect_recent_changes_and_commit() {
    let codex_home = codex_new_home();
    if !codex_home.join("provider-auth.json").exists() {
        eprintln!(
            "skipping live_codex_new_inspect_recent_changes_and_commit – provider-auth.json not found"
        );
        return;
    }

    let dir = build_recent_changes_probe_repo();
    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codexium").unwrap())
        .env("CODEX_HOME", codex_home)
        .args([
            "exec",
            "--json",
            "--cd",
            dir.path().to_str().expect("utf-8 temp path"),
            "--dangerously-bypass-approvals-and-sandbox",
            "-c",
            "model_provider=\"xiaomi-token-plan-sgp\"",
            "--model",
            "mimo-v2.5-pro",
            "inspect recent changes and commit in the form of `type(scope): desc`. Do not end the turn early. Do not provide a final answer until `git log -1 --pretty=%s` shows the new conventional commit subject.",
        ])
        .output()
        .expect("run codex exec");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    output.assert().success();
    let subject = Command::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .current_dir(dir.path())
        .output()
        .expect("git log");
    let subject = String::from_utf8(subject.stdout).expect("utf-8 git subject");
    let subject = subject.trim();
    assert_ne!(subject, "chore: initial");
    assert!(
        subject.contains('(')
            && subject.contains("): ")
            && subject
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase()),
        "commit subject did not match `type(scope): desc`: {subject}; stdout={stdout}"
    );
    Command::new("git")
        .args(["status", "--short"])
        .current_dir(dir.path())
        .assert()
        .stdout("");
}

#[ignore]
#[test]
fn live_codex_new_large_repo_commit_waits_for_subagents_before_final_answer() {
    let codex_home = codex_new_home();
    if !codex_home.join("provider-auth.json").exists() {
        eprintln!(
            "skipping live_codex_new_large_repo_commit_waits_for_subagents_before_final_answer – provider-auth.json not found"
        );
        return;
    }

    let dir = build_large_probe_repo();
    let prompt = concat!(
        "Inspect this large repository using subagents: spawn at least two subagents if the tool is available, ",
        "one to inspect src/ and one to inspect docs/tests/fixtures. Wait for their results. ",
        "Do not manually inspect the repo as a fallback before those two subagents complete. ",
        "Then append a README.md section named \"Live Probe Result\" with a concise inspection summary, ",
        "run git status, commit exactly README.md with message \"live probe update\", ",
        "and verify the commit exists with git log -1 --oneline. Do not end the turn early. ",
        "Do not provide a final answer until after the commit verification command succeeds."
    );

    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codexium").unwrap())
        .env("CODEX_HOME", codex_home)
        .args([
            "exec",
            "--json",
            "--cd",
            dir.path().to_str().expect("utf-8 temp path"),
            "--dangerously-bypass-approvals-and-sandbox",
            "--enable",
            "multi_agent_v2",
            "-c",
            "model_provider=\"xiaomi-token-plan-sgp\"",
            "-c",
            "features.multi_agent_v2.non_code_mode_only=false",
            "--model",
            "mimo-v2.5-pro",
            prompt,
        ])
        .output()
        .expect("run codex exec");

    let events = json_events(&output.stdout);
    let turn_completed_index = events
        .iter()
        .position(|event| event.get("type").and_then(Value::as_str) == Some("turn.completed"))
        .expect("turn should complete");
    let final_answer_index = events[..turn_completed_index]
        .iter()
        .rposition(|event| item_type(event) == Some("agent_message"))
        .expect("agent should produce a final answer before turn completion");
    let completed_subagents_before_final = events[..final_answer_index]
        .iter()
        .filter(|event| {
            event
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                == Some("collab_tool_call")
                && event.get("type").and_then(Value::as_str) == Some("item.completed")
        })
        .count();

    assert!(
        completed_subagents_before_final >= 2,
        "agent finalized before two subagents completed; stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );

    output.assert().success();
    Command::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .current_dir(dir.path())
        .assert()
        .stdout(predicate::str::contains("live probe update"));
    assert!(
        std::fs::read_to_string(dir.path().join("README.md"))
            .expect("README")
            .contains("Live Probe Result")
    );
}
