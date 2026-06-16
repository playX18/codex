//! Provider picker for switching between ChatGPT and configured third-party providers.

use super::*;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_models_dev::MODELS_DEV_CACHE_FILE;
use codex_models_dev::ModelsDevCatalog;
use codex_provider_catalog::ProviderAuthStore;
use ratatui::text::Line;
use std::fs;
use std::io;

#[cfg(test)]
#[path = "provider_popups_tests.rs"]
mod tests;

impl ChatWidget {
    pub(crate) fn open_provider_popup(&mut self) {
        let codex_home = match codex_utils_home_dir::find_codex_home() {
            Ok(home) => home,
            Err(err) => {
                self.add_error_message(format!("Failed to locate Codex home: {err}"));
                return;
            }
        };

        let auth = match ProviderAuthStore::load_from(codex_home.as_path()) {
            Ok(auth) => auth,
            Err(err) => {
                self.add_error_message(format!("Failed to read provider-auth.json: {err}"));
                return;
            }
        };

        let active_provider_id = self.config.model_provider_id.clone();
        let mut items: Vec<SelectionItem> = Vec::new();
        let openai_auth = match openai_auth_status(codex_home.as_path()) {
            Ok(auth) => auth,
            Err(err) => {
                self.add_error_message(format!("Failed to read auth.json: {err}"));
                None
            }
        };
        let (openai_description, openai_search, openai_disabled) = match openai_auth {
            Some(status) => (
                format!("{OPENAI_PROVIDER_ID} · connected via {status}"),
                format!("ChatGPT OpenAI openai connected {status}"),
                false,
            ),
            None => (
                format!("{OPENAI_PROVIDER_ID} · not connected"),
                "ChatGPT OpenAI openai not connected unconnected login".to_string(),
                true,
            ),
        };

        items.push(SelectionItem {
            name: "ChatGPT (OpenAI)".to_string(),
            description: Some(openai_description),
            is_current: active_provider_id == OPENAI_PROVIDER_ID,
            is_disabled: openai_disabled,
            actions: Vec::new(),
            dismiss_on_select: true,
            search_value: Some(openai_search),
            ..Default::default()
        });

        let providers = load_models_dev_providers(codex_home.as_path());
        let mut provider_rows = match providers {
            Ok(providers) => providers,
            Err(err) => {
                self.add_error_message(format!("Failed to read models.dev cache: {err}"));
                Vec::new()
            }
        };
        if provider_rows.is_empty() {
            provider_rows = auth
                .entries
                .keys()
                .map(|provider_id| (provider_id.clone(), provider_id.clone()))
                .collect();
        }
        provider_rows.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));

        for (provider_id, provider_name) in provider_rows {
            let is_current = active_provider_id == provider_id;
            let is_connected = auth.entries.contains_key(&provider_id);
            let connection_search = if is_connected {
                "connected"
            } else {
                "not connected unconnected"
            };
            let actions: Vec<SelectionAction> = if is_connected {
                Vec::new()
            } else {
                let provider_id_for_action = provider_id.clone();
                let provider_name_for_action = provider_name.clone();
                vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenProviderApiKeyPrompt {
                        provider_id: provider_id_for_action.clone(),
                        provider_name: provider_name_for_action.clone(),
                    });
                })]
            };
            items.push(SelectionItem {
                name: provider_name.clone(),
                description: Some(if is_connected {
                    format!("{provider_id} · connected")
                } else {
                    format!("{provider_id} · not connected")
                }),
                is_current,
                is_disabled: is_connected,
                actions,
                dismiss_on_select: true,
                search_value: Some(format!("{provider_name} {provider_id} {connection_search}")),
                ..Default::default()
            });
        }

        if items.len() == 1 {
            items.push(SelectionItem {
                name: "Connect a provider".to_string(),
                description: Some(
                    "Fetch providers from models.dev and choose one to connect".to_string(),
                ),
                is_current: false,
                is_disabled: true,
                actions: Vec::new(),
                dismiss_on_select: false,
                search_value: Some(
                    "connect provider add provider login models.dev refresh".to_string(),
                ),
                ..Default::default()
            });
            items.push(SelectionItem {
                name: "Refresh provider catalog".to_string(),
                description: Some(
                    "Run `codexium providers refresh`, then open /provider again".to_string(),
                ),
                is_current: false,
                is_disabled: true,
                actions: Vec::new(),
                dismiss_on_select: false,
                ..Default::default()
            });
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from("Providers".bold()));
        header.push(Line::from(
            "Connect a provider, review credentials, or switch model providers.".dim(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            is_searchable: true,
            search_placeholder: Some("Type to search providers".to_string()),
            ..Default::default()
        });
    }

    pub(crate) fn open_provider_api_key_prompt(
        &mut self,
        provider_id: String,
        provider_name: String,
    ) {
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            format!("API key for {provider_name}"),
            "Paste API key and press Enter".to_string(),
            String::new(),
            Some(
                "The key is stored in provider-auth.json; ChatGPT auth.json is not modified."
                    .to_string(),
            ),
            Box::new(move |api_key: String| {
                tx.send(AppEvent::ProviderApiKeySubmitted {
                    provider_id: provider_id.clone(),
                    api_key,
                });
            }),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(super) fn third_party_provider_footer_label(&self) -> Option<String> {
        if self.config.model_provider_id == OPENAI_PROVIDER_ID {
            return None;
        }
        let name = self.config.model_provider.name.trim();
        if name.is_empty() {
            Some(self.config.model_provider_id.clone())
        } else {
            Some(name.to_string())
        }
    }

    pub(crate) fn update_provider_indicator(&mut self) {
        let label = self.third_party_provider_footer_label();
        self.bottom_pane.set_third_party_provider_label(label);
    }

    pub(crate) fn on_provider_switched(
        &mut self,
        provider_id: String,
        provider_info: ModelProviderInfo,
    ) {
        self.config.model_provider_id = provider_id;
        self.config.model_provider = provider_info;
        self.update_provider_indicator();
    }
}

fn load_models_dev_providers(
    codex_home: &std::path::Path,
) -> std::io::Result<Vec<(String, String)>> {
    let path = codex_home.join(MODELS_DEV_CACHE_FILE);
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let catalog: ModelsDevCatalog = serde_json::from_str(&contents)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    Ok(catalog
        .providers
        .into_iter()
        .map(|(provider_id, provider)| (provider_id, provider.name))
        .collect())
}

fn openai_auth_status(codex_home: &std::path::Path) -> io::Result<Option<&'static str>> {
    let auth_path = codex_home.join("auth.json");
    let contents = match fs::read_to_string(auth_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let value = serde_json::from_str::<serde_json::Value>(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    if value.get("tokens").is_some_and(|tokens| !tokens.is_null()) {
        return Ok(Some("oauth"));
    }
    if value
        .get("OPENAI_API_KEY")
        .and_then(|key| key.as_str())
        .is_some_and(|key| !key.is_empty())
    {
        return Ok(Some("api-key"));
    }
    Ok(None)
}
