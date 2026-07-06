use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "frontend/dist"]
#[include = "assets/*"]
#[include = "index.html"]
struct Assets;

fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    }
}

fn serve_file(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let headers =
                HeaderMap::from_iter([(header::CONTENT_TYPE, mime_type(path).parse().unwrap())]);
            (headers, content.data).into_response()
        }
        None => serve_embedded_index(),
    }
}

fn serve_embedded_index() -> Response {
    match Assets::get("index.html") {
        Some(content) => {
            let headers = HeaderMap::from_iter([(
                header::CONTENT_TYPE,
                "text/html; charset=utf-8".parse().unwrap(),
            )]);
            (headers, content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.is_empty() || path == "index.html" || !path.contains('.') {
        return serve_embedded_index();
    }

    serve_file(path)
}
