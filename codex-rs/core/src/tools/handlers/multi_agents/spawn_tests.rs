use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn spawn_agent_result_tells_model_to_wait_before_finalizing() {
    let result = SpawnAgentResult {
        agent_id: "agent-1".to_string(),
        nickname: Some("explorer".to_string()),
        next_action: SPAWN_AGENT_V1_NEXT_ACTION,
    };

    assert_eq!(
        serde_json::to_value(result).expect("spawn_agent result should serialize"),
        json!({
            "agent_id": "agent-1",
            "nickname": "explorer",
            "next_action": SPAWN_AGENT_V1_NEXT_ACTION,
        })
    );
}
