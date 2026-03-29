use std::{
  convert::Infallible,
  net::{SocketAddr, TcpListener},
  path::PathBuf,
  sync::Arc,
  thread,
  time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use axum::{
  extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    State,
  },
  http::{
    header::{CONTENT_TYPE, HeaderValue},
    StatusCode,
  },
  response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
  },
  routing::{get, post},
  Json, Router,
};
use base64::Engine;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{sync::mpsc, time::sleep};
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{
  connect_async,
  tungstenite::{client::IntoClientRequest, protocol::Message as ClientWsMessage},
};
use tower_http::services::ServeDir;

use crate::{
  app_log,
  config::{AppSettings as RuntimeSettings, AsrSettings, LlmSettings, TtsSettings},
  speech::{AsrEvent, SpeechRuntime},
};

#[derive(Clone)]
struct AppState {
  client: reqwest::Client,
  speech: Arc<SpeechRuntime>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
  pub role: String,
  pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatRequest {
  pub messages: Vec<ChatMessage>,
  pub settings: RuntimeSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsRequest {
  pub text: String,
  pub settings: TtsSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FrontendLogRequest {
  pub level: String,
  pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeechSocketMessage {
  #[serde(rename = "type")]
  pub kind: String,
  #[serde(default)]
  pub asr: Option<AsrSettings>,
  #[serde(default)]
  pub pcm: String,
}

#[derive(Debug)]
enum StreamEvent {
  Token(String),
  Error(String),
  Done,
}

#[derive(Default)]
struct ThinkingFilter {
  in_thinking: bool,
  tag_buffer: String,
}

impl ThinkingFilter {
  fn push(&mut self, chunk: &str) -> String {
    const OPEN_TAG: &str = "<think>";
    const CLOSE_TAG: &str = "</think>";

    let mut visible = String::new();

    for ch in chunk.chars() {
      if self.tag_buffer.is_empty() {
        if ch == '<' {
          self.tag_buffer.push(ch);
          continue;
        }

        if !self.in_thinking {
          visible.push(ch);
        }
        continue;
      }

      self.tag_buffer.push(ch);
      let target = if self.in_thinking { CLOSE_TAG } else { OPEN_TAG };

      if target.starts_with(&self.tag_buffer) {
        if self.tag_buffer == target {
          self.in_thinking = !self.in_thinking;
          self.tag_buffer.clear();
        }
        continue;
      }

      if self.in_thinking {
        self.tag_buffer.clear();
      } else {
        visible.push_str(&self.tag_buffer);
        self.tag_buffer.clear();
      }
    }

    visible
  }

  fn finish(&mut self) -> Option<String> {
    if self.in_thinking || self.tag_buffer.is_empty() {
      self.tag_buffer.clear();
      return None;
    }

    let leftover = self.tag_buffer.clone();
    self.tag_buffer.clear();
    Some(leftover)
  }
}

struct PromptEchoFilter {
  candidates: Vec<String>,
  buffer: String,
  decided: bool,
}

impl PromptEchoFilter {
  fn new(system_prompt: &str) -> Self {
    Self {
      candidates: build_prompt_echo_candidates(system_prompt),
      buffer: String::new(),
      decided: false,
    }
  }

  fn push(&mut self, chunk: &str) -> String {
    if self.decided || self.candidates.is_empty() {
      return chunk.to_string();
    }

    self.buffer.push_str(chunk);
    if !self.should_flush() {
      return String::new();
    }

    self.decided = true;
    let buffered = std::mem::take(&mut self.buffer);
    strip_prompt_echo_prefix(&buffered, &self.candidates)
  }

  fn finish(&mut self) -> Option<String> {
    if self.decided {
      return None;
    }

    self.decided = true;
    if self.buffer.is_empty() {
      return None;
    }

    let buffered = std::mem::take(&mut self.buffer);
    let visible = strip_prompt_echo_prefix(&buffered, &self.candidates);
    if visible.is_empty() {
      None
    } else {
      Some(visible)
    }
  }

  fn should_flush(&self) -> bool {
    const MIN_BUFFER_CHARS: usize = 96;

    self.buffer.chars().count() >= MIN_BUFFER_CHARS
      || self
        .buffer
        .chars()
        .any(|ch| matches!(ch, '\n' | '。' | '，' | '！' | '!' | '?'))
  }
}

fn build_prompt_echo_candidates(system_prompt: &str) -> Vec<String> {
  let trimmed = system_prompt.trim();
  if trimmed.is_empty() {
    return Vec::new();
  }

  let mut candidates = Vec::new();
  candidates.push(trimmed.to_string());

  for section in trimmed.split("\n\n") {
    let section = section.trim();
    if normalized_char_count(section) >= 16 {
      candidates.push(section.to_string());
    }
  }

  for line in trimmed.lines() {
    let line = line.trim();
    if normalized_char_count(line) >= 16 {
      candidates.push(line.to_string());
    }
  }

  candidates.sort();
  candidates.dedup();
  candidates.sort_by_key(|candidate| std::cmp::Reverse(normalized_char_count(candidate)));
  candidates
}

fn strip_prompt_echo_prefix(output: &str, candidates: &[String]) -> String {
  let trimmed = output.trim_start();
  if trimmed.is_empty() {
    return String::new();
  }

  let mut best_match: Option<(usize, usize)> = None;

  for candidate in candidates {
    if let Some((matched_count, consumed_bytes)) = prefix_overlap_len(trimmed, candidate) {
      let replace = match best_match {
        Some((best_count, _)) => matched_count > best_count,
        None => true,
      };

      if replace {
        best_match = Some((matched_count, consumed_bytes));
      }
    }
  }

  let Some((_, consumed_bytes)) = best_match else {
    return trimmed.to_string();
  };

  trimmed[consumed_bytes..]
    .trim_start_matches(is_meta_prefix_char)
    .trim_start()
    .to_string()
}

fn prefix_overlap_len(output: &str, candidate: &str) -> Option<(usize, usize)> {
  const MIN_MATCH_CHARS: usize = 16;

  let normalized_output = normalized_chars_with_offsets(output);
  let normalized_candidate = normalized_chars(candidate);
  let comparable = normalized_output.len().min(normalized_candidate.len());

  if comparable < MIN_MATCH_CHARS {
    return None;
  }

  let mut matched = 0usize;
  while matched < comparable && normalized_output[matched].0 == normalized_candidate[matched] {
    matched += 1;
  }

  if matched < MIN_MATCH_CHARS {
    return None;
  }

  Some((matched, normalized_output[matched - 1].1))
}

fn normalized_chars_with_offsets(text: &str) -> Vec<(char, usize)> {
  let mut chars = Vec::new();
  for (idx, ch) in text.char_indices() {
    if let Some(normalized) = normalize_match_char(ch) {
      chars.push((normalized, idx + ch.len_utf8()));
    }
  }
  chars
}

fn normalized_chars(text: &str) -> Vec<char> {
  text.chars().filter_map(normalize_match_char).collect()
}

fn normalized_char_count(text: &str) -> usize {
  text.chars().filter_map(normalize_match_char).count()
}

fn normalize_match_char(ch: char) -> Option<char> {
  if ch.is_whitespace() {
    return None;
  }

  if matches!(
    ch,
    '"' | '\'' | '`'
      | ':' | '：'
      | ',' | '，'
      | '.' | '。'
      | ';' | '；'
      | '!' | '！'
      | '?' | '？'
      | '-' | '—' | '_'
      | '(' | ')' | '[' | ']' | '{' | '}'
      | '<' | '>' | '/' | '\\'
  ) {
    return None;
  }

  Some(ch.to_ascii_lowercase())
}

fn is_meta_prefix_char(ch: char) -> bool {
  ch.is_whitespace()
    || matches!(
      ch,
      ':' | '：'
        | ',' | '，'
        | '.' | '。'
        | ';' | '；'
        | '-' | '—'
        | '"' | '\'' | '`'
    )
}

#[derive(Serialize)]
struct HealthResponse {
  ok: bool,
  version: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
  error: String,
}

pub fn spawn_server(
  app_root: PathBuf,
  web_dir: PathBuf,
  live2d_dir: PathBuf,
  warmup_settings: Option<RuntimeSettings>,
) -> Result<SocketAddr> {
  let listener = TcpListener::bind("127.0.0.1:3217")
    .context("failed to bind local Fast Avatar AI HTTP server")?;
  listener
    .set_nonblocking(true)
    .context("failed to configure HTTP listener")?;
  let addr = listener.local_addr()?;

  let state = AppState {
    client: reqwest::Client::builder()
      .timeout(Duration::from_secs(120))
      .user_agent("FastAvatarAI/0.2.0")
      .build()
      .context("failed to build HTTP client")?,
    speech: Arc::new(SpeechRuntime::new(app_root)),
  };

  thread::spawn(move || {
    let runtime = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .expect("failed to start tokio runtime");

    runtime.block_on(async move {
      if let Some(settings) = warmup_settings {
        let _ = app_log::write_line(
          "INFO",
          &format!(
            "speech warmup skipped | asr={} tts={} reason=startup prewarm disabled for stability",
            settings.asr.engine, settings.tts.engine
          ),
        );
      } else {
        let _ = app_log::write_line("WARN", "speech warmup skipped: runtime settings unavailable");
      }

      let listener = tokio::net::TcpListener::from_std(listener)
        .expect("failed to convert listener");
      let router = build_router(state, web_dir, live2d_dir);
      let service = router.into_make_service();
      if let Err(error) = axum::serve(listener, service).await {
        eprintln!("HTTP server stopped: {error}");
      }
    });
  });

  Ok(addr)
}

fn build_router(state: AppState, web_dir: PathBuf, live2d_dir: PathBuf) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .route("/api/log", post(frontend_log))
    .route("/api/chat/stream", post(chat_stream))
    .route("/api/tts", post(tts_proxy))
    .route("/api/session/ws", get(speech_session_ws))
    .nest_service("/live2d", ServeDir::new(live2d_dir))
    .fallback_service(ServeDir::new(web_dir).append_index_html_on_directories(true))
    .with_state(state)
}

async fn health() -> Json<HealthResponse> {
  Json(HealthResponse {
    ok: true,
    version: "0.2.0",
  })
}

async fn frontend_log(Json(request): Json<FrontendLogRequest>) -> StatusCode {
  let level = request.level.trim();
  let message = request.message.trim();
  let line = format!("[frontend:{}] {}", if level.is_empty() { "INFO" } else { level }, message);
  if let Err(error) = app_log::write_line("INFO", &line) {
    eprintln!("failed to write frontend log: {error}");
    return StatusCode::INTERNAL_SERVER_ERROR;
  }
  StatusCode::NO_CONTENT
}

async fn chat_stream(
  State(state): State<AppState>,
  Json(request): Json<ChatRequest>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
  let (tx, rx) = mpsc::channel::<StreamEvent>(64);

  tokio::spawn(async move {
    let result = run_chat_stream(state, request, tx.clone()).await;
    if let Err(error) = result {
      let _ = tx.send(StreamEvent::Error(error.to_string())).await;
    }
    let _ = tx.send(StreamEvent::Done).await;
  });

  let stream = ReceiverStream::new(rx).map(|event| {
    Ok::<Event, Infallible>(match event {
      StreamEvent::Token(text) => Event::default()
        .event("token")
        .data(json!({ "text": text }).to_string()),
      StreamEvent::Error(message) => Event::default()
        .event("error")
        .data(json!({ "message": message }).to_string()),
      StreamEvent::Done => Event::default().event("done").data("{}"),
    })
  });

  Sse::new(stream).keep_alive(
    KeepAlive::new()
      .interval(Duration::from_secs(15))
      .text("ping"),
  )
}

async fn tts_proxy(
  State(state): State<AppState>,
  Json(request): Json<TtsRequest>,
) -> Response {
  let text = request.text.trim();
  if text.is_empty() {
    return (
      StatusCode::BAD_REQUEST,
      Json(ErrorResponse {
        error: "tts text is empty".into(),
      }),
    )
      .into_response();
  }

  if request.settings.engine.trim() == "qwen_realtime" {
    return tts_proxy_qwen_realtime(request).await;
  }

  match state.speech.synthesize(&request.settings, text) {
    Ok(render) => Response::builder()
      .status(StatusCode::OK)
      .header(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"))
      .header("x-audio-format", render.format)
      .header("x-sample-rate", render.sample_rate.to_string())
      .header("x-channels", render.channels.to_string())
      .body(axum::body::Body::from(render.bytes))
      .unwrap_or_else(|error| {
        (
          StatusCode::INTERNAL_SERVER_ERROR,
          Json(ErrorResponse {
            error: format!("failed to build response: {error}"),
          }),
        )
          .into_response()
      }),
    Err(error) => {
      let _ = app_log::write_line(
        "ERROR",
        &format!(
          "tts synth failed | engine={} model_dir={} text_len={} error={}",
          request.settings.engine,
          request.settings.model_dir,
          text.chars().count(),
          error
        ),
      );
      (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
          error: error.to_string(),
        }),
      )
        .into_response()
    }
  }
}

async fn tts_proxy_qwen_realtime(request: TtsRequest) -> Response {
  let text = request.text.trim().to_string();
  let api_key = request.settings.api_key.trim().to_string();
  let voice = request.settings.voice.trim().to_string();
  let model = request.settings.model.trim().to_string();

  if api_key.is_empty() {
    return (
      StatusCode::BAD_REQUEST,
      Json(ErrorResponse {
        error: "Qwen TTS API Key 未配置".into(),
      }),
    )
      .into_response();
  }

  if voice.is_empty() {
    return (
      StatusCode::BAD_REQUEST,
      Json(ErrorResponse {
        error: "Qwen TTS 音色未配置".into(),
      }),
    )
      .into_response();
  }

  if model.is_empty() {
    return (
      StatusCode::BAD_REQUEST,
      Json(ErrorResponse {
        error: "Qwen TTS 模型未配置".into(),
      }),
    )
      .into_response();
  }

  let ws_url = match build_qwen_realtime_ws_url(&request.settings.endpoint, &model) {
    Ok(url) => url,
    Err(error) => {
      return (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
          error: error.to_string(),
        }),
      )
        .into_response();
    }
  };

