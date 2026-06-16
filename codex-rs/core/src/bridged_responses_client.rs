use std::sync::Arc;
use std::time::Instant;

use codex_api::ApiError;
use codex_api::Provider as ApiProvider;
use codex_api::RequestTelemetry;
use codex_api::ResponseStream;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesClient as ApiResponsesClient;
use codex_api::ResponsesOptions;
use codex_api::SharedAuthProvider;
use codex_api::SseTelemetry;
use codex_api::build_session_headers;
use codex_api::spawn_response_stream;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::RequestBody;
use codex_client::RequestCompression;
use codex_client::StreamResponse;
use codex_client::TransportError;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::UpstreamWireApi;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_wire_bridge::CodexChatHistoryStore;
use codex_wire_bridge::create_responses_sse_stream_from_chat_with_context;
use codex_wire_bridge::record_responses_sse_stream;
use codex_wire_bridge::responses_to_chat_completions_with_reasoning;
use codex_wire_bridge::transform::build_codex_tool_context_from_request;
use futures::StreamExt;
use http::HeaderValue;
use http::Method;

/// Responses client that transparently bridges to Chat Completions upstreams.
pub struct BridgedResponsesClient<T: HttpTransport> {
    inner: ApiResponsesClient<T>,
    provider_info: ModelProviderInfo,
    history: Arc<CodexChatHistoryStore>,
    transport: T,
    api_provider: ApiProvider,
    api_auth: SharedAuthProvider,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport + Clone> BridgedResponsesClient<T> {
    pub fn new(
        transport: T,
        provider_info: ModelProviderInfo,
        api_provider: ApiProvider,
        api_auth: SharedAuthProvider,
        history: Arc<CodexChatHistoryStore>,
    ) -> Self {
        let inner =
            ApiResponsesClient::new(transport.clone(), api_provider.clone(), api_auth.clone());
        Self {
            inner,
            provider_info,
            history,
            transport,
            api_provider,
            api_auth,
            request_telemetry: None,
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        mut self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        self.inner = self.inner.with_telemetry(request.clone(), sse.clone());
        self.request_telemetry = request;
        self.sse_telemetry = sse;
        self
    }

    pub async fn stream_request(
        &self,
        request: ResponsesApiRequest,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        if self.provider_info.upstream_wire_api == UpstreamWireApi::ChatCompletions {
            return self.stream_via_chat_bridge(request, options).await;
        }
        self.inner.stream_request(request, options).await
    }

    async fn stream_via_chat_bridge(
        &self,
        request: ResponsesApiRequest,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let mut body = serde_json::to_value(&request).map_err(|err| {
            ApiError::Stream(format!("failed to encode responses request: {err}"))
        })?;
        let _ = self.history.enrich_request(&mut body).await;
        let tool_context = build_codex_tool_context_from_request(&body);
        let reasoning = self.provider_info.codex_chat_reasoning.as_ref();
        let chat_body = responses_to_chat_completions_with_reasoning(body, reasoning)
            .map_err(|err| ApiError::Stream(err.to_string()))?;

        let ResponsesOptions {
            session_id,
            thread_id,
            session_source,
            extra_headers,
            compression: _,
            turn_state,
        } = options;

        let mut headers = extra_headers;
        if let Some(ref thread_id) = thread_id {
            insert_request_header(&mut headers, "x-client-request-id", thread_id);
        }
        headers.extend(build_session_headers(session_id, thread_id));
        if let Some(subagent) = subagent_header_value(&session_source) {
            insert_request_header(&mut headers, "x-openai-subagent", &subagent);
        }
        self.api_auth.add_auth_headers(&mut headers);
        headers.insert(
            http::header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        );
        headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let url = format!(
            "{}/chat/completions",
            self.api_provider.base_url.trim_end_matches('/')
        );
        let start = Instant::now();
        let stream_result = self
            .transport
            .stream(Request {
                method: Method::POST,
                url,
                headers,
                body: Some(RequestBody::Json(chat_body)),
                compression: RequestCompression::None,
                timeout: None,
            })
            .await;
        if let Some(telemetry) = self.request_telemetry.as_ref() {
            let (status, err) = match &stream_result {
                Ok(resp) => (Some(resp.status), None),
                Err(err) => (transport_http_status(err), Some(err)),
            };
            telemetry.on_request(0, status, err, start.elapsed());
        }
        let stream_response = stream_result.map_err(ApiError::Transport)?;

        let converted =
            create_responses_sse_stream_from_chat_with_context(stream_response.bytes, tool_context);
        let recorded = record_responses_sse_stream(converted, self.history.clone())
            .map(|chunk| chunk.map_err(|err| TransportError::Network(err.to_string())));
        let idle_timeout = self.api_provider.stream_idle_timeout;
        Ok(spawn_response_stream(
            StreamResponse {
                status: stream_response.status,
                headers: stream_response.headers,
                bytes: recorded.boxed(),
            },
            idle_timeout,
            self.sse_telemetry.clone(),
            turn_state,
        ))
    }
}

fn transport_http_status(err: &TransportError) -> Option<http::StatusCode> {
    match err {
        TransportError::Http { status, .. } => Some(*status),
        _ => None,
    }
}

fn insert_request_header(headers: &mut http::HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        name.parse::<http::HeaderName>(),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

fn subagent_header_value(source: &Option<SessionSource>) -> Option<String> {
    let SessionSource::SubAgent(sub) = source.as_ref()? else {
        return None;
    };
    match sub {
        SubAgentSource::Review => Some("review".to_string()),
        SubAgentSource::Compact => Some("compact".to_string()),
        SubAgentSource::MemoryConsolidation => Some("memory_consolidation".to_string()),
        SubAgentSource::ThreadSpawn { .. } => Some("collab_spawn".to_string()),
        SubAgentSource::Other(label) => Some(label.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_api::AuthProvider;
    use codex_api::ResponseEvent;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use futures::StreamExt;
    use http::HeaderMap;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio_util::bytes::Bytes;

    #[derive(Clone, Default)]
    struct NoAuth;

    impl AuthProvider for NoAuth {
        fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
    }

    #[derive(Clone)]
    struct FixtureTransport {
        body: String,
        requests: Arc<Mutex<Vec<Request>>>,
    }

    impl FixtureTransport {
        fn new(body: String) -> Self {
            Self {
                body,
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn requests(&self) -> Vec<Request> {
            self.requests
                .lock()
                .unwrap_or_else(|err| panic!("mutex poisoned: {err}"))
                .clone()
        }
    }

    impl HttpTransport for FixtureTransport {
        async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
            Err(TransportError::Build("execute should not run".to_string()))
        }

        async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
            self.requests
                .lock()
                .unwrap_or_else(|err| panic!("mutex poisoned: {err}"))
                .push(req);
            let stream = futures::stream::iter(vec![Ok::<Bytes, TransportError>(Bytes::from(
                self.body.clone(),
            ))]);
            Ok(StreamResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                bytes: Box::pin(stream),
            })
        }
    }

    fn api_provider() -> ApiProvider {
        ApiProvider {
            name: "test".to_string(),
            base_url: "https://example.com/v1".to_string(),
            query_params: None,
            headers: HeaderMap::new(),
            retry: codex_api::RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: false,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_millis(50),
        }
    }

    fn chat_sse_body(chunk: serde_json::Value) -> String {
        format!("data: {chunk}\n\ndata: [DONE]\n\n")
    }

    fn responses_options() -> ResponsesOptions {
        ResponsesOptions {
            session_id: None,
            thread_id: None,
            session_source: None,
            extra_headers: HeaderMap::new(),
            compression: codex_api::Compression::None,
            turn_state: None,
        }
    }

    #[tokio::test]
    async fn chat_bridge_restores_flattened_namespace_tool_calls() {
        let transport = FixtureTransport::new(chat_sse_body(json!({
            "id": "chatcmpl_1",
            "created": 123,
            "model": "mimo-v2.5",
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_spawn",
                        "type": "function",
                        "function": {
                            "name": "multi_agent_v1__spawn_agent",
                            "arguments": "{\"agent_type\":\"explorer\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })));
        let provider_info = ModelProviderInfo {
            upstream_wire_api: UpstreamWireApi::ChatCompletions,
            ..Default::default()
        };
        let client = BridgedResponsesClient::new(
            transport.clone(),
            provider_info,
            api_provider(),
            Arc::new(NoAuth),
            Arc::new(CodexChatHistoryStore::default()),
        );

        let request = ResponsesApiRequest {
            model: "mimo-v2.5".to_string(),
            instructions: String::new(),
            input: Vec::new(),
            tools: vec![json!({
                "type": "namespace",
                "name": "multi_agent_v1",
                "description": "Tools for spawning and managing sub-agents.",
                "tools": [{
                    "type": "function",
                    "name": "spawn_agent",
                    "description": "Spawn a sub-agent.",
                    "parameters": {"type": "object"}
                }]
            })],
            tool_choice: "auto".to_string(),
            parallel_tool_calls: true,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: None,
            text: None,
            client_metadata: None,
        };

        let mut stream = client
            .stream_request(request, responses_options())
            .await
            .expect("bridge stream should start");
        let mut function_call = None;
        while let Some(event) = stream.next().await {
            if let ResponseEvent::OutputItemDone(item) = event.expect("event should parse") {
                function_call = Some(item);
                break;
            }
        }

        assert_eq!(
            function_call,
            Some(ResponseItem::FunctionCall {
                id: Some("fc_call_spawn".to_string()),
                name: "spawn_agent".to_string(),
                namespace: Some("multi_agent_v1".to_string()),
                arguments: "{\"agent_type\":\"explorer\"}".to_string(),
                call_id: "call_spawn".to_string(),
                metadata: None,
            })
        );

        let sent_requests = transport.requests();
        assert_eq!(sent_requests.len(), 1);
    }

    fn codex_new_home() -> PathBuf {
        std::env::var_os("CODEX_LIVE_TEST_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex-new")))
            .expect("set CODEX_LIVE_TEST_HOME or HOME")
    }

    fn provider_info_from_codex_home(
        codex_home: &Path,
        provider_id: &str,
    ) -> anyhow::Result<ModelProviderInfo> {
        let config_text = std::fs::read_to_string(codex_home.join("config.toml"))?;
        let config_toml: toml::Value = toml::from_str(&config_text)?;
        let provider_toml = config_toml
            .get("model_providers")
            .and_then(toml::Value::as_table)
            .and_then(|providers| providers.get(provider_id))
            .unwrap_or_else(|| panic!("missing model_providers.{provider_id} in config.toml"));
        let mut provider_info: ModelProviderInfo = provider_toml.clone().try_into()?;
        provider_info.upstream_wire_api = UpstreamWireApi::ChatCompletions;
        Ok(provider_info)
    }

    fn live_bridge_client_from_codex_home(
        provider_id: &str,
    ) -> anyhow::Result<BridgedResponsesClient<codex_client::ReqwestTransport>> {
        let codex_home = codex_new_home();
        let provider_info = provider_info_from_codex_home(&codex_home, provider_id)?;
        let api_key = codex_model_provider::resolve_provider_env_key_auth(
            provider_id,
            &provider_info,
            &codex_home,
        )?
        .unwrap_or_else(|| {
            panic!("provider-auth.json missing API auth for provider {provider_id}")
        });
        let api_provider = provider_info.to_api_provider(/*auth_mode*/ None)?;

        Ok(BridgedResponsesClient::new(
            codex_client::ReqwestTransport::new(codex_login::default_client::build_reqwest_client()),
            provider_info,
            api_provider,
            Arc::new(codex_model_provider::BearerAuthProvider::new(api_key)),
            Arc::new(CodexChatHistoryStore::default()),
        ))
    }

    fn live_namespace_tool_request(model: &str) -> ResponsesApiRequest {
        ResponsesApiRequest {
            model: model.to_string(),
            instructions: "Use the provided tool. Do not answer in text.".to_string(),
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Call the spawn_agent tool with agent_type explorer.".to_string(),
                }],
                phase: None,
                metadata: None,
            }],
            tools: vec![json!({
                "type": "namespace",
                "name": "multi_agent_v1",
                "description": "Tools for spawning and managing sub-agents.",
                "tools": [{
                    "type": "function",
                    "name": "spawn_agent",
                    "description": "Spawn a sub-agent.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "agent_type": {"type": "string"},
                            "message": {"type": "string"}
                        },
                        "required": ["agent_type"]
                    }
                }]
            })],
            tool_choice: "required".to_string(),
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: None,
            text: None,
            client_metadata: None,
        }
    }

    async fn assert_live_bridge_restores_namespace_tool_call(
        provider_id: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        let client = live_bridge_client_from_codex_home(provider_id)?;
        let request = live_namespace_tool_request(model);
        let mut stream = client.stream_request(request, responses_options()).await?;
        let mut function_call = None;
        while let Some(event) = stream.next().await {
            if let ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                name,
                namespace,
                arguments,
                ..
            }) = event?
            {
                function_call = Some((name, namespace, arguments));
                break;
            }
        }

