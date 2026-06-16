use super::ContextualUserFragment;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CollaborationModeInstructions {
    instructions: String,
}

impl CollaborationModeInstructions {
    pub(crate) fn from_collaboration_mode_with_compose_catalog(
        collaboration_mode: &CollaborationMode,
        compose_skills_catalog: Option<&str>,
    ) -> Option<Self> {
        let instructions = collaboration_mode
            .settings
            .developer_instructions
            .as_ref()
            .filter(|instructions| !instructions.is_empty())?;

        let instructions = if collaboration_mode.mode == ModeKind::Compose {
            enrich_compose_instructions(instructions, compose_skills_catalog)
        } else {
            instructions.clone()
        };

        Some(Self { instructions })
    }
}

fn enrich_compose_instructions(base: &str, compose_skills_catalog: Option<&str>) -> String {
    let Some(catalog) = compose_skills_catalog.filter(|catalog| !catalog.is_empty()) else {
        return base.to_string();
    };
    if base.contains("<compose_skills>") {
        return base.to_string();
    }
    format!("{base}\n\n{catalog}")
}

impl ContextualUserFragment for CollaborationModeInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (COLLABORATION_MODE_OPEN_TAG, COLLABORATION_MODE_CLOSE_TAG)
    }

    fn body(&self) -> String {
        self.instructions.clone()
    }
}

#[cfg(test)]
#[path = "collaboration_mode_instructions_tests.rs"]
mod tests;