  let mut ws_request = match ws_url.as_str().into_client_request() {
    Ok(request_builder) => request_builder,
    Err(error) => {
      return (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
          error: format!("failed to build Qwen TTS websocket request: {error}"),
        }),
      )
        .into_response();
    }
  };
  match format!("Bearer {}", api_key).parse() {
    Ok(value) => {
      ws_request.headers_mut().insert("Authorization", value);
    }
    Err(error) => {
      return (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
          error: format!("failed to encode Qwen TTS authorization header: {error}"),
        }),
      )
        .into_response();
    }
  }

  let (mut socket, _) = match connect_async(ws_request).await {
    Ok(connection) => connection,
    Err(error) => {
      let _ = app_log::write_line(
        "ERROR",
        &format!("qwen realtime websocket connect failed | model={} error={}", model, error),
      );
      return (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
          error: format!("failed to connect Qwen TTS websocket: {error}"),
        }),
      )
        .into_response();
    }
  };

  let mode = normalize_qwen_mode(&request.settings.mode).to_string();
  let response_format = "pcm";
  let sample_rate = 24_000_u32;
  let language_type = normalize_qwen_language_type(&request.settings.language);
  let session_update = json!({
    "type": "session.update",
    "session": {
      "voice": voice,
      "mode": mode,
      "language_type": language_type,
      "response_format": response_format,
      "sample_rate": sample_rate
    }
  });

  if let Err(error) = socket
    .send(ClientWsMessage::Text(session_update.to_string().into()))
    .await
  {
    return (
      StatusCode::BAD_REQUEST,
      Json(ErrorResponse {
        error: format!("failed to send Qwen TTS session.update: {error}"),
      }),
    )
      .into_response();
  }

  let negotiated_sample_rate = match wait_for_qwen_session_ready(&mut socket).await {
    Ok(value) => value,
    Err(error) => {
      let _ = app_log::write_line(
        "ERROR",
        &format!("qwen realtime session init failed | model={} error={}", model, error),
      );
      return (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
          error: error.to_string(),
        }),
      )
        .into_response();
    }
  };

  let (write, read) = socket.split();
  let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(32);
  let request_settings = request.settings.clone();
  tokio::spawn(async move {
    stream_qwen_realtime_audio(write, read, tx, request_settings, text).await;
  });

  Response::builder()
    .status(StatusCode::OK)
    .header(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"))
    .header("x-audio-format", "pcm_s16le")
    .header("x-sample-rate", negotiated_sample_rate.to_string())
    .header("x-channels", "1")
    .header("x-leading-padding-ms", "60")
    .header("x-trailing-padding-ms", "180")
    .body(axum::body::Body::from_stream(ReceiverStream::new(rx)))
    .unwrap_or_else(|error| {
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
          error: format!("failed to build Qwen TTS response: {error}"),
        }),
      )
        .into_response()
    })
}

