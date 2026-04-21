use axum::response::Response;
use axum::http::StatusCode;

/// GET /dashboard - serve index.html with deprecation banner
pub async fn serve_dashboard_root() -> Response {
    serve_file_with_banner("index.html")
}

/// GET /dashboard/* - serve static dashboard files (SPA with index.html fallback)
pub async fn serve_dashboard(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    // Path traversal protection
    if path.contains("..") || path.starts_with('/') || path.contains('\\') {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(axum::body::Body::from("Invalid path"))
            .unwrap_or_else(|_| Response::new(axum::body::Body::from("Bad request")));
    }
    if path == "index.html" {
        serve_file_with_banner("index.html")
    } else {
        serve_file(&path)
    }
}

const DEPRECATION_BANNER: &str = r#"<div style="background:#2563eb;color:#fff;text-align:center;padding:10px 20px;font-family:system-ui,sans-serif;font-size:14px;">
  ⚠️ Этот интерфейс устарел. Новый интерфейс доступен на <a href="https://flowlink.flow-masters.ru" style="color:#fff;text-decoration:underline;font-weight:bold;">flowlink.flow-masters.ru</a>
</div>"#;

fn serve_file_with_banner(path: &str) -> Response {
    let dashboard_dir = std::path::Path::new("/opt/flowlink/dashboard");
    let file_path = dashboard_dir.join(path);

    if let Ok(content) = std::fs::read(&file_path) {
        let html = String::from_utf8_lossy(&content);
        let modified = html.replacen("<body", &format!("<body\n{}\n", DEPRECATION_BANNER), 1);

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(axum::body::Body::from(modified))
            .unwrap_or_else(|_| Response::new(axum::body::Body::from("Internal error")));

        log::info!("Served dashboard file (with banner): {}", path);
        return response;
    }

    serve_file(path)
}

fn serve_file(path: &str) -> Response {
    let dashboard_dir = std::path::Path::new("/opt/flowlink/dashboard");
    let file_path = dashboard_dir.join(path);

    if let Ok(content) = std::fs::read(&file_path) {
        let mime_type = mime_guess(path);

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_type)
            .body(axum::body::Body::from(content))
            .unwrap_or_else(|_| Response::new(axum::body::Body::from("Internal error")));

        log::info!("Served dashboard file: {}", path);
        return response;
    }

    if let Ok(content) = std::fs::read(dashboard_dir.join("index.html")) {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(axum::body::Body::from(content))
            .unwrap_or_else(|_| Response::new(axum::body::Body::from("Internal error")));

        log::info!("Served dashboard index.html (fallback)");
        return response;
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(axum::body::Body::from("Dashboard file not found"))
        .unwrap_or_else(|_| Response::new(axum::body::Body::from("Not found")))
}

fn mime_guess(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "js" => "application/javascript",
        "mjs" => "application/javascript",
        "css" => "text/css",
        "html" => "text/html",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}
