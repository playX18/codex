use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn spawn_agent_result_tells_model_to_wait_before_finalizing() {
    let result = SpawnAgentResult::WithNickname {
        task_name: "/root/explore".to_string(),
        nickname: Some("explorer".to_string()),
        next_action: SPAWN_AGENT_V2_NEXT_ACTION,
    };

    assert_eq!(
        serde_json::to_value(result).expect("spawn_agent result should serialize"),
        json!({
            "task_name": "/root/explore",
            "nickname": "explorer",
            "next_action": SPAWN_AGENT_V2_NEXT_ACTION,
        })
    );
}

#[test]
fn hidden_metadata_spawn_agent_result_keeps_wait_instruction() {
    let result = SpawnAgentResult::HiddenMetadata {
        task_name: "/root/explore".to_string(),
        next_action: SPAWN_AGENT_V2_NEXT_ACTION,
    };

    assert_eq!(
        serde_json::to_value(result).expect("spawn_agent result should serialize"),
        json!({
            "task_name": "/root/explore",
            "next_action": SPAWN_AGENT_V2_NEXT_ACTION,
        })
    );
}
