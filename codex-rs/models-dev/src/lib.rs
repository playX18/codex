//! Fetch and cache the models.dev provider catalog.

mod cache;
#[cfg(test)]
#[path = "cache_tests.rs"]
mod cache_tests;
mod cost;
#[cfg(test)]
#[path = "cost_tests.rs"]
mod cost_tests;
mod lookup;
#[cfg(test)]
#[path = "lookup_tests.rs"]
mod lookup_tests;
mod schema;

pub use cache::BACKGROUND_REFRESH_INTERVAL;
pub use cache::DEFAULT_MODELS_DEV_URL;
pub use cache::MODELS_DEV_CACHE_FILE;
pub use cache::ModelsDevCache;
pub use cache::ModelsDevRefreshError;
pub use cost::TokenBillableUsage;
pub use cost::estimate_cost_usd;
pub use cost::format_cost_estimate_usd;
pub use cost::format_turn_cost_estimate_usd;
pub use lookup::lookup_model_cost;
pub use schema::ModelsDevCatalog;
pub use schema::ModelsDevModel;
pub use schema::ModelsDevModelCost;
pub use schema::ModelsDevModelLimit;
pub use schema::ModelsDevProvider;
