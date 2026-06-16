use codex_model_provider_info::CodexChatReasoningConfig;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::UpstreamWireApi;
use codex_model_provider_info::WireApi;
use codex_models_dev::ModelsDevModel;
use codex_models_dev::ModelsDevProvider;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::openai_models::default_input_modalities;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::catalog::PROVIDER_CATALOG_DIR;

pub fn infer_upstream_wire_api(base_url: Option<&str>) -> UpstreamWireApi {
    let Some(url) = base_url.map(str::to_ascii_lowercase) else {
        return UpstreamWireApi::ChatCompletions;
    };
    if url.contains("/responses") {
        return UpstreamWireApi::Responses;
    }
    UpstreamWireApi::ChatCompletions
}

pub fn map_provider_to_model_provider_info(provider: &ModelsDevProvider) -> ModelProviderInfo {
    let base_url = provider.api.clone();
    let env_key = provider.env.first().cloned();
    let upstream = infer_upstream_wire_api(base_url.as_deref());
    let codex_chat_reasoning = if upstream == UpstreamWireApi::ChatCompletions {
        Some(default_chat_reasoning_config(provider))
    } else {
        None
    };

    ModelProviderInfo {
        name: provider.name.clone(),
        base_url,
        env_key: env_key.clone(),
        env_key_instructions: env_key.map(|key| {
            format!(
                "Set the `{key}` environment variable with your {} API key.",
                provider.name
            )
        }),
        wire_api: WireApi::Responses,
        upstream_wire_api: upstream,
        codex_chat_reasoning,
        requires_openai_auth: false,
        ..Default::default()
    }
}

fn default_chat_reasoning_config(provider: &ModelsDevProvider) -> CodexChatReasoningConfig {
    let supports_reasoning = provider.models.values().any(|model| model.reasoning);
    CodexChatReasoningConfig {
        supports_thinking: None,
        supports_effort: supports_reasoning.then_some(true),
        thinking_param: None,
        effort_param: supports_reasoning.then_some("reasoning_effort".to_string()),
        effort_value_mode: supports_reasoning.then_some("openai".to_string()),
        output_format: None,
    }
}

pub fn map_model_to_model_info(model: &ModelsDevModel, priority: usize) -> ModelInfo {
    let context_window = i64::try_from(model.limit.context).unwrap_or(128_000);
    let reasoning_levels = if model.reasoning {
        [
            ReasoningEffort::Minimal,
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
        ]
        .into_iter()
        .map(|effort| ReasoningEffortPreset {
            description: effort.to_string(),
            effort,
        })
        .collect()
    } else {
        vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "medium".to_string(),
        }]
    };

    ModelInfo {
        slug: model.id.clone(),
        display_name: model.name.clone(),
        description: Some(model.name.clone()),
        default_reasoning_level: None,
        supported_reasoning_levels: reasoning_levels,
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 1000 + i32::try_from(priority).unwrap_or(0),
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        availability_nux: None,
        upgrade: None,
        base_instructions: "You are a helpful coding assistant.".to_string(),
        model_messages: None,
        supports_reasoning_summaries: model.reasoning,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        web_search_tool_type: WebSearchToolType::Text,
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_parallel_tool_calls: model.tool_call,
        supports_image_detail_original: false,
        context_window: Some(context_window),
        max_context_window: Some(context_window),
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        auto_review_model_override: None,
        tool_mode: None,
        multi_agent_version: None,
    }
}

pub fn build_models_response(provider: &ModelsDevProvider) -> ModelsResponse {
    let mut models: Vec<_> = provider.models.values().collect();
    models.sort_by_key(|model| model.id.as_str());
    ModelsResponse {
        models: models
            .into_iter()
            .enumerate()
            .map(|(index, model)| map_model_to_model_info(model, index))
            .collect(),
    }
}

pub fn write_provider_catalog(
    codex_home: &Path,
    provider_id: &str,
    provider: &ModelsDevProvider,
) -> io::Result<PathBuf> {
    let dir = codex_home.join(PROVIDER_CATALOG_DIR);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{provider_id}.json"));
    let response = build_models_response(provider);
    fs::write(
        &path,
        serde_json::to_vec_pretty(&response)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?,
    )?;
    Ok(path)
}

pub fn provider_catalog_relative_path(provider_id: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(
        PathBuf::from(PROVIDER_CATALOG_DIR).join(format!("{provider_id}.json")),
    )
    .expect("relative catalog path should be valid")
}