async fn wait_for_qwen_session_ready(
  socket: &mut tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
  >,
) -> Result<u32> {
  while let Some(message) = socket.next().await {
    let message = message.map_err(|error| anyhow!("failed to read Qwen TTS websocket message: {error}"))?;
    let Some(text) = websocket_text_message(&message) else {
      continue;
    };
    let payload: serde_json::Value =
      serde_json::from_str(&text).context("failed to decode Qwen TTS websocket payload")?;
    match payload.get("type").and_then(|value| value.as_str()).unwrap_or_default() {
      "session.created" => continue,
      "session.updated" => {
        let sample_rate = payload
          .get("session")
          .and_then(|session| session.get("sample_rate"))
          .and_then(|value| value.as_u64())
          .map(|value| value as u32)
          .unwrap_or(24_000);
        return Ok(sample_rate);
      }
      "error" => bail!("{}", qwen_error_message(&payload)),
      _ => continue,
    }
  }

  bail!("Qwen TTS websocket closed before session.updated")
}

async fn stream_qwen_realtime_audio(
  mut write: futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ClientWsMessage,
  >,
  mut read: futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
  >,
  tx: mpsc::Sender<Result<Bytes, Infallible>>,
  settings: TtsSettings,
  text: String,
) {
  let mode = normalize_qwen_mode(&settings.mode).to_string();
  let send_task = tokio::spawn(async move {
    for chunk in chunk_qwen_text_input(&text) {
      let append = json!({
        "type": "input_text_buffer.append",
        "text": chunk,
      });
      write.send(ClientWsMessage::Text(append.to_string().into())).await?;
      sleep(Duration::from_millis(8)).await;
    }

    if mode == "commit" {
      let commit = json!({ "type": "input_text_buffer.commit" });
      write.send(ClientWsMessage::Text(commit.to_string().into())).await?;
    }

    let finish = json!({ "type": "session.finish" });
    write.send(ClientWsMessage::Text(finish.to_string().into())).await?;
    Result::<(), tokio_tungstenite::tungstenite::Error>::Ok(())
  });

  let mut audio_chunks = 0usize;
  while let Some(message) = read.next().await {
    let message = match message {
      Ok(message) => message,
      Err(error) => {
        let _ = app_log::write_line("ERROR", &format!("qwen realtime read failed | error={}", error));
        break;
      }
    };

    let Some(text) = websocket_text_message(&message) else {
      continue;
    };
    let payload: serde_json::Value = match serde_json::from_str(&text) {
      Ok(value) => value,
      Err(error) => {
        let _ = app_log::write_line(
          "WARN",
          &format!("qwen realtime payload decode failed | error={}", error),
        );
        continue;
      }
    };

    match payload.get("type").and_then(|value| value.as_str()).unwrap_or_default() {
      "response.audio.delta" => {
        let Some(delta) = payload.get("delta").and_then(|value| value.as_str()) else {
          continue;
        };
        match base64::engine::general_purpose::STANDARD.decode(delta) {
          Ok(bytes) if !bytes.is_empty() => {
            audio_chunks += 1;
            if tx.send(Ok(Bytes::from(bytes))).await.is_err() {
              break;
            }
          }
          Ok(_) => {}
          Err(error) => {
            let _ = app_log::write_line(
              "WARN",
              &format!("qwen realtime audio delta decode failed | error={}", error),
            );
          }
        }
      }
      "response.done" => {}
      "session.finished" => break,
      "error" => {
        let _ = app_log::write_line("ERROR", &format!("qwen realtime stream error | {}", qwen_error_message(&payload)));
        break;
      }
      _ => {}
    }
  }

  match send_task.await {
    Ok(Ok(())) => {}
    Ok(Err(error)) => {
      let _ = app_log::write_line("ERROR", &format!("qwen realtime write failed | error={}", error));
    }
    Err(error) => {
      let _ = app_log::write_line("ERROR", &format!("qwen realtime write task join failed | error={}", error));
    }
  }

  let _ = app_log::write_line(
    "INFO",
    &format!(
      "qwen realtime stream completed | model={} voice={} chunks={}",
      settings.model, settings.voice, audio_chunks
    ),
  );
}

