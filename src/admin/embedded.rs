use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use sha2::{Digest, Sha256};

#[derive(RustEmbed)]
#[folder = "web/dist/"]
pub struct Asset;

pub async fn static_handler(uri: Uri) -> Response {
    let raw_path = uri.path().trim_start_matches('/');
    let path = if raw_path.is_empty() {
        "index.html"
    } else {
        raw_path
    };

    // 1. Direct file lookup
    if let Some(content) = Asset::get(path) {
        return serve_asset(path, content);
    }

    // 2. SPA History Fallback: If not backend API endpoints, fallback to index.html
    let uri_str = uri.path();
    if !uri_str.starts_with("/api/")
        && !uri_str.starts_with("/v1/")
        && !uri_str.starts_with("/health/")
        && uri_str != "/metrics"
    {
        if let Some(content) = Asset::get("index.html") {
            return serve_asset("index.html", content);
        }
    }

    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

fn serve_asset(path: &str, content: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let is_html =
        mime.type_() == mime_guess::mime::TEXT && mime.subtype() == mime_guess::mime::HTML;

    let content_type = if is_html {
        "text/html; charset=utf-8"
    } else {
        mime.as_ref()
    };

    let mut hasher = Sha256::new();
    hasher.update(&content.data);
    let hash = hasher.finalize();
    let etag = format!("\"{:x}\"", hash);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type)
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        )
        .header(header::ETAG, etag)
        .body(Body::from(content.data.into_owned()))
        .unwrap_or_else(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        })
}
