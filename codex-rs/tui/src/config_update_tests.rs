use super::*;
use color_eyre::eyre::WrapErr;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn app_scoped_key_path_quotes_dotted_app_ids() {
    assert_eq!(
        app_scoped_key_path("plugin.linear", "enabled"),
        "apps.\"plugin.linear\".enabled"
    );
}

#[test]
fn trusted_project_edit_targets_project_trust_level() {
    assert_eq!(
        trusted_project_edit(Path::new("/workspace/team.project")),
        ConfigEdit {
            key_path: "projects.\"/workspace/team.project\".trust_level".to_string(),
            value: serde_json::json!("trusted"),
            merge_strategy: MergeStrategy::Replace,
        }
    );
}

#[test]
fn model_provider_switch_uses_absolute_catalog_path() {
    let edits = build_model_provider_switch_edits(
        Path::new("/home/user/.codex-new"),
        "xiaomi-token-plan-sgp",
    );

    assert_eq!(
        edits,
        vec![
            ConfigEdit {
                key_path: "model_provider".to_string(),
                value: serde_json::json!("xiaomi-token-plan-sgp"),
                merge_strategy: MergeStrategy::Replace,
            },
            ConfigEdit {
                key_path: "model_catalog_json".to_string(),
                value: serde_json::json!(
                    "/home/user/.codex-new/provider-catalog/xiaomi-token-plan-sgp.json"
                ),
                merge_strategy: MergeStrategy::Replace,
            },
        ]
    );
}

#[test]
fn model_provider_config_edit_removes_null_values() {
    let edit = build_model_provider_config_edit(
        "xiaomi-token-plan-sgp",
        serde_json::json!({
            "name": "Xiaomi Token Plan (Singapore)",
            "base_url": "https://token-plan-sgp.xiaomimimo.com/v1",
            "env_key": "XIAOMI_API_KEY",
            "auth": null,
            "codex_chat_reasoning": {
                "supports_thinking": true,
                "output_format": null
            },
            "query_params": [null, {"region": "sgp"}]
        }),
    );

    assert_eq!(
        edit,
        ConfigEdit {
            key_path: "model_providers.\"xiaomi-token-plan-sgp\"".to_string(),
            value: serde_json::json!({
                "name": "Xiaomi Token Plan (Singapore)",
                "base_url": "https://token-plan-sgp.xiaomimimo.com/v1",
                "env_key": "XIAOMI_API_KEY",
                "codex_chat_reasoning": {
                    "supports_thinking": true
                },
                "query_params": [{"region": "sgp"}]
            }),
            merge_strategy: MergeStrategy::Replace,
        }
    );
}

#[test]
fn format_config_error_preserves_server_validation_message() {
    let err = Err::<(), _>(color_eyre::eyre::eyre!(
        "config/batchWrite failed: Invalid configuration: features.fast_mode=true violates \
         managed requirements; allowed set [fast_mode=false]"
    ))
    .wrap_err("config/batchWrite failed in TUI")
    .unwrap_err();

    assert_eq!(
        format_config_error(&err),
        "config/batchWrite failed in TUI: config/batchWrite failed: Invalid configuration: \
         features.fast_mode=true violates managed requirements; allowed set [fast_mode=false]"
    );
}
