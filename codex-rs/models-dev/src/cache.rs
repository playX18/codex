use crate::schema::ModelsDevCatalog;
use crate::schema::ModelsDevProvider;
use fs2::FileExt;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use thiserror::Error;
use tracing::info;
use tracing::warn;

pub const DEFAULT_MODELS_DEV_URL: &str = "https://models.dev";
pub const MODELS_DEV_CACHE_FILE: &str = "models-dev.json";
const FRESH_TTL: Duration = Duration::from_secs(5 * 60);
pub const BACKGROUND_REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Error)]
pub enum ModelsDevRefreshError {
    #[error("network fetch disabled")]
    FetchDisabled,
    #[error("failed to fetch models.dev: {0}")]
    FetchFailed(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// Disk-backed models.dev catalog with TTL and cross-process locking.
#[derive(Debug, Clone)]
pub struct ModelsDevCache {
    cache_path: PathBuf,
    source_url: String,
    http: Client,
}

impl ModelsDevCache {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            cache_path: codex_home.join(MODELS_DEV_CACHE_FILE),
            source_url: env::var("CODEX_MODELS_DEV_URL")
                .ok()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_MODELS_DEV_URL.to_string()),
            http: Client::new(),
        }
    }

    pub fn with_paths(cache_path: PathBuf, source_url: String) -> Self {
        Self {
            cache_path,
            source_url,
            http: Client::new(),
        }
    }

    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    pub fn source_url(&self) -> &str {
        &self.source_url
    }

    pub fn fetch_disabled() -> bool {
        env::var("CODEX_DISABLE_MODELS_DEV_FETCH")
            .ok()
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    }

    /// Always read disk first; fetch only on miss/stale/force.
    pub async fn get(&self, force_refresh: bool) -> io::Result<HashMap<String, ModelsDevProvider>> {
        if !force_refresh && let Some(catalog) = self.load_fresh_from_disk()? {
            return Ok(catalog.providers);
        }

        if Self::fetch_disabled() {
            if let Some(catalog) = self.load_from_disk()? {
                return Ok(catalog.providers);
            }
            return Ok(HashMap::new());
        }

        if let Err(err) = self.refresh(force_refresh).await {
            warn!("models.dev refresh failed: {err}");
            if let Some(catalog) = self.load_from_disk()? {
                return Ok(catalog.providers);
            }
        }

        Ok(self
            .load_from_disk()?
            .map(|catalog| catalog.providers)
            .unwrap_or_default())
    }

    pub async fn refresh(&self, force: bool) -> Result<(), ModelsDevRefreshError> {
        if !force && self.is_fresh_on_disk()? {
            return Ok(());
        }
        if Self::fetch_disabled() {
            return Err(ModelsDevRefreshError::FetchDisabled);
        }

        let text = self.fetch_remote().await?;
        self.write_with_lock(&text, force)?;
        info!(path = %self.cache_path.display(), "models.dev cache refreshed");
        Ok(())
    }

    pub fn spawn_background_refresh(&self) {
        if Self::fetch_disabled() {
            return;
        }
        let cache = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(BACKGROUND_REFRESH_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(err) = cache.refresh(/*force*/ false).await {
                    warn!("background models.dev refresh failed: {err}");
                }
            }
        });
    }

    pub(crate) fn is_fresh_on_disk(&self) -> io::Result<bool> {
        let metadata = match fs::metadata(&self.cache_path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(err),
        };
        let modified = metadata.modified()?;
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::MAX);
        Ok(age <= FRESH_TTL)
    }

    fn load_fresh_from_disk(&self) -> io::Result<Option<ModelsDevCatalog>> {
        if !self.is_fresh_on_disk()? {
            return Ok(None);
        }
        self.load_from_disk()
    }

    pub(crate) fn load_from_disk(&self) -> io::Result<Option<ModelsDevCatalog>> {
        let contents = match fs::read_to_string(&self.cache_path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };

        match serde_json::from_str::<HashMap<String, ModelsDevProvider>>(&contents) {
            Ok(providers) => Ok(Some(ModelsDevCatalog { providers })),
            Err(err) => {
                warn!(
                    path = %self.cache_path.display(),
                    "models.dev cache corrupt, removing: {err}"
                );
                let _ = fs::remove_file(&self.cache_path);
                Ok(None)
            }
        }
    }

    async fn fetch_remote(&self) -> Result<String, ModelsDevRefreshError> {
        let url = format!("{}/api.json", self.source_url.trim_end_matches('/'));
        let response = self
            .http
            .get(url)
            .header("User-Agent", "codex-models-dev")
            .send()
            .await
            .map_err(|err| ModelsDevRefreshError::FetchFailed(err.to_string()))?;
        if !response.status().is_success() {
            return Err(ModelsDevRefreshError::FetchFailed(format!(
                "status {}",
                response.status()
            )));
        }
        response
            .text()
            .await
            .map_err(|err| ModelsDevRefreshError::FetchFailed(err.to_string()))
    }

    #[cfg(test)]
    pub(crate) fn write_with_lock_for_test(
        &self,
        text: &str,
        force: bool,
    ) -> Result<(), ModelsDevRefreshError> {
        self.write_with_lock(text, force)
    }

    fn write_with_lock(&self, text: &str, force: bool) -> Result<(), ModelsDevRefreshError> {
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let lock_path = self.cache_path.with_extension("lock");
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)?;

        lock_file.lock_exclusive()?;

        if !force && self.is_fresh_on_disk()? {
            lock_file.unlock()?;
            return Ok(());
        }

        let tempfile = self.cache_path.with_extension(format!(
            "tmp.{}.{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        {
            let mut file = fs::File::create(&tempfile)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }

        if let Err(err) = fs::rename(&tempfile, &self.cache_path) {
            let _ = fs::remove_file(&tempfile);
            lock_file.unlock()?;
            return Err(ModelsDevRefreshError::Io(err));
        }

        lock_file.unlock()?;
        Ok(())
    }
}