fn build_qwen_realtime_ws_url(endpoint: &str, model: &str) -> Result<String> {
  let endpoint = endpoint.trim();
  if endpoint.is_empty() {
    bail!("Qwen TTS endpoint 未配置");
  }
  let mut url = url::Url::parse(endpoint).with_context(|| format!("invalid Qwen TTS endpoint: {endpoint}"))?;
  if !matches!(url.scheme(), "ws" | "wss") {
    bail!("Qwen TTS endpoint 必须使用 ws:// 或 wss://");
  }
  if url.query_pairs().all(|(key, _)| key != "model") {
    url.query_pairs_mut().append_pair("model", model.trim());
  }
  Ok(url.to_string())
}

fn normalize_qwen_language_type(language: &str) -> &'static str {
  match language.trim().to_ascii_lowercase().as_str() {
    "en" | "english" => "English",
    "auto" => "Auto",
    _ => "Chinese",
  }
}

fn normalize_qwen_mode(mode: &str) -> &'static str {
  match mode.trim() {
    "commit" => "commit",
    _ => "server_commit",
  }
}

fn chunk_qwen_text_input(text: &str) -> Vec<String> {
  let mut chunks = Vec::new();
  let mut current = String::new();
  let mut count = 0usize;
  for ch in text.chars() {
    current.push(ch);
    count += 1;
    if count >= 24 || matches!(ch, '。' | '！' | '？' | '；' | ';' | ',' | '，' | '\n') {
      let next = current.trim();
      if !next.is_empty() {
        chunks.push(next.to_string());
      }
      current.clear();
      count = 0;
    }
  }
  let tail = current.trim();
  if !tail.is_empty() {
    chunks.push(tail.to_string());
  }
  if chunks.is_empty() {
    chunks.push(text.trim().to_string());
  }
  chunks
}

