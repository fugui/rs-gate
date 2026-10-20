use axum::http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use std::time::Duration;
use uuid::Uuid;

/// 强制剥离的 Hop-by-Hop 请求头清单
const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// 代理 HTTP 客户端，统一管理与物理后端的长连接池
#[derive(Clone)]
pub struct ProxyClient {
    client: Client,
}

impl Default for ProxyClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// 清洗入站请求头并注入链路追踪与托管凭据
    pub fn prepare_upstream_headers(
        &self,
        inbound_headers: &HeaderMap,
        upstream_api_key: Option<&str>,
        client_ip: Option<&str>,
    ) -> (HeaderMap, String) {
        let mut headers = HeaderMap::new();

        // 1. 提取或生成 x-request-id
        let request_id = inbound_headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // 2. 拷贝合法头，剥离 Hop-by-Hop
        for (name, value) in inbound_headers.iter() {
            let name_str = name.as_str().to_ascii_lowercase();

            // 检查是否为 Hop-by-Hop
            if HOP_BY_HOP_HEADERS.contains(&name_str.as_str()) {
                continue;
            }

            // 临时跳过 Authorization 和 Host，由后续步骤统一处理
            if name_str == "authorization" || name_str == "host" {
                continue;
            }

            headers.insert(name.clone(), value.clone());
        }

        // 3. 注入 x-request-id
        if let Ok(val) = HeaderValue::from_str(&request_id) {
            headers.insert(HeaderName::from_static("x-request-id"), val);
        }

        // 4. 处理凭据隔离: 仅在后端配置 upstream_api_key 时注入真实凭据
        if let Some(key) = upstream_api_key {
            let auth_value = format!("Bearer {}", key);
            if let Ok(val) = HeaderValue::from_str(&auth_value) {
                headers.insert(HeaderName::from_static("authorization"), val);
            }
        }

        // 5. 追加客户端 IP 到 X-Forwarded-For
        if let Some(ip) = client_ip {
            let current_xff = inbound_headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok());
            let new_xff = match current_xff {
                Some(existing) if !existing.is_empty() => format!("{}, {}", existing, ip),
                _ => ip.to_string(),
            };
            if let Ok(val) = HeaderValue::from_str(&new_xff) {
                headers.insert(HeaderName::from_static("x-forwarded-for"), val);
            }
        }

        (headers, request_id)
    }
}
