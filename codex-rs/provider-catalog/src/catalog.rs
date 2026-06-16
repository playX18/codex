use codex_protocol::openai_models::ModelsResponse;
use std::fs;
use std::io;
use std::path::PathBuf;

pub const PROVIDER_CATALOG_DIR: &str = "provider-catalog";

#[derive(Debug, Clone)]
pub struct ProviderCatalogStore {
    codex_home: PathBuf,
}

impl ProviderCatalogStore {
    pub fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    pub fn catalog_path(&self, provider_id: &str) -> PathBuf {
        self.codex_home
            .join(PROVIDER_CATALOG_DIR)
            .join(format!("{provider_id}.json"))
    }

    pub fn load(&self, provider_id: &str) -> io::Result<Option<ModelsResponse>> {
        let path = self.catalog_path(provider_id);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(Some(serde_json::from_str(&contents).map_err(|err| {
            io::Error::new(io::ErrorKind::InvalidData, err.to_string())
        })?))
    }

    pub fn save(&self, provider_id: &str, catalog: &ModelsResponse) -> io::Result<PathBuf> {
        let path = self.catalog_path(provider_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &path,
            serde_json::to_vec_pretty(catalog)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?,
        )?;
        Ok(path)
    }
}
