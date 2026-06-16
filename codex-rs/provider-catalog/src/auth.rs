use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::EnvVarError;
use codex_protocol::error::Result as CodexResult;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub const PROVIDER_AUTH_FILE: &str = "provider-auth.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProviderAuthEntry {
    Api {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    Oauth {
        access: String,
        refresh: String,
        expires: i64,
        #[serde(flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProviderAuthStore {
    #[serde(flatten)]
    pub entries: HashMap<String, ProviderAuthEntry>,
}

impl ProviderAuthStore {
    pub fn load_from(codex_home: &Path) -> io::Result<Self> {
        let path = codex_home.join(PROVIDER_AUTH_FILE);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => return Err(err),
        };
        serde_json::from_str(&contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
    }

    pub fn save_to(&self, codex_home: &Path) -> io::Result<PathBuf> {
        let path = codex_home.join(PROVIDER_AUTH_FILE);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &path,
            serde_json::to_vec_pretty(self)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?,
        )?;
        Ok(path)
    }

    pub fn set_api_key(&mut self, provider_id: &str, key: String) {
        self.entries.insert(
            provider_id.to_string(),
            ProviderAuthEntry::Api {
                key,
                metadata: None,
            },
        );
    }

    pub fn remove(&mut self, provider_id: &str) -> Option<ProviderAuthEntry> {
        self.entries.remove(provider_id)
    }

    pub fn get(&self, provider_id: &str) -> Option<&ProviderAuthEntry> {
        self.entries.get(provider_id)
    }

    pub fn api_key_for_provider(&self, provider_id: &str) -> Option<String> {
        let ProviderAuthEntry::Api { key, .. } = self.get(provider_id)? else {
            return None;
        };
        (!key.trim().is_empty()).then(|| key.clone())
    }

    pub fn apply_env_for_provider(&self, provider_id: &str, env_key: &str) -> io::Result<()> {
        let Some(ProviderAuthEntry::Api { key, .. }) = self.get(provider_id) else {
            return Ok(());
        };
        // SAFETY: called during provider activation before threads spawn network requests.
        unsafe {
            std::env::set_var(env_key, key);
        }
        Ok(())
    }
}

/// Resolves an API key for providers configured with `env_key`.
///
/// Priority:
/// 1. Non-empty environment variable named by `env_key`
/// 2. API key stored in `provider-auth.json` for `provider_id`
pub fn resolve_env_key_api_key(
    provider_id: &str,
    provider: &ModelProviderInfo,
    auth_store: &ProviderAuthStore,
) -> CodexResult<Option<String>> {
    let Some(env_key) = provider.env_key.as_deref() else {
        return Ok(None);
    };

    if let Ok(value) = std::env::var(env_key)
        && !value.trim().is_empty()
    {
        return Ok(Some(value));
    }

    if let Some(key) = auth_store.api_key_for_provider(provider_id) {
        return Ok(Some(key));
    }

    Err(CodexErr::EnvVar(EnvVarError {
        var: env_key.to_string(),
        instructions: provider.env_key_instructions.clone(),
    }))
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