        let Some((name, namespace, arguments)) = function_call else {
            panic!("expected live bridge to return a function call");
        };
        assert_eq!(name, "spawn_agent");
        assert_eq!(namespace, Some("multi_agent_v1".to_string()));
        assert!(
            arguments.contains("explorer"),
            "expected tool arguments to include explorer role, got {arguments:?}"
        );
        Ok(())
    }

    struct LiveConversationTurn {
        text: String,
        end_turn: Option<bool>,
    }

    fn live_conversation_request(
        model: &str,
        transcript: &[(String, String)],
        user_prompt: &str,
    ) -> ResponsesApiRequest {
        let mut input = transcript
            .iter()
            .map(|(role, text)| {
                let content = if role == "assistant" {
                    vec![ContentItem::OutputText { text: text.clone() }]
                } else {
                    vec![ContentItem::InputText { text: text.clone() }]
                };
                ResponseItem::Message {
                    id: None,
                    role: role.clone(),
                    content,
                    phase: None,
                    metadata: None,
                }
            })
            .collect::<Vec<_>>();
        input.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: user_prompt.to_string(),
            }],
            phase: None,
            metadata: None,
        });

        ResponsesApiRequest {
            model: model.to_string(),
            instructions: "Reply exactly with the requested sentinel text. Do not use tools, markdown, or extra prose.".to_string(),
            input,
            tools: Vec::new(),
            tool_choice: "auto".to_string(),
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: None,
            text: None,
            client_metadata: None,
        }
    }

    async fn run_live_conversation_turn(
        client: &BridgedResponsesClient<codex_client::ReqwestTransport>,
        model: &str,
        transcript: &[(String, String)],
        user_prompt: &str,
    ) -> anyhow::Result<LiveConversationTurn> {
        let request = live_conversation_request(model, transcript, user_prompt);
        let mut stream = client.stream_request(request, responses_options()).await?;
        let mut text_deltas = String::new();
        let mut assistant_messages = Vec::new();
        let mut completed = None;

        loop {
            let event = tokio::time::timeout(Duration::from_secs(120), stream.next())
                .await
                .map_err(|_| anyhow::anyhow!("live stream timed out before completion"))?;
            let Some(event) = event else {
                break;
            };
            match event? {
                ResponseEvent::OutputTextDelta(delta) => text_deltas.push_str(&delta),
                ResponseEvent::OutputItemDone(ResponseItem::Message { role, content, .. })
                    if role == "assistant" =>
                {
                    let text = content
                        .into_iter()
                        .filter_map(|item| match item {
                            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                                Some(text)
                            }
                            ContentItem::InputImage { .. } => None,
                        })
                        .collect::<String>();
                    if !text.trim().is_empty() {
                        assistant_messages.push(text);
                    }
                }
                ResponseEvent::Completed { end_turn, .. } => {
                    completed = Some(end_turn);
                    break;
                }
                _ => {}
            }
        }

        let Some(end_turn) = completed else {
            anyhow::bail!("live stream ended before ResponseEvent::Completed");
        };

        let text = if assistant_messages.is_empty() {
            text_deltas
        } else {
            assistant_messages.join("\n")
        };
        if text.trim().is_empty() {
            anyhow::bail!("live stream completed without assistant text");
        }

        Ok(LiveConversationTurn { text, end_turn })
    }

    async fn assert_live_model_completes_three_turn_conversation(
        provider_id: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        let client = live_bridge_client_from_codex_home(provider_id)?;
        let turns = [
            (
                "Live stream completion check turn 1 of 3. Reply exactly: LIVE_TURN_1_READY",
                "LIVE_TURN_1_READY",
            ),
            (
                "Live stream completion check turn 2 of 3. Keep the conversation going and reply exactly: LIVE_TURN_2_STILL_RUNNING",
                "LIVE_TURN_2_STILL_RUNNING",
            ),
            (
                "Live stream completion check turn 3 of 3. Do not stop before the final sentinel. Reply exactly: LIVE_TURN_3_COMPLETE END_OF_LIVE_CONVERSATION_CHECK",
                "LIVE_TURN_3_COMPLETE END_OF_LIVE_CONVERSATION_CHECK",
            ),
        ];
        let mut transcript = Vec::new();

        for (prompt, expected) in turns {
            let turn = run_live_conversation_turn(&client, model, &transcript, prompt).await?;
            assert_ne!(
                turn.end_turn,
                Some(false),
                "{model} reported end_turn=false for expected completed turn"
            );
            assert!(
                turn.text.contains(expected),
                "{model} did not complete the expected sentinel {expected:?}; got {:?}",
                turn.text
            );
            transcript.push(("user".to_string(), prompt.to_string()));
            transcript.push(("assistant".to_string(), turn.text));
        }

        Ok(())
    }

    fn live_request_user_input_tool_request(model: &str) -> ResponsesApiRequest {
        ResponsesApiRequest {
            model: model.to_string(),
            instructions: "You must call the request_user_input tool. Do not answer in text."
                .to_string(),
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Ask me whether to continue the live test. Use request_user_input with one question whose id is continue_live_test.".to_string(),
                }],
                phase: None,
                metadata: None,
            }],
            tools: vec![json!({
                "type": "function",
                "name": "request_user_input",
                "description": "Ask the user to choose between options before continuing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "questions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": {"type": "string"},
                                    "header": {"type": "string"},
                                    "question": {"type": "string"},
                                    "options": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": {"type": "string"},
                                                "description": {"type": "string"}
                                            },
                                            "required": ["label", "description"],
                                            "additionalProperties": false
                                        }
                                    }
                                },
                                "required": ["id", "header", "question", "options"],
                                "additionalProperties": false
                            }
                        },
                        "autoResolutionMs": {"type": "number"}
                    },
                    "required": ["questions"],
                    "additionalProperties": false
                }
            })],
            tool_choice: "required".to_string(),
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: None,
            text: None,
            client_metadata: None,
        }
    }

    async fn assert_live_model_calls_request_user_input(
        provider_id: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        let client = live_bridge_client_from_codex_home(provider_id)?;
        let request = live_request_user_input_tool_request(model);
        let mut stream = client.stream_request(request, responses_options()).await?;
        let mut function_call = None;
        let mut assistant_text = String::new();
        let mut completed = false;

        loop {
            let event = tokio::time::timeout(Duration::from_secs(120), stream.next())
                .await
                .map_err(|_| anyhow::anyhow!("live stream timed out before tool call"))?;
            let Some(event) = event else {
                break;
            };
            match event? {
                ResponseEvent::OutputTextDelta(delta) => assistant_text.push_str(&delta),
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                    name,
                    namespace,
                    arguments,
                    ..
                }) => {
                    function_call = Some((name, namespace, arguments));
                    break;
                }
                ResponseEvent::Completed { .. } => {
                    completed = true;
                    break;
                }
                _ => {}
            }
        }

        let Some((name, namespace, arguments)) = function_call else {
            anyhow::bail!(
                "{model} did not call request_user_input; completed={completed}, assistant_text={assistant_text:?}"
            );
        };
        assert_eq!(name, "request_user_input");
        assert_eq!(namespace, None);

        let args: serde_json::Value = serde_json::from_str(&arguments).map_err(|err| {
            anyhow::anyhow!(
                "request_user_input arguments were invalid JSON: {err}; arguments={arguments:?}"
            )
        })?;
        let question_id = args
            .get("questions")
            .and_then(serde_json::Value::as_array)
            .and_then(|questions| questions.first())
            .and_then(|question| question.get("id"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(question_id, Some("continue_live_test"));
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_opencode_go_deepseek_v4_flash_restores_namespace_tool_calls_from_codex_new()
    -> anyhow::Result<()> {
        assert_live_bridge_restores_namespace_tool_call("opencode-go", "deepseek-v4-flash").await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_deepseek_deepseek_v4_flash_restores_namespace_tool_calls_from_codex_new()
    -> anyhow::Result<()> {
        assert_live_bridge_restores_namespace_tool_call("deepseek", "deepseek-v4-flash").await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_mimo_v2_5_completes_three_turn_conversation_from_codex_new() -> anyhow::Result<()>
    {
        assert_live_model_completes_three_turn_conversation("xiaomi-token-plan-sgp", "mimo-v2.5")
            .await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_mimo_v2_5_pro_completes_three_turn_conversation_from_codex_new()
    -> anyhow::Result<()> {
        assert_live_model_completes_three_turn_conversation(
            "xiaomi-token-plan-sgp",
            "mimo-v2.5-pro",
        )
        .await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_deepseek_v4_flash_completes_three_turn_conversation_from_codex_new()
    -> anyhow::Result<()> {
        assert_live_model_completes_three_turn_conversation("deepseek", "deepseek-v4-flash").await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_opencode_go_deepseek_v4_flash_completes_three_turn_conversation_from_codex_new()
    -> anyhow::Result<()> {
        assert_live_model_completes_three_turn_conversation("opencode-go", "deepseek-v4-flash")
            .await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_mimo_v2_5_calls_request_user_input_from_codex_new() -> anyhow::Result<()> {
        assert_live_model_calls_request_user_input("xiaomi-token-plan-sgp", "mimo-v2.5").await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_mimo_v2_5_pro_calls_request_user_input_from_codex_new() -> anyhow::Result<()> {
        assert_live_model_calls_request_user_input("xiaomi-token-plan-sgp", "mimo-v2.5-pro").await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_deepseek_v4_flash_calls_request_user_input_from_codex_new() -> anyhow::Result<()>
    {
        assert_live_model_calls_request_user_input("deepseek", "deepseek-v4-flash").await
    }

    #[tokio::test]
    #[ignore = "requires ~/.codex-new provider config, provider-auth.json, and network"]
    async fn live_opencode_go_deepseek_v4_flash_calls_request_user_input_from_codex_new()
    -> anyhow::Result<()> {
        assert_live_model_calls_request_user_input("opencode-go", "deepseek-v4-flash").await
    }
}
