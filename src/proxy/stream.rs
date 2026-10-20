use crate::proxy::guard::ActiveConnectionGuard;
use axum::{
    body::Body,
    http::header,
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

/// 规范化 Token 消耗度量数据
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct SseChunkUsagePayload {
    usage: Option<TokenUsageRaw>,
}

#[derive(Debug, Deserialize)]
struct TokenUsageRaw {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

/// 增量流式 SSE Usage 解析器
#[derive(Debug, Default)]
pub struct SseUsageParser {
    usage: Option<TokenUsage>,
}

impl SseUsageParser {
    pub fn new() -> Self {
        Self { usage: None }
    }

    /// 增量输入上游字节块并嗅探 usage
    pub fn feed_chunk(&mut self, chunk: &[u8]) {
        // 快速子串预检：绝大多数普通文本 chunk 纳秒级直接跳过
        if !chunk.windows(8).any(|w| w == b"\"usage\":") {
            return;
        }

        // 尝试从 SSE 文本行或 JSON 载荷中解析
        if let Ok(text) = std::str::from_utf8(chunk) {
            for line in text.lines() {
                let trimmed = line.trim();
                let payload_str = if let Some(stripped) = trimmed.strip_prefix("data:") {
                    stripped.trim()
                } else {
                    trimmed
                };

                if payload_str.starts_with('{') && payload_str.ends_with('}') {
                    if let Ok(payload) = serde_json::from_str::<SseChunkUsagePayload>(payload_str) {
                        if let Some(raw) = payload.usage {
                            let prompt = raw.prompt_tokens.unwrap_or(0);
                            let completion = raw.completion_tokens.unwrap_or(0);
                            let total = raw.total_tokens.unwrap_or(prompt + completion);
                            self.usage = Some(TokenUsage {
                                prompt_tokens: prompt,
                                completion_tokens: completion,
                                total_tokens: total,
                            });
                            return;
                        }
                    }
                }
            }
        }
    }

    /// 提取解析到的 Token 消耗
    pub fn extract_usage(&self) -> Option<TokenUsage> {
        self.usage
    }
}

/// 构建支持客户端断连主动取消与 RAII 计数回收的 SSE 响应流
pub fn create_abortable_sse_stream(
    upstream_response: reqwest::Response,
    guard: ActiveConnectionGuard,
) -> Response {
    let mut byte_stream = upstream_response.bytes_stream();

    let sse_stream = async_stream::stream! {
        // guard 的所有权移入流式生成器内部
        // 当流正常结束、上游报错、或下游客户端断开导致 Stream 被 Drop 时，guard 必然被 Drop！
        let _guard = guard;

        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(bytes) => yield Ok::<_, axum::Error>(bytes),
                Err(err) => {
                    tracing::warn!("Upstream SSE stream reading error: {}", err);
                    let err_frame = format!(
                        "event: error\ndata: {{\"error\":{{\"message\":\"Upstream SSE connection broken: {}\",\"type\":\"server_error\",\"code\":\"upstream_stream_error\"}}}}\n\ndata: [DONE]\n\n",
                        err
                    );
                    yield Ok::<_, axum::Error>(axum::body::Bytes::from(err_frame));
                    break;
                }
            }
        }
    };

    let builder = Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive");

    match builder.body(Body::from_stream(sse_stream)) {
        Ok(res) => res,
        Err(err) => {
            tracing::error!("Failed to build SSE response: {}", err);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error Building Stream",
            )
                .into_response()
        }
    }
}
