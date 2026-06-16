use crate::schema::ModelsDevModelCost;
use crate::schema::ModelsDevProvider;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Looks up models.dev pricing for a provider/model pair from a cached catalog file.
pub fn lookup_model_cost(
    cache_path: &Path,
    provider_id: &str,
    model_id: &str,
) -> io::Result<Option<ModelsDevModelCost>> {
    let contents = match fs::read_to_string(cache_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let providers: HashMap<String, ModelsDevProvider> = serde_json::from_str(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;

    let Some(provider) = providers.get(provider_id) else {
        return Ok(None);
    };

    Ok(provider
        .models
        .get(model_id)
        .or_else(|| provider.models.values().find(|model| model.id == model_id))
        .and_then(|model| model.cost.clone()))
}
