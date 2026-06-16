//! Map models.dev providers to Codex `ModelProviderInfo` and `ModelInfo` catalogs.

mod auth;
mod catalog;
mod mapping;
#[cfg(test)]
#[path = "mapping_tests.rs"]
mod mapping_tests;

pub use auth::ProviderAuthEntry;
pub use auth::ProviderAuthStore;
pub use auth::resolve_env_key_api_key;
pub use catalog::PROVIDER_CATALOG_DIR;
pub use catalog::ProviderCatalogStore;
pub use mapping::build_models_response;
pub use mapping::infer_upstream_wire_api;
pub use mapping::map_model_to_model_info;
pub use mapping::map_provider_to_model_provider_info;
pub use mapping::provider_catalog_relative_path;
pub use mapping::write_provider_catalog;