fn websocket_text_message(message: &ClientWsMessage) -> Option<String> {
  match message {
    ClientWsMessage::Text(text) => Some(text.to_string()),
    ClientWsMessage::Binary(bytes) => String::from_utf8(bytes.to_vec()).ok(),
    _ => None,
  }
}

fn qwen_error_message(payload: &serde_json::Value) -> String {
  payload
    .get("error")
    .and_then(|value| {
      value
        .get("message")
        .and_then(|message| message.as_str())
        .or_else(|| value.as_str())
    })
    .or_else(|| payload.get("message").and_then(|value| value.as_str()))
    .unwrap_or("unknown Qwen realtime error")
    .to_string()
}

async fn speech_session_ws(
  ws: WebSocketUpgrade,
  State(state): State<AppState>,
) -> impl IntoResponse {
  ws.on_upgrade(move |socket| handle_speech_session(socket, state))
}

async fn handle_speech_session(mut socket: WebSocket, state: AppState) {
  let mut session = None;
  if send_ws_message(&mut socket, json!({ "type": "session.ready" }).to_string())
    .await
    .is_err()
  {
    return;
  }

  while let Some(message) = socket.next().await {
    let Ok(message) = message else {
      break;
    };

    let Message::Text(text) = message else {
      if matches!(message, Message::Close(_)) {
        break;
      }
      continue;
    };

    let payload: SpeechSocketMessage = match serde_json::from_str(&text) {
      Ok(payload) => payload,
      Err(error) => {
        let _ = send_ws_message(
          &mut socket,
          json!({ "type": "error", "message": format!("invalid socket payload: {error}") }).to_string(),
        )
        .await;
        continue;
      }
    };

    match payload.kind.as_str() {
      "session.start" => {
        let Some(asr_settings) = payload.asr else {
          let _ = send_ws_message(
            &mut socket,
            json!({ "type": "error", "message": "missing asr settings" }).to_string(),
          )
          .await;
          continue;
        };
        match state.speech.create_asr_session(&asr_settings) {
          Ok(next_session) => {
            session = Some(next_session);
            let _ = send_ws_message(&mut socket, json!({ "type": "session.listening" }).to_string()).await;
          }
          Err(error) => {
            let _ = send_ws_message(
              &mut socket,
              json!({ "type": "error", "message": error.to_string() }).to_string(),
            )
            .await;
          }
        }
      }
      "audio.chunk" => {
        let Some(ref mut current_session) = session else {
          let _ = send_ws_message(
            &mut socket,
            json!({ "type": "error", "message": "speech session not started" }).to_string(),
          )
          .await;
          continue;
        };
        match decode_pcm_chunk(&payload.pcm) {
          Ok(samples) => {
            if let Err(error) = emit_asr_events(&mut socket, current_session.accept_samples(samples)).await {
              let _ = send_ws_message(
                &mut socket,
                json!({ "type": "error", "message": error.to_string() }).to_string(),
              )
              .await;
            }
          }
          Err(error) => {
            let _ = send_ws_message(
              &mut socket,
              json!({ "type": "error", "message": error.to_string() }).to_string(),
            )
            .await;
          }
        }
      }
      "audio.flush" => {
        let Some(ref mut current_session) = session else {
          continue;
        };
        if let Err(error) = emit_asr_events(&mut socket, current_session.flush()).await {
          let _ = send_ws_message(
            &mut socket,
            json!({ "type": "error", "message": error.to_string() }).to_string(),
          )
          .await;
        }
      }
      "session.stop" => break,
      _ => {
        let _ = send_ws_message(
          &mut socket,
          json!({ "type": "error", "message": "unsupported speech event" }).to_string(),
        )
        .await;
      }
    }
  }
}

