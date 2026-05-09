use std::sync::Arc;

use tiny_http::{Method, Request, Response};

use super::assets;
use super::handlers;
use super::server_state::ServerState;

pub fn dispatch(mut request: Request, state: &Arc<ServerState>) {
    let url = request.url().to_string();
    let (path, query) = split_path_query(&url);

    let response = match (request.method(), path) {
        (Method::Get, "/") => Some(no_cache(html_response(200, assets::INDEX_HTML.to_string()))),
        (Method::Get, "/static/app.js") => Some(no_cache(text_response(
            200,
            "application/javascript; charset=utf-8",
            assets::APP_JS.to_string(),
        ))),
        (Method::Get, "/static/app.css") => Some(no_cache(text_response(
            200,
            "text/css; charset=utf-8",
            assets::APP_CSS.to_string(),
        ))),
        (Method::Get, "/api/tree") => Some(handlers::tree(query)),
        (Method::Get, "/api/image") => {
            let _ = handlers::image(request, query);
            return;
        }
        (Method::Get, "/api/thumb") => {
            let _ = handlers::thumb(request, query);
            return;
        }
        (Method::Get, "/api/tags") => Some(handlers::get_tags(query)),
        (Method::Put, "/api/tags") => Some(handlers::put_tags(&mut request, query)),
        (Method::Post, "/api/tags") => Some(handlers::put_tags(&mut request, query)),
        (Method::Get, "/api/hotkeys") => Some(handlers::hotkeys(state)),
        (Method::Get, "/api/filter") => Some(handlers::filter(query)),
        _ => None,
    };

    let response = response.unwrap_or_else(|| text_response(404, "text/plain", "Not found".into()));
    let _ = request.respond(response);
}

fn split_path_query(url: &str) -> (&str, &str) {
    match url.find('?') {
        Some(idx) => (&url[..idx], &url[idx + 1..]),
        None => (url, ""),
    }
}

pub fn json_response(status: u16, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
                .unwrap(),
        )
}

pub fn text_response(status: u16, content_type: &str, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes(b"Content-Type".as_slice(), content_type.as_bytes())
        .expect("valid Content-Type header");
    Response::from_string(body)
        .with_status_code(status)
        .with_header(header)
}

pub fn html_response(status: u16, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    text_response(status, "text/html; charset=utf-8", body)
}

/// Adds a `Cache-Control: no-store` header so the browser always fetches the latest
/// embedded asset after a rebuild, instead of serving a stale copy from disk cache.
fn no_cache(
    response: Response<std::io::Cursor<Vec<u8>>>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes(b"Cache-Control".as_slice(), b"no-store".as_slice())
        .expect("valid Cache-Control header");
    response.with_header(header)
}
