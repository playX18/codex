use super::ModelsDevCatalog;
use super::ModelsDevModelCost;
use crate::MODELS_DEV_CACHE_FILE;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn models_dev_cost_accepts_object_values() {
    let catalog: ModelsDevCatalog = serde_json::from_value(serde_json::json!({
        "example": {
            "id": "example",
            "name": "Example",
            "models": {
                "example-model": {
                    "id": "example-model",
                    "name": "Example Model",
                    "release_date": null,
                    "family": null,
                    "limit": {
                        "context": 1000,
                        "output": 100
                    },
                    "status": null,
                    "cost": {
                        "input": {
                            "usd": 1.25
                        },
                        "output": 2.5,
                        "cache_read": {
                            "prompt": 0.125
                        }
                    }
                }
            }
        }
    }))
    .expect("models.dev catalog should tolerate object-shaped costs");

    let cost = catalog.providers["example"].models["example-model"]
        .cost
        .clone();
    assert_eq!(
        cost,
        Some(ModelsDevModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.125,
            cache_write: 0.0,
        })
    );
}

#[test]
#[ignore]
fn parses_models_dev_cache_from_codex_home() {
    let codex_home = std::env::var_os("CODEX_LIVE_TEST_CODEX_HOME")
        .map(PathBuf::from)
        .expect("CODEX_LIVE_TEST_CODEX_HOME must point at a Codex home");
    let contents = std::fs::read_to_string(codex_home.join(MODELS_DEV_CACHE_FILE))
        .expect("models.dev cache should be readable");
    let catalog: ModelsDevCatalog =
        serde_json::from_str(&contents).expect("models.dev cache should parse");

    assert!(
        !catalog.providers.is_empty(),
        "models.dev cache should include providers"
    );
}