async fn emit_asr_events(
  socket: &mut WebSocket,
  result: Result<Vec<AsrEvent>>,
) -> Result<()> {
  for event in result? {
    match event {
      AsrEvent::SpeechStart => {
        send_ws_message(socket, json!({ "type": "speech.start" }).to_string()).await?;
      }
      AsrEvent::SpeechEnd => {
        send_ws_message(socket, json!({ "type": "speech.end" }).to_string()).await?;
      }
      AsrEvent::TranscriptFinal(text) => {
        send_ws_message(socket, json!({ "type": "asr.final", "text": text }).to_string()).await?;
      }
    }
  }
  Ok(())
}

async fn send_ws_message(socket: &mut WebSocket, message: String) -> Result<()> {
  socket
    .send(Message::Text(message.into()))
    .await
    .map_err(|error| anyhow!("failed to send websocket message: {error}"))
}

fn decode_pcm_chunk(value: &str) -> Result<Vec<f32>> {
  let bytes = base64::engine::general_purpose::STANDARD
    .decode(value)
    .context("failed to decode PCM chunk")?;
  if bytes.len() % 2 != 0 {
    bail!("PCM chunk must be 16-bit aligned");
  }

  Ok(
    bytes
      .chunks_exact(2)
      .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
      .collect(),
  )
}

