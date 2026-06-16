use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn openai_auth_status_reads_oauth_auth_json() {
    let codex_home = TempDir::new().expect("tempdir");
    fs::write(
        codex_home.path().join("auth.json"),
        r#"{"OPENAI_API_KEY":null,"tokens":{"access_token":"token"}}"#,
    )
    .expect("write auth");

    assert_eq!(
        openai_auth_status(codex_home.path()).expect("auth status"),
        Some("oauth")
    );
}

#[test]
fn openai_auth_status_reads_api_key_auth_json() {
    let codex_home = TempDir::new().expect("tempdir");
    fs::write(
        codex_home.path().join("auth.json"),
        r#"{"OPENAI_API_KEY":"sk-test","tokens":null}"#,
    )
    .expect("write auth");

    assert_eq!(
        openai_auth_status(codex_home.path()).expect("auth status"),
        Some("api-key")
    );
}

#[test]
fn openai_auth_status_requires_auth_json() {
    let codex_home = TempDir::new().expect("tempdir");

    assert_eq!(
        openai_auth_status(codex_home.path()).expect("auth status"),
        None
    );
}
