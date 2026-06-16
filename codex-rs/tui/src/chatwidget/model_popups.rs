//! Model, collaboration, and reasoning popups for `ChatWidget`.
//!
//! These surfaces are tightly related because changing one often redirects
//! into another, especially while Plan mode is active.

use super::*;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_models_manager::provider_catalog_manager::provider_scoped_cache_path;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_provider_catalog::ProviderAuthStore;
use codex_provider_catalog::ProviderCatalogStore;
use std::fs;

impl ChatWidget {
    /// Open a popup to choose a quick auto model. Selecting "All models"
    /// opens the full picker with every available preset.
    pub(crate) fn open_model_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Model selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let provider_models = self.connected_provider_model_items();
        if !provider_models.is_empty() {
            self.open_provider_models_popup(provider_models, /*provider_filter*/ None);
            return;
        }

        let presets: Vec<ModelPreset> = match self.model_catalog.try_list_models() {
            Ok(models) => models,
            Err(_) => {
                self.add_info_message(
                    "Models are being updated; please try /model again in a moment.".to_string(),
                    /*hint*/ None,
                );
                return;
            }
        };
        self.open_model_popup_with_presets(presets);
    }

    fn connected_provider_model_items(&self) -> Vec<ProviderModelPickerItem> {
        let mut items = Vec::new();
        let models = self
            .model_catalog
            .try_list_models()
            .expect("model catalog listing is infallible");
        let provider_id = self.config.model_provider_id.clone();
        let provider_name = if provider_id == OPENAI_PROVIDER_ID {
            "ChatGPT (OpenAI)".to_string()
        } else {
            let provider_name = self.config.model_provider.name.trim();
            if provider_name.is_empty() {
                provider_id.clone()
            } else {
                provider_name.to_string()
            }
        };
        items.extend(models.into_iter().map(|model| ProviderModelPickerItem {
            provider_id: provider_id.clone(),
            provider_name: provider_name.clone(),
            model,
        }));
        if provider_id != OPENAI_PROVIDER_ID {
            items.extend(openai_model_items_if_authenticated(
                self.config.codex_home.as_path(),
            ));
        }

        let Ok(auth) = ProviderAuthStore::load_from(self.config.codex_home.as_path()) else {
            return items;
        };
        let store = ProviderCatalogStore::new(self.config.codex_home.to_path_buf());
        let active_provider_id = self.config.model_provider_id.as_str();
        let mut provider_ids = auth.entries.keys().cloned().collect::<Vec<_>>();
        provider_ids.sort();
        for provider_id in provider_ids {
            if provider_id == active_provider_id {
                continue;
            }
            let Ok(Some(catalog)) = store.load(&provider_id) else {
                continue;
            };
            let provider_name = provider_id.clone();
            items.extend(catalog.models.into_iter().map(|model| {
                let model = ModelPreset::from(model);
                ProviderModelPickerItem {
                    provider_id: provider_id.clone(),
                    provider_name: provider_name.clone(),
                    model,
                }
            }));
        }

        items
    }

    pub(crate) fn open_provider_models_popup(
        &mut self,
        models: Vec<ProviderModelPickerItem>,
        provider_filter: Option<String>,
    ) {
        let visible_models = models
            .iter()
            .filter(|item| item.model.show_in_picker)
            .filter(|item| {
                provider_filter
                    .as_ref()
                    .is_none_or(|filter| item.provider_id == *filter)
            })
            .cloned()
            .collect::<Vec<_>>();
        if visible_models.is_empty() {
            self.add_info_message(
                "No models are available for this provider filter.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let mut provider_filters = models
            .iter()
            .map(|item| (item.provider_id.clone(), item.provider_name.clone()))
            .collect::<Vec<_>>();
        provider_filters.sort();
        provider_filters.dedup();

        let mut items = Vec::new();
        if provider_filters.len() > 1 {
            let all_models = models.clone();
            items.push(SelectionItem {
                name: "All connected providers".to_string(),
                description: Some("Show models from every connected provider".to_string()),
                is_current: provider_filter.is_none(),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenProviderModelsPopup {
                        models: all_models.clone(),
                        provider_filter: None,
                    });
                })],
                dismiss_on_select: true,
                search_value: Some("all connected providers".to_string()),
                ..Default::default()
            });
            for (provider_id, provider_name) in provider_filters {
                let all_models = models.clone();
                let provider_id_for_action = provider_id.clone();
                items.push(SelectionItem {
                    name: format!("Provider: {provider_name}"),
                    description: Some(provider_id.clone()),
                    is_current: provider_filter.as_deref() == Some(provider_id.as_str()),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenProviderModelsPopup {
                            models: all_models.clone(),
                            provider_filter: Some(provider_id_for_action.clone()),
                        });
                    })],
                    dismiss_on_select: true,
                    search_value: Some(format!("{provider_name} {provider_id}")),
                    ..Default::default()
                });
            }
        }

        let current_provider = self.config.model_provider_id.as_str();
        let current_model = self.current_model();
        for item in visible_models {
            let provider_id = item.provider_id.clone();
            let model = item.model.model.clone();
            let effort = Some(item.model.default_reasoning_effort.clone());
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::PersistProviderModelSelection {
                    provider_id: provider_id.clone(),
                    model: model.clone(),
                    effort: effort.clone(),
                });
            })];
            items.push(SelectionItem {
                name: item.model.model.clone(),
                description: Some(format!(
                    "{} · {}",
                    item.provider_name, item.model.description
                )),
                is_current: item.provider_id == current_provider
                    && item.model.model == current_model,
                is_default: item.model.is_default,
                actions,
                dismiss_on_select: true,
                search_value: Some(format!(
                    "{} {} {} {}",
                    item.model.model, item.model.description, item.provider_name, item.provider_id
                )),
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model",
            "Choose a model from any connected provider.",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(self.bottom_pane.standard_popup_hint_line()),
            items,
            header,
            is_searchable: true,
            search_placeholder: Some("Type to search models or providers".to_string()),
            ..Default::default()
        });
    }

    fn model_menu_header(&self, title: &str, subtitle: &str) -> Box<dyn Renderable> {
        let title = title.to_string();
        let subtitle = subtitle.to_string();
        let mut header = ColumnRenderable::new();
        header.push(Line::from(title.bold()));
        header.push(Line::from(subtitle.dim()));
        if let Some(warning) = self.model_menu_warning_line() {
            header.push(warning);
        }
        Box::new(header)
    }

    fn model_menu_warning_line(&self) -> Option<Line<'static>> {
        let base_url = self.custom_openai_base_url()?;
        let warning = format!(
            "Warning: OpenAI base URL is overridden to {base_url}. Selecting models may not be supported or work properly."
        );
        Some(Line::from(warning.red()))
    }

    fn custom_openai_base_url(&self) -> Option<String> {
        if !self.config.model_provider.is_openai() {
            return None;
        }

        let base_url = self.config.model_provider.base_url.as_ref()?;
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized = trimmed.trim_end_matches('/');
        if normalized == DEFAULT_OPENAI_BASE_URL {
            return None;
        }

        Some(trimmed.to_string())
    }

    pub(crate) fn open_model_popup_with_presets(&mut self, presets: Vec<ModelPreset>) {
        let presets: Vec<ModelPreset> = presets
            .into_iter()
            .filter(|preset| preset.show_in_picker)
            .collect();

        let current_model = self.current_model();
        let current_label = presets
            .iter()
            .find(|preset| preset.model.as_str() == current_model)
            .map(|preset| preset.model.to_string())
            .unwrap_or_else(|| self.model_display_name().to_string());

        let (mut auto_presets, other_presets): (Vec<ModelPreset>, Vec<ModelPreset>) = presets
            .into_iter()
            .partition(|preset| Self::is_auto_model(&preset.model));

        if auto_presets.is_empty() {
            self.open_all_models_popup(other_presets);
            return;
        }

        auto_presets.sort_by_key(|preset| Self::auto_model_order(&preset.model));
        let mut items: Vec<SelectionItem> = auto_presets
            .into_iter()
            .map(|preset| {
                let description =
                    (!preset.description.is_empty()).then_some(preset.description.clone());
                let model = preset.model.clone();
                let should_prompt_plan_mode_scope = self.should_prompt_plan_mode_reasoning_scope(
                    model.as_str(),
                    Some(preset.default_reasoning_effort.clone()),
                );
                let actions = Self::model_selection_actions(
                    model.clone(),
                    Some(preset.default_reasoning_effort.clone()),
                    should_prompt_plan_mode_scope,
                );
                SelectionItem {
                    name: model.clone(),
                    description,
                    is_current: model.as_str() == current_model,
                    is_default: preset.is_default,
                    actions,
                    dismiss_on_select: true,
                    search_value: Some(format!("{} {}", model, preset.description)),
                    ..Default::default()
                }
            })
            .collect();

        if !other_presets.is_empty() {
            let all_models = other_presets;
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenAllModelsPopup {
                    models: all_models.clone(),
                });
            })];

            let is_current = !items.iter().any(|item| item.is_current);
            let description = Some(format!(
                "Choose a specific model and reasoning level (current: {current_label})"
            ));

            items.push(SelectionItem {
                name: "All models".to_string(),
                description,
                is_current,
                actions,
                dismiss_on_select: true,
                search_value: Some(format!("all models {current_label}")),
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model",
            "Pick a quick auto mode or browse all models.",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header,
            is_searchable: true,
            search_placeholder: Some("Type to search models".to_string()),
            ..Default::default()
        });
    }

    fn is_auto_model(model: &str) -> bool {
        model.starts_with("codex-auto-")
    }

    fn auto_model_order(model: &str) -> usize {
        match model {
            "codex-auto-fast" => 0,
            "codex-auto-balanced" => 1,
            "codex-auto-thorough" => 2,
            _ => 3,
        }
    }

    pub(crate) fn open_all_models_popup(&mut self, presets: Vec<ModelPreset>) {
        if presets.is_empty() {
            self.add_info_message(
                "No additional models are available right now.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let mut items: Vec<SelectionItem> = Vec::new();
        for preset in presets.into_iter() {
            let description =
                (!preset.description.is_empty()).then_some(preset.description.to_string());
            let is_current = preset.model.as_str() == self.current_model();
            let single_supported_effort = preset.supported_reasoning_efforts.len() == 1;
            let preset_for_action = preset.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                let preset_for_event = preset_for_action.clone();
                tx.send(AppEvent::OpenReasoningPopup {
                    model: preset_for_event,
                });
            })];
            items.push(SelectionItem {
                name: preset.model.clone(),
                description,
                is_current,
                is_default: preset.is_default,
                actions,
                dismiss_on_select: single_supported_effort,
                dismiss_parent_on_child_accept: !single_supported_effort,
                search_value: Some(format!("{} {}", preset.model, preset.description)),
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model and Effort",
            "Access legacy models by running codexium -m <model_name> or in your config.toml",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(self.bottom_pane.standard_popup_hint_line()),
            items,
            header,
            is_searchable: true,
            search_placeholder: Some("Type to search models".to_string()),
            ..Default::default()
        });
    }

    fn model_selection_actions(
        model_for_action: String,
        effort_for_action: Option<ReasoningEffortConfig>,
        should_prompt_plan_mode_scope: bool,
    ) -> Vec<SelectionAction> {
        vec![Box::new(move |tx| {
            if should_prompt_plan_mode_scope {
                tx.send(AppEvent::OpenPlanReasoningScopePrompt {
                    model: model_for_action.clone(),
                    effort: effort_for_action.clone(),
                });
                return;
            }

            tx.send(AppEvent::UpdateModel(model_for_action.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(effort_for_action.clone()));
            tx.send(AppEvent::PersistModelSelection {
                model: model_for_action.clone(),
                effort: effort_for_action.clone(),
            });
        })]
    }

    fn should_prompt_plan_mode_reasoning_scope(
        &self,
        selected_model: &str,
        selected_effort: Option<ReasoningEffortConfig>,
    ) -> bool {
        if !self.collaboration_modes_enabled()
            || self.active_mode_kind() != ModeKind::Plan
            || selected_model != self.current_model()
        {
            return false;
        }

        // Prompt whenever the selection is not a true no-op for both:
        // 1) the active Plan-mode effective reasoning, and
        // 2) the stored global defaults that would be updated by the fallback path.
        selected_effort != self.effective_reasoning_effort()
            || selected_model != self.current_collaboration_mode.model()
            || selected_effort != self.current_collaboration_mode.reasoning_effort()
    }

    pub(crate) fn open_plan_reasoning_scope_prompt(
        &mut self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
    ) {
        let reasoning_phrase = match effort.as_ref() {
            Some(ReasoningEffortConfig::None) => "no reasoning".to_string(),
            Some(selected_effort) => {
                format!(
                    "{} reasoning",
                    Self::reasoning_effort_sentence_label(selected_effort)
                )
            }
            None => "the selected reasoning".to_string(),
        };
        let plan_only_description = format!("Always use {reasoning_phrase} in Plan mode.");
        let plan_reasoning_source = if let Some(plan_override) =
            self.config.plan_mode_reasoning_effort.as_ref()
        {
            format!(
                "user-chosen Plan override ({})",
                Self::reasoning_effort_sentence_label(plan_override)
            )
        } else if let Some(plan_mask) = collaboration_modes::plan_mask(self.model_catalog.as_ref())
        {
            match plan_mask
                .reasoning_effort
                .as_ref()
                .and_then(|effort| effort.as_ref())
            {
                Some(plan_effort) => format!(
                    "built-in Plan default ({})",
                    Self::reasoning_effort_sentence_label(plan_effort)
                ),
                None => "built-in Plan default (no reasoning)".to_string(),
            }
        } else {
            "built-in Plan default".to_string()
        };
        let all_modes_description = format!(
            "Set the global default reasoning level and the Plan mode override. This replaces the current {plan_reasoning_source}."
        );
        let subtitle = format!("Choose where to apply {reasoning_phrase}.");

        let plan_only_actions: Vec<SelectionAction> = vec![Box::new({
            let model = model.clone();
            let effort = effort.clone();
            move |tx| {
                tx.send(AppEvent::UpdateModel(model.clone()));
                tx.send(AppEvent::UpdatePlanModeReasoningEffort(effort.clone()));
                tx.send(AppEvent::PersistPlanModeReasoningEffort(effort.clone()));
            }
        })];
        let all_modes_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::UpdateModel(model.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(effort.clone()));
            tx.send(AppEvent::UpdatePlanModeReasoningEffort(effort.clone()));
            tx.send(AppEvent::PersistPlanModeReasoningEffort(effort.clone()));
            tx.send(AppEvent::PersistModelSelection {
                model: model.clone(),
                effort: effort.clone(),
            });
        })];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(PLAN_MODE_REASONING_SCOPE_TITLE.to_string()),
            subtitle: Some(subtitle),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![
                SelectionItem {
                    name: PLAN_MODE_REASONING_SCOPE_PLAN_ONLY.to_string(),
                    description: Some(plan_only_description),
                    actions: plan_only_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: PLAN_MODE_REASONING_SCOPE_ALL_MODES.to_string(),
                    description: Some(all_modes_description),
                    actions: all_modes_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        self.notify(Notification::PlanModePrompt {
            title: PLAN_MODE_REASONING_SCOPE_TITLE.to_string(),
        });
    }

    /// Open a popup to choose the reasoning effort (stage 2) for the given model.
    pub(crate) fn open_reasoning_popup(&mut self, preset: ModelPreset) {
        let default_effort = preset.default_reasoning_effort;
        let supported = preset.supported_reasoning_efforts;
        let in_plan_mode =
            self.collaboration_modes_enabled() && self.active_mode_kind() == ModeKind::Plan;

        let warn_effort = if supported
            .iter()
            .any(|option| option.effort == ReasoningEffortConfig::XHigh)
        {
            Some(ReasoningEffortConfig::XHigh)
        } else if supported
            .iter()
            .any(|option| option.effort == ReasoningEffortConfig::High)
        {
            Some(ReasoningEffortConfig::High)
        } else {
            None
        };
        let warning_text = warn_effort.as_ref().map(|effort| {
            let effort_label = Self::reasoning_effort_label(effort);
            format!("⚠ {effort_label} reasoning effort can quickly consume Plus plan rate limits.")
        });
        let warn_for_model = preset.model.starts_with("gpt-5.1-codex")
            || preset.model.starts_with("gpt-5.1-codex-max")
            || preset.model.starts_with("gpt-5.2");

        let mut choices: Vec<ReasoningEffortConfig> = supported
            .iter()
            .map(|option| option.effort.clone())
            .collect();
        if choices.is_empty() {
            choices.push(default_effort.clone());
        }

        if choices.len() == 1 {
            let selected_effort = choices.first().cloned();
            let selected_model = preset.model;
            if self
                .should_prompt_plan_mode_reasoning_scope(&selected_model, selected_effort.clone())
            {
                self.app_event_tx
                    .send(AppEvent::OpenPlanReasoningScopePrompt {
                        model: selected_model,
                        effort: selected_effort,
                    });
            } else {
                self.apply_model_and_effort(selected_model, selected_effort);
            }
            return;
        }

        let default_choice = choices
            .contains(&default_effort)
            .then(|| default_effort.clone())
            .or_else(|| choices.first().cloned())
            .or(Some(default_effort));

        let model_slug = preset.model.to_string();
        let is_current_model = self.current_model() == preset.model.as_str();
        let highlight_choice = if is_current_model {
            if in_plan_mode {
                self.config
                    .plan_mode_reasoning_effort
                    .clone()
                    .or_else(|| self.effective_reasoning_effort())
            } else {
                self.effective_reasoning_effort()
            }
        } else {
            default_choice.clone()
        };
        let selection_choice = highlight_choice.clone().or_else(|| default_choice.clone());
        let initial_selected_idx = choices
            .iter()
            .position(|choice| Some(choice) == selection_choice.as_ref());
        let mut items: Vec<SelectionItem> = Vec::new();
        for choice in choices.iter() {
            let effort = choice.clone();
            let mut effort_label = Self::reasoning_effort_label(&effort);
            if Some(choice) == default_choice.as_ref() {
                effort_label.push_str(" (default)");
            }

            let description = supported
                .iter()
                .find(|option| option.effort == effort)
                .map(|option| option.description.to_string())
                .filter(|text| !text.is_empty());

            let show_warning = warn_for_model && warn_effort.as_ref() == Some(&effort);
            let selected_description = if show_warning {
                warning_text.as_ref().map(|warning_message| {
                    description.as_ref().map_or_else(
                        || warning_message.clone(),
                        |d| format!("{d}\n{warning_message}"),
                    )
                })
            } else {
                None
            };

            let model_for_action = model_slug.clone();
            let choice_effort = Some(effort);
            let should_prompt_plan_mode_scope = self.should_prompt_plan_mode_reasoning_scope(
                model_slug.as_str(),
                choice_effort.clone(),
            );
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                if should_prompt_plan_mode_scope {
                    tx.send(AppEvent::OpenPlanReasoningScopePrompt {
                        model: model_for_action.clone(),
                        effort: choice_effort.clone(),
                    });
                } else {
                    tx.send(AppEvent::UpdateModel(model_for_action.clone()));
                    tx.send(AppEvent::UpdateReasoningEffort(choice_effort.clone()));
                    tx.send(AppEvent::PersistModelSelection {
                        model: model_for_action.clone(),
                        effort: choice_effort.clone(),
                    });
                }
            })];

            items.push(SelectionItem {
                name: effort_label,
                description,
                selected_description,
                is_current: is_current_model && Some(choice) == highlight_choice.as_ref(),
                actions,
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from(
            format!("Select Reasoning Level for {model_slug}").bold(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    pub(super) fn reasoning_effort_label(effort: &ReasoningEffortConfig) -> String {
        match effort {
            ReasoningEffortConfig::None => "None".to_string(),
            ReasoningEffortConfig::Minimal => "Minimal".to_string(),
            ReasoningEffortConfig::Low => "Low".to_string(),
            ReasoningEffortConfig::Medium => "Medium".to_string(),
            ReasoningEffortConfig::High => "High".to_string(),
            ReasoningEffortConfig::XHigh => "Extra high".to_string(),
            ReasoningEffortConfig::Custom(value) => value.clone(),
        }
    }

    pub(super) fn reasoning_effort_sentence_label(effort: &ReasoningEffortConfig) -> String {
        match effort {
            ReasoningEffortConfig::Custom(value) => value.clone(),
            effort => Self::reasoning_effort_label(effort).to_lowercase(),
        }
    }

    pub(super) fn apply_model_and_effort_without_persist(
        &self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
    ) {
        self.app_event_tx.send(AppEvent::UpdateModel(model));
        self.app_event_tx
            .send(AppEvent::UpdateReasoningEffort(effort));
    }

    fn apply_model_and_effort(&self, model: String, effort: Option<ReasoningEffortConfig>) {
        self.apply_model_and_effort_without_persist(model.clone(), effort.clone());
        self.app_event_tx
            .send(AppEvent::PersistModelSelection { model, effort });
    }

    /// Optional picker for Compose subagent model defaults (`/compose-models`).
    pub(crate) fn open_compose_subagent_model_picker(&mut self) {
        if self.active_mode_kind() != ModeKind::Compose {
            self.add_info_message(
                "/compose-models is available in Compose mode.".to_string(),
                Some("Use /compose or Shift+Tab to enter Compose mode.".to_string()),
            );
            return;
        }
        if !self.is_session_configured() {
            self.add_info_message(
                "Model selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let presets: Vec<ModelPreset> = match self.model_catalog.try_list_models() {
            Ok(models) => models,
            Err(_) => {
                self.add_info_message(
                    "Models are being updated; try /compose-models again in a moment.".to_string(),
                    /*hint*/ None,
                );
                return;
            }
        };

        let allowlist: HashSet<&str> = collaboration_modes::COMPOSE_MODEL_ALLOWLIST
            .iter()
            .copied()
            .collect();
        let filtered: Vec<ModelPreset> = presets
            .into_iter()
            .filter(|preset| preset.show_in_picker && allowlist.contains(preset.model.as_str()))
            .collect();

        let mut items: Vec<SelectionItem> = Vec::new();
        items.push(SelectionItem {
            name: "Automated (default)".to_string(),
            description: Some(
                "Orchestrator picks subagent models from task role and complexity.".to_string(),
            ),
            is_current: self.compose_subagent_model_prefs.is_none(),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::ClearComposeSubagentModelPrefs);
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        if filtered.is_empty() {
            self.add_info_message(
                "No Compose subagent models are available right now.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        for preset in filtered {
            let description =
                (!preset.description.is_empty()).then_some(preset.description.to_string());
            let is_current = self
                .compose_subagent_model_prefs
                .as_ref()
                .is_some_and(|prefs| prefs.model == preset.model);
            let effort = preset.default_reasoning_effort.clone();
            let model = preset.model.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::SetComposeSubagentModelPrefs {
                    model: model.clone(),
                    reasoning: Some(effort.clone()),
                });
            })];
            items.push(SelectionItem {
                name: preset.model.clone(),
                description,
                is_current,
                actions,
                dismiss_on_select: true,
                search_value: Some(format!("{} {}", preset.model, preset.description)),
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Subagent models",
            "Optional default for spawn_agent. Automated upgrades still apply for reviewers and complex roles.",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header,
            is_searchable: true,
            search_placeholder: Some("Type to search subagent models".to_string()),
            ..Default::default()
        });
    }
}

fn openai_model_items_if_authenticated(
    codex_home: &std::path::Path,
) -> Vec<ProviderModelPickerItem> {
    if !codex_home.join("auth.json").exists() {
        return Vec::new();
    }

    let models = load_openai_models(codex_home).unwrap_or_default();
    models
        .into_iter()
        .map(|model| ProviderModelPickerItem {
            provider_id: OPENAI_PROVIDER_ID.to_string(),
            provider_name: "ChatGPT (OpenAI)".to_string(),
            model: ModelPreset::from(model),
        })
        .collect()
}

fn load_openai_models(codex_home: &std::path::Path) -> Option<Vec<ModelInfo>> {
    let cache_path = provider_scoped_cache_path(&codex_home.to_path_buf(), OPENAI_PROVIDER_ID);
    if let Ok(contents) = fs::read_to_string(cache_path)
        && let Ok(catalog) = serde_json::from_str::<ModelsResponse>(&contents)
    {
        return Some(catalog.models);
    }

    codex_models_manager::bundled_models_response()
        .ok()
        .map(|catalog| catalog.models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn openai_model_items_require_auth_json() {
        let codex_home = TempDir::new().expect("tempdir");

        assert!(openai_model_items_if_authenticated(codex_home.path()).is_empty());
    }

    #[test]
    fn openai_model_items_read_provider_scoped_cache() {
        let codex_home = TempDir::new().expect("tempdir");
        fs::write(
            codex_home.path().join("auth.json"),
            r#"{"OPENAI_API_KEY":null,"tokens":{"access_token":"token"}}"#,
        )
        .expect("write auth");
        let cache_path =
            provider_scoped_cache_path(&codex_home.path().to_path_buf(), OPENAI_PROVIDER_ID);
        fs::write(
            cache_path,
            serde_json::to_string(&ModelsResponse {
                models: vec![codex_models_manager::model_info::model_info_from_slug(
                    "gpt-test",
                )],
            })
            .expect("serialize catalog"),
        )
        .expect("write cache");

        let items = openai_model_items_if_authenticated(codex_home.path());

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].provider_id, OPENAI_PROVIDER_ID);
        assert_eq!(items[0].model.model, "gpt-test");
    }
}