async fn run_chat_stream(
  state: AppState,
  request: ChatRequest,
  tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
  let provider = request.settings.llm.provider.trim().to_lowercase();
  let endpoint = request.settings.llm.endpoint.trim().to_string();
  let messages = request.messages;

  if provider == "ollama" && !endpoint.is_empty() {
    return run_ollama_stream(&state, &endpoint, &request.settings.llm, &messages, tx).await;
  }

  if provider == "openai_compat" && !endpoint.is_empty() {
    return run_openai_stream(&state, &endpoint, &request.settings.llm, &messages, tx).await;
  }

  run_mock_stream(&messages, tx).await
}

async fn run_mock_stream(messages: &[ChatMessage], tx: mpsc::Sender<StreamEvent>) -> Result<()> {
  let user_text = messages
    .iter()
    .rev()
    .find(|message| message.role == "user")
    .map(|message| message.content.trim())
    .unwrap_or("");

  let reply = build_mock_reply(user_text);
  for chunk in split_text_chunks(&reply, 2) {
    tx.send(StreamEvent::Token(chunk))
      .await
      .map_err(|_| anyhow!("stream closed"))?;
    sleep(Duration::from_millis(34)).await;
  }
  Ok(())
}

fn build_mock_reply(user_text: &str) -> String {
  let text = user_text.trim();
  if text.is_empty() {
    return "我在。你可以直接说一句话，我会尽量用更短、更自然的方式接住你。".to_string();
  }

  let lower = text.to_lowercase();
  if text.contains("你好") || lower.contains("hello") || lower.contains("hi") {
    return "你好，我在。今天想先聊点什么？".to_string();
  }

  if text.contains("累") || text.contains("压力") || text.contains("烦") || text.contains("难过") {
    return "听起来你已经撑得有点久了。我们可以先把最重的那件事放到桌面上，再只处理最小的一步。".to_string();
  }

  if text.contains("不会") || text.contains("怎么") || text.contains("帮我") || text.contains("解决") {
    return "可以，我们先不追求一次做完。你把目标告诉我，我会帮你拆成很小的步骤。".to_string();
  }

  format!(
    "我听到了：{text}。如果你愿意，我们可以继续把它说得更清楚一点，我会尽量保持简短并且陪着你。"
  )
}

async fn run_openai_stream(
  state: &AppState,
  endpoint: &str,
  llm: &LlmSettings,
  messages: &[ChatMessage],
  tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
  let mut request = state
    .client
    .post(endpoint)
    .json(&json!({
      "model": llm.model,
      "messages": messages,
      "stream": true,
      "temperature": llm.temperature,
      "max_tokens": llm.max_tokens,
      "enable_thinking": false,
      "chat_template_kwargs": {
        "enable_thinking": false
      }
    }));

  if !llm.api_key.trim().is_empty() {
    request = request.bearer_auth(llm.api_key.trim());
  }

  let response = request
    .send()
    .await
    .with_context(|| format!("failed to send request to {endpoint}"))?;
  let status = response.status();
  if !status.is_success() {
    let body = response.text().await.unwrap_or_default();
    let body = truncate_for_log(&body, 800);
    let message = format!(
      "remote llm returned error for {endpoint} | status={} model={} provider={} body={}",
      status, llm.model, llm.provider, body
    );
    let _ = app_log::write_line("ERROR", &message);
    bail!(message);
  }

  let mut body = response.bytes_stream();
  let mut buffer = String::new();
  let mut thinking_filter = ThinkingFilter::default();
  let system_prompt = messages
    .iter()
    .find(|message| message.role == "system")
    .map(|message| message.content.as_str())
    .unwrap_or_default();
  let mut prompt_echo_filter = PromptEchoFilter::new(system_prompt);

  while let Some(chunk) = body.next().await {
    let chunk = chunk.context("failed to read llm stream chunk")?;
    buffer.push_str(std::str::from_utf8(&chunk)?);

    while let Some(idx) = buffer.find('\n') {
      let line = buffer.drain(..=idx).collect::<String>();
      let line = line.trim().trim_start_matches("data:").trim();
      if line.is_empty() {
        continue;
      }
      if line == "[DONE]" {
        return Ok(());
      }

      let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => continue,
      };

      if let Some(content) = value["choices"][0]["delta"]["content"].as_str() {
        let visible = prompt_echo_filter.push(&thinking_filter.push(content));
        if !visible.is_empty() {
          tx.send(StreamEvent::Token(visible))
            .await
            .map_err(|_| anyhow!("stream closed"))?;
        }
      }
    }
  }

  if let Some(leftover) = thinking_filter.finish() {
    let visible = prompt_echo_filter.push(&leftover);
    if !visible.is_empty() {
      tx.send(StreamEvent::Token(visible))
        .await
        .map_err(|_| anyhow!("stream closed"))?;
    }
  }

  if let Some(leftover) = prompt_echo_filter.finish() {
    tx.send(StreamEvent::Token(leftover))
      .await
      .map_err(|_| anyhow!("stream closed"))?;
  }

  Ok(())
}

