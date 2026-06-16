use codex_model_provider_info::UpstreamWireApi;
use codex_models_dev::ModelsDevModel;
use codex_models_dev::ModelsDevProvider;
use pretty_assertions::assert_eq;

use super::infer_upstream_wire_api;
use super::map_model_to_model_info;
use super::map_provider_to_model_provider_info;

fn sample_provider() -> ModelsDevProvider {
    ModelsDevProvider {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        env: vec!["ANTHROPIC_API_KEY".to_string()],
        api: Some("https://api.anthropic.com/v1".to_string()),
        npm: None,
        models: [(
            "claude-sonnet".to_string(),
            ModelsDevModel {
                id: "claude-sonnet".to_string(),
                name: "Claude Sonnet".to_string(),
                reasoning: true,
                tool_call: true,
                attachment: false,
                temperature: true,
                release_date: None,
                family: None,
                limit: codex_models_dev::ModelsDevModelLimit {
                    context: 200_000,
                    output: 8_192,
                    input: None,
                },
                status: None,
                cost: None,
            },
        )]
        .into(),
    }
}

#[test]
fn maps_provider_template() {
    let provider = sample_provider();
    let info = map_provider_to_model_provider_info(&provider);
    assert_eq!(info.name, "Anthropic");
    assert_eq!(info.env_key.as_deref(), Some("ANTHROPIC_API_KEY"));
    assert_eq!(info.upstream_wire_api, UpstreamWireApi::ChatCompletions);
    assert!(!info.requires_openai_auth);
    let reasoning = info
        .codex_chat_reasoning
        .expect("chat provider should have reasoning config");
    assert_eq!(reasoning.supports_thinking, None);
    assert_eq!(reasoning.supports_effort, Some(true));
    assert_eq!(reasoning.thinking_param, None);
    assert_eq!(reasoning.effort_param.as_deref(), Some("reasoning_effort"));
}

#[test]
fn maps_model_info_with_reasoning() {
    let provider = sample_provider();
    let model = provider.models.get("claude-sonnet").expect("model");
    let info = map_model_to_model_info(model, 0);
    assert_eq!(info.slug, "claude-sonnet");
    assert_eq!(info.context_window, Some(200_000));
    assert!(info.supports_reasoning_summaries);
}

#[test]
fn infers_responses_upstream_from_url() {
    assert_eq!(
        infer_upstream_wire_api(Some("https://example.test/v1/responses")),
        UpstreamWireApi::Responses
    );
}
