use codex_model_provider::ProviderRuntimeContext;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_dev::ModelsDevProvider;
use codex_provider_catalog::ProviderAuthStore;
use codex_provider_catalog::map_provider_to_model_provider_info;
use codex_provider_catalog::write_provider_catalog;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::TempDir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn provider_switch_preserves_chatgpt_auth_json() {
    let temp = TempDir::new().expect("temp");
    let codex_home = temp.path();
    std::fs::write(
        codex_home.join("auth.json"),
        r#"{"OPENAI_API_KEY":null,"tokens":{"access_token":"chatgpt-token"}}"#,
    )
    .expect("write auth");

    let mut auth = ProviderAuthStore::default();
    auth.set_api_key("anthropic", "sk-test".to_string());
    auth.save_to(codex_home).expect("save provider auth");

    let chatgpt_auth = std::fs::read_to_string(codex_home.join("auth.json")).expect("read auth");
    assert!(chatgpt_auth.contains("chatgpt-token"));
    assert!(!chatgpt_auth.contains("sk-test"));
}

#[test]
fn provider_catalog_written_offline() {
    let temp = TempDir::new().expect("temp");
    let provider = ModelsDevProvider {
        id: "deepseek".to_string(),
        name: "DeepSeek".to_string(),
        env: vec!["DEEPSEEK_API_KEY".to_string()],
        api: Some("https://api.deepseek.com/v1".to_string()),
        npm: None,
        models: HashMap::new(),
    };
    let path = write_provider_catalog(temp.path(), "deepseek", &provider).expect("write catalog");
    assert!(path.exists());
    let info = map_provider_to_model_provider_info(&provider);
    assert_eq!(info.upstream_wire_api.to_string(), "chat_completions");
}

#[tokio::test]
async fn provider_auth_store_supplies_api_key_when_env_unset() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let env_key = "CODEX_PROVIDER_CATALOG_TEST_KEY";
    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::remove_var(env_key);
    }

    let temp = TempDir::new().expect("temp");
    let codex_home = temp.path();
    let provider_id = "xiaomi-token-plan-sgp";

    let mut auth = ProviderAuthStore::default();
    auth.set_api_key(provider_id, "stored-provider-key".to_string());
    auth.save_to(codex_home).expect("save provider auth");

    let provider = ModelProviderInfo {
        env_key: Some(env_key.to_string()),
        ..map_provider_to_model_provider_info(&ModelsDevProvider {
            id: provider_id.to_string(),
            name: "Xiaomi".to_string(),
            env: vec![env_key.to_string()],
            api: Some("https://example.test/v1".to_string()),
            npm: None,
            models: HashMap::new(),
        })
    };

    let runtime_provider = create_model_provider(
        provider,
        /*auth_manager*/ None,
        Some(ProviderRuntimeContext::new(provider_id, codex_home)),
    );
    let api_auth = runtime_provider
        .api_auth()
        .await
        .expect("provider auth should resolve");
    let headers = api_auth.to_auth_headers();
    assert_eq!(
        headers
            .get("authorization")
            .map(|value| value.to_str().ok()),
        Some(Some("Bearer stored-provider-key"))
    );
}

#[tokio::test]
async fn provider_auth_store_prefers_env_var_over_stored_key() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let env_key = "CODEX_PROVIDER_CATALOG_TEST_KEY";
    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::set_var(env_key, "env-provider-key");
    }

    let temp = TempDir::new().expect("temp");
    let codex_home = temp.path();
    let provider_id = "xiaomi-token-plan-sgp";

    let mut auth = ProviderAuthStore::default();
    auth.set_api_key(provider_id, "stored-provider-key".to_string());
    auth.save_to(codex_home).expect("save provider auth");

    let provider = ModelProviderInfo {
        env_key: Some(env_key.to_string()),
        ..Default::default()
    };

    let runtime_provider = create_model_provider(
        provider,
        /*auth_manager*/ None,
        Some(ProviderRuntimeContext::new(provider_id, codex_home)),
    );
    let api_auth = runtime_provider
        .api_auth()
        .await
        .expect("provider auth should resolve");
    let headers = api_auth.to_auth_headers();
    assert_eq!(
        headers
            .get("authorization")
            .map(|value| value.to_str().ok()),
        Some(Some("Bearer env-provider-key"))
    );

    // SAFETY: serialized by ENV_LOCK.
    unsafe {
        std::env::remove_var(env_key);
    }
}