async fn run_ollama_stream(
  state: &AppState,
  endpoint: &str,
  llm: &LlmSettings,
  messages: &[ChatMessage],
  tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
  let response = state
    .client
    .post(endpoint)
    .json(&json!({
      "model": llm.model,
      "messages": messages,
      "stream": true,
      "think": false,
      "options": {
        "temperature": llm.temperature,
        "num_predict": llm.max_tokens,
        "num_ctx": llm.context_length,
      }
    }))
    .send()
    .await
    .with_context(|| format!("failed to send request to {endpoint}"))?;
  let status = response.status();
  if !status.is_success() {
    let body = response.text().await.unwrap_or_default();
    let body = truncate_for_log(&body, 800);
    let message = format!(
      "remote llm returned error for {endpoint} | status={} model={} provider={} body={}",
      status, llm.model, llm.provider, body
    );
    let _ = app_log::write_line("ERROR", &message);
    bail!(message);
  }

  let mut body = response.bytes_stream();
  let mut buffer = String::new();
  let mut thinking_filter = ThinkingFilter::default();
  let system_prompt = messages
    .iter()
    .find(|message| message.role == "system")
    .map(|message| message.content.as_str())
    .unwrap_or_default();
  let mut prompt_echo_filter = PromptEchoFilter::new(system_prompt);

  while let Some(chunk) = body.next().await {
    let chunk = chunk.context("failed to read llm stream chunk")?;
    buffer.push_str(std::str::from_utf8(&chunk)?);

    while let Some(idx) = buffer.find('\n') {
      let line = buffer.drain(..=idx).collect::<String>();
      let line = line.trim();
      if line.is_empty() {
        continue;
      }

      let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => continue,
      };

      if value["done"].as_bool().unwrap_or(false) {
        return Ok(());
      }

      if let Some(content) = value["message"]["content"].as_str() {
        let visible = prompt_echo_filter.push(&thinking_filter.push(content));
        if !visible.is_empty() {
          tx.send(StreamEvent::Token(visible))
            .await
            .map_err(|_| anyhow!("stream closed"))?;
        }
      }
    }
  }

  if let Some(leftover) = thinking_filter.finish() {
    let visible = prompt_echo_filter.push(&leftover);
    if !visible.is_empty() {
      tx.send(StreamEvent::Token(visible))
        .await
        .map_err(|_| anyhow!("stream closed"))?;
    }
  }

  if let Some(leftover) = prompt_echo_filter.finish() {
    tx.send(StreamEvent::Token(leftover))
      .await
      .map_err(|_| anyhow!("stream closed"))?;
  }

  Ok(())
}

fn truncate_for_log(value: &str, max_chars: usize) -> String {
  let mut out = String::new();
  for (idx, ch) in value.chars().enumerate() {
    if idx >= max_chars {
      out.push_str("...(truncated)");
      break;
    }
    out.push(ch);
  }
  out.replace('\n', "\\n")
}

fn split_text_chunks(text: &str, chunk_size: usize) -> Vec<String> {
  let mut chunks = Vec::new();
  let mut buffer = String::new();
  let mut count = 0usize;

  for ch in text.chars() {
    buffer.push(ch);
    count += 1;
    if count >= chunk_size {
      chunks.push(buffer.clone());
      buffer.clear();
      count = 0;
    }
  }

  if !buffer.is_empty() {
    chunks.push(buffer);
  }

  chunks
}
