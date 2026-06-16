use codex_models_manager::collaboration_mode_presets::builtin_collaboration_mode_presets;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;

use crate::model_catalog::ModelCatalog;

fn filtered_presets(_model_catalog: &ModelCatalog) -> Vec<CollaborationModeMask> {
    builtin_collaboration_mode_presets()
        .into_iter()
        .filter(|mask| mask.mode.is_some_and(ModeKind::is_tui_visible))
        .collect()
}

pub(crate) fn default_mask(model_catalog: &ModelCatalog) -> Option<CollaborationModeMask> {
    let presets = filtered_presets(model_catalog);
    presets
        .iter()
        .find(|mask| mask.mode == Some(ModeKind::Default))
        .cloned()
        .or_else(|| presets.into_iter().next())
}

pub(crate) fn mask_for_kind(
    model_catalog: &ModelCatalog,
    kind: ModeKind,
) -> Option<CollaborationModeMask> {
    if !kind.is_tui_visible() {
        return None;
    }
    filtered_presets(model_catalog)
        .into_iter()
        .find(|mask| mask.mode == Some(kind))
}

/// Cycle to the next collaboration mode preset in list order.
pub(crate) fn next_mask(
    model_catalog: &ModelCatalog,
    current: Option<&CollaborationModeMask>,
) -> Option<CollaborationModeMask> {
    let presets = filtered_presets(model_catalog);
    if presets.is_empty() {
        return None;
    }
    let current_kind = current.and_then(|mask| mask.mode);
    let next_index = presets
        .iter()
        .position(|mask| mask.mode == current_kind)
        .map_or(0, |idx| (idx + 1) % presets.len());
    presets.get(next_index).cloned()
}

pub(crate) fn default_mode_mask(model_catalog: &ModelCatalog) -> Option<CollaborationModeMask> {
    mask_for_kind(model_catalog, ModeKind::Default)
}

pub(crate) fn plan_mask(model_catalog: &ModelCatalog) -> Option<CollaborationModeMask> {
    mask_for_kind(model_catalog, ModeKind::Plan)
}

pub(crate) fn compose_mask(model_catalog: &ModelCatalog) -> Option<CollaborationModeMask> {
    mask_for_kind(model_catalog, ModeKind::Compose)
}

/// Slugs offered when the user optionally sets Compose subagent model defaults.
pub(crate) const COMPOSE_MODEL_ALLOWLIST: &[&str] =
    &["gpt-5.5", "gpt-5.4-mini", "gpt-5.3-codex-spark"];

/// Optional user-chosen defaults for subagent `spawn_agent` calls in Compose mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComposeSubagentModelPrefs {
    pub model: String,
    pub reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
}

pub(crate) fn compose_subagent_model_pref_instructions(
    prefs: &ComposeSubagentModelPrefs,
) -> String {
    let reasoning = prefs
        .reasoning_effort
        .as_ref()
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| "default".to_string());
    format!(
        "## User subagent model preference (session)\n\n\
         The user chose optional defaults for subagents in this Compose session:\n\
         - model: {}\n\
         - reasoning_effort: {}\n\n\
         Use these as the baseline on every `spawn_agent` call. Still apply automated \
         upgrades when role heuristics require a higher tier (for example reviewers must be \
         ≥ the implementer's model and effort).",
        prefs.model, reasoning
    )
}
