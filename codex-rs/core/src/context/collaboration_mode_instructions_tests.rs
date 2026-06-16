use super::*;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;

#[test]
fn compose_catalog_is_appended_to_compose_mode_instructions() {
    let mode = CollaborationMode {
        mode: ModeKind::Compose,
        settings: Settings {
            model: "gpt-5.5".to_string(),
            reasoning_effort: None,
            developer_instructions: Some("Compose body".to_string()),
        },
    };
    let catalog =
        "<compose_skills>\n  <skill>\n    <name>compose:ask</name>\n  </skill>\n</compose_skills>";

    let instructions = CollaborationModeInstructions::from_collaboration_mode_with_compose_catalog(
        &mode,
        Some(catalog),
    )
    .expect("compose instructions should render");

    assert!(instructions.body().contains("Compose body"));
    assert!(instructions.body().contains("<compose_skills>"));
}
