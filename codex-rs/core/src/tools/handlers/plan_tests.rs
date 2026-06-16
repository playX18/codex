use super::*;
use crate::session::tests::make_session_and_context;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolExecutor;
use crate::turn_diff_tracker::TurnDiffTracker;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_plan_tracks_unfinished_items() {
    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    let output = PlanHandler
        .handle(ToolInvocation {
            session: session.clone(),
            turn: turn.clone(),
            cancellation_token: CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
            call_id: "call-1".to_string(),
            tool_name: ToolName::plain("update_plan"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: json!({
                    "plan": [
                        {"step": "inspect", "status": "completed"},
                        {"step": "commit", "status": "pending"}
                    ]
                })
                .to_string(),
            },
        })
        .await
        .expect("pending plan should update");
    assert_eq!(output.log_preview(), PLAN_UPDATED_MESSAGE);
    assert_eq!(turn.unfinished_plan_items.load(Ordering::Relaxed), true);

    PlanHandler
        .handle(ToolInvocation {
            session,
            turn: turn.clone(),
            cancellation_token: CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
            call_id: "call-2".to_string(),
            tool_name: ToolName::plain("update_plan"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: json!({
                    "plan": [
                        {"step": "inspect", "status": "completed"},
                        {"step": "commit", "status": "completed"}
                    ]
                })
                .to_string(),
            },
        })
        .await
        .expect("completed plan should update");

    assert_eq!(turn.unfinished_plan_items.load(Ordering::Relaxed), false);
}
