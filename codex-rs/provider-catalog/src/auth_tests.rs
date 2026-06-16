use super::*;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use tempfile::TempDir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn provider_with_env_key(env_key: &str) -> ModelProviderInfo {
    ModelProviderInfo {
        env_key: Some(env_key.to_string()),
        env_key_instructions: Some(format!("Set `{env_key}`.")),
        ..Default::default()
    }
}

#[test]
fn resolve_env_key_api_key_uses_provider_auth_when_env_unset() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let env_key = "CODEX_PROVIDER_AUTH_TEST_KEY";
    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::remove_var(env_key);
    }

    let temp = TempDir::new().expect("temp");
    let mut auth = ProviderAuthStore::default();
    auth.set_api_key("xiaomi-token-plan-sgp", "stored-key".to_string());
    auth.save_to(temp.path()).expect("save provider auth");

    let auth_store = ProviderAuthStore::load_from(temp.path()).expect("load provider auth");
    let provider = provider_with_env_key(env_key);

    let api_key = resolve_env_key_api_key("xiaomi-token-plan-sgp", &provider, &auth_store)
        .expect("api key should resolve");
    assert_eq!(api_key, Some("stored-key".to_string()));
}

#[test]
fn resolve_env_key_api_key_prefers_env_var_over_provider_auth() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let env_key = "CODEX_PROVIDER_AUTH_TEST_KEY";
    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::set_var(env_key, "env-key");
    }

    let temp = TempDir::new().expect("temp");
    let mut auth = ProviderAuthStore::default();
    auth.set_api_key("xiaomi-token-plan-sgp", "stored-key".to_string());
    auth.save_to(temp.path()).expect("save provider auth");

    let auth_store = ProviderAuthStore::load_from(temp.path()).expect("load provider auth");
    let provider = provider_with_env_key(env_key);

    let api_key = resolve_env_key_api_key("xiaomi-token-plan-sgp", &provider, &auth_store)
        .expect("api key should resolve");
    assert_eq!(api_key, Some("env-key".to_string()));

    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::remove_var(env_key);
    }
}

#[test]
fn resolve_env_key_api_key_returns_error_when_missing() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let env_key = "CODEX_PROVIDER_AUTH_TEST_KEY";
    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::remove_var(env_key);
    }

    let auth_store = ProviderAuthStore::default();
    let provider = provider_with_env_key(env_key);

    let err = resolve_env_key_api_key("xiaomi-token-plan-sgp", &provider, &auth_store)
        .expect_err("missing key should error");
    match err {
        CodexErr::EnvVar(env_err) => {
            assert_eq!(env_err.var, env_key);
            assert_eq!(env_err.instructions, Some(format!("Set `{env_key}`.")));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
