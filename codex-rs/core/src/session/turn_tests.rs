use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnItemContributor;
use codex_protocol::AgentPath;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::TurnAbortReason;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

struct RewriteAgentMessageContributor;

struct NeverEndingRegularTask;

impl crate::tasks::SessionTask for NeverEndingRegularTask {
    fn kind(&self) -> crate::state::TaskKind {
        crate::state::TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.turn_test_never_ending"
    }

    async fn run(
        self: Arc<Self>,
        _session: Arc<crate::tasks::SessionTaskContext>,
        _ctx: Arc<TurnContext>,
        _input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        cancellation_token.cancelled().await;
        None
    }
}

impl TurnItemContributor for RewriteAgentMessageContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.content = vec![AgentMessageContent::Text {
                    text: "plan contributed assistant text".to_string(),
                }];
            }
            Ok(())
        })
    }
}

fn assistant_output_text(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some("msg-1".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        metadata: None,
    }
}

#[tokio::test]
async fn plan_mode_uses_contributed_turn_item_for_last_agent_message() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let mut state = PlanModeStreamState::new(&turn_context.sub_id);
    let mut last_agent_message = None;
    let item = assistant_output_text("original assistant text");

    let handled = handle_assistant_item_done_in_plan_mode(
        &session,
        &turn_context,
        &turn_store,
        &item,
        &mut state,
        /*previously_active_item*/ None,
        &mut last_agent_message,
    )
    .await;

    assert!(handled);
    assert_eq!(
        last_agent_message.as_deref(),
        Some("plan contributed assistant text")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_wait_reopens_mailbox_after_answer_boundary() {
    let (sess, tc, _rx) = crate::session::tests::make_session_and_context_with_rx().await;
    sess.spawn_task(Arc::clone(&tc), Vec::new(), NeverEndingRegularTask)
        .await;
    sess.input_queue
        .defer_mailbox_delivery_to_next_turn(&sess.active_turn, &tc.sub_id)
        .await;
    sess.input_queue
        .enqueue_mailbox_communication(InterAgentCommunication::new(
            AgentPath::try_from("/root/worker").expect("agent path should parse"),
            AgentPath::root(),
            Vec::new(),
            "worker completed".to_string(),
            /*trigger_turn*/ false,
        ))
        .await;

    let cancellation_token = CancellationToken::new();
    let resumed = wait_for_multi_agent_child_activity(&sess, &tc, &cancellation_token).await;

    assert!(resumed);
    assert_eq!(
        sess.input_queue.get_pending_input(&sess.active_turn).await,
        vec![TurnInput::InterAgentCommunication(
            InterAgentCommunication::new(
                AgentPath::try_from("/root/worker").expect("agent path should parse"),
                AgentPath::root(),
                Vec::new(),
                "worker completed".to_string(),
                /*trigger_turn*/ false,
            )
        )],
    );

    sess.abort_all_tasks(TurnAbortReason::Replaced).await;
}

#[test]
fn progress_continuation_detects_promised_work() {
    assert!(assistant_message_promises_continuation(
        "Now I have a thorough picture of all the changes. Let me organize and create the commits. I'll stage files in logical groups and commit each one."
    ));
    assert!(assistant_message_promises_continuation(
        "Continuing through the remaining changes: the status line metrics and token throughput tracking."
    ));
    assert!(assistant_message_promises_continuation(
        "Both conflicts resolved. Let me verify and finalize."
    ));
    assert!(assistant_message_promises_continuation(
        "No conflict markers remain. Let me mark the files as resolved and finalize."
    ));
}

#[test]
fn progress_continuation_ignores_final_or_optional_followup() {
    assert!(!assistant_message_promises_continuation(
        "I inspected the changes and found no issues."
    ));
    assert!(!assistant_message_promises_continuation(
        "If you want, I can stage these files next."
    ));
}

#[test]
fn length_auto_continue_item_has_user_role_and_content() {
    let item = length_auto_continue_item();
    match &item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "user");
            assert!(!content.is_empty());
            match &content[0] {
                ContentItem::InputText { text } => {
                    assert!(text.contains("Continue"));
                    assert!(text.contains("stopped"));
                }
                other => panic!("expected InputText, got {other:?}"),
            }
        }
        other => panic!("expected Message, got {other:?}"),
    }
}

#[test]
fn empty_output_nudge_item_has_user_role_and_content() {
    let item = empty_output_nudge_item();
    match &item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "user");
            assert!(!content.is_empty());
            match &content[0] {
                ContentItem::InputText { text } => {
                    assert!(text.contains("empty"));
                    assert!(text.contains("tool"));
                }
                other => panic!("expected InputText, got {other:?}"),
            }
        }
        other => panic!("expected Message, got {other:?}"),
    }
}
