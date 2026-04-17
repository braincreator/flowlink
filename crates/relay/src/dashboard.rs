use axum::response::Response;
use axum::http::StatusCode;

/// GET /dashboard - serve index.html
pub async fn serve_dashboard_root() -> Response {
    serve_file("index.html")
}

/// GET /dashboard/* - serve static dashboard files (SPA with index.html fallback)
pub async fn serve_dashboard(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    serve_file(&path)
}

fn serve_file(path: &str) -> Response {
    let dashboard_dir = std::path::Path::new("/opt/flowlink/dashboard");
    let file_path = dashboard_dir.join(path);

    if let Ok(content) = std::fs::read(&file_path) {
        let mime_type = mime_guess(&path);

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
