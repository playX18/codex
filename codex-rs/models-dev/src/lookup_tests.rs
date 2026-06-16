use crate::lookup::lookup_model_cost;
use crate::schema::ModelsDevModel;
use crate::schema::ModelsDevModelCost;
use crate::schema::ModelsDevModelLimit;
use crate::schema::ModelsDevProvider;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

fn sample_model(id: &str, cost: ModelsDevModelCost) -> ModelsDevModel {
    ModelsDevModel {
        id: id.to_string(),
        name: id.to_string(),
        reasoning: false,
        tool_call: true,
        attachment: false,
        temperature: true,
        release_date: None,
        family: None,
        limit: ModelsDevModelLimit {
            context: 128_000,
            output: 16_000,
            input: None,
        },
        status: None,
        cost: Some(cost),
    }
}

#[test]
fn lookup_model_cost_reads_cached_catalog() {
    let temp = TempDir::new().expect("tempdir");
    let cache_path = temp.path().join("models-dev.json");
    let providers = HashMap::from([(
        "anthropic".to_string(),
        ModelsDevProvider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            env: vec!["ANTHROPIC_API_KEY".to_string()],
            api: Some("https://api.anthropic.com/v1".to_string()),
            npm: None,
            models: HashMap::from([(
                "claude-opus-4-5".to_string(),
                sample_model(
                    "claude-opus-4-5",
                    ModelsDevModelCost {
                        input: 5.0,
                        output: 25.0,
                        cache_read: 0.5,
                        cache_write: 6.25,
                    },
                ),
            )]),
        },
    )]);
    fs::write(
        &cache_path,
        serde_json::to_string(&providers).expect("json"),
    )
    .expect("write");

    let cost = lookup_model_cost(&cache_path, "anthropic", "claude-opus-4-5")
        .expect("lookup")
        .expect("cost");
    assert_eq!(cost.input, 5.0);
    assert_eq!(cost.output, 25.0);
}

#[test]
fn lookup_model_cost_matches_model_id_when_map_key_differs() {
    let temp = TempDir::new().expect("tempdir");
    let cache_path = temp.path().join("models-dev.json");
    let providers = HashMap::from([(
        "openrouter".to_string(),
        ModelsDevProvider {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            env: vec!["OPENROUTER_API_KEY".to_string()],
            api: Some("https://openrouter.ai/api/v1".to_string()),
            npm: None,
            models: HashMap::from([(
                "vendor/model-slug".to_string(),
                sample_model(
                    "actual-model-id",
                    ModelsDevModelCost {
                        input: 1.0,
                        output: 2.0,
                        cache_read: 0.0,
                        cache_write: 0.0,
                    },
                ),
            )]),
        },
    )]);
    fs::write(
        &cache_path,
        serde_json::to_string(&providers).expect("json"),
    )
    .expect("write");

    let cost = lookup_model_cost(&cache_path, "openrouter", "actual-model-id")
        .expect("lookup")
        .expect("cost");
    assert_eq!(cost.input, 1.0);
}

#[test]
fn lookup_model_cost_returns_none_when_missing() {
    let temp = TempDir::new().expect("tempdir");
    let cache_path = temp.path().join("models-dev.json");
    assert_eq!(
        lookup_model_cost(&cache_path, "anthropic", "missing").expect("lookup"),
        None
    );
}
