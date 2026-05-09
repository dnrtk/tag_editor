use std::sync::Arc;
use std::thread;

use crate::config::Config;

mod assets;
mod handlers;
mod router;
mod server_state;

pub use server_state::ServerState;

pub struct WebHandle {
    /// Underlying server. `unblock()` causes the worker thread to drain and exit.
    pub server: Arc<tiny_http::Server>,
    /// Bind address (e.g. "0.0.0.0:47823") suitable for log/UI display.
    pub bind: String,
}

impl WebHandle {
    /// Returns a URL pointing at the loopback interface for opening in a local browser.
    pub fn local_url(&self) -> String {
        // The server listens on 0.0.0.0 but we open via 127.0.0.1 so the browser
        // doesn't trip on "0.0.0.0" being interpreted as a literal hostname.
        match self.bind.split(':').next_back() {
            Some(port) => format!("http://127.0.0.1:{}/", port),
            None => format!("http://{}/", self.bind),
        }
    }

    pub fn shutdown(&self) {
        self.server.unblock();
    }
}

/// Spawn the embedded HTTP server on a background thread. Returns a handle on success,
/// or the underlying error message if the listener could not be created.
///
/// The server binds to `0.0.0.0:<port>` so it is reachable from peers on the same LAN.
pub fn spawn(config: Config) -> Result<WebHandle, String> {
    let port = config.web_port;
    let bind = format!("0.0.0.0:{}", port);

    let server = tiny_http::Server::http(&bind).map_err(|e| e.to_string())?;
    let server = Arc::new(server);
    let state = Arc::new(ServerState::new(config));

    let server_for_thread = server.clone();
    thread::Builder::new()
        .name("tag_editor_web".to_string())
        .spawn(move || run(server_for_thread, state))
        .map_err(|e| format!("failed to spawn web thread: {}", e))?;

    Ok(WebHandle { server, bind })
}

fn run(server: Arc<tiny_http::Server>, state: Arc<ServerState>) {
    for request in server.incoming_requests() {
        let state = state.clone();
        // Each request is handled on a worker thread so a slow image read does not
        // block other clients (browsers commonly issue several requests in parallel).
        let _ = thread::Builder::new()
            .name("tag_editor_web_req".to_string())
            .spawn(move || {
                router::dispatch(request, &state);
            });
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    use super::*;

    /// End-to-end smoke test: bind on an ephemeral local port, spawn the worker thread,
    /// and verify a single GET / yields a 200 with the embedded HTML.
    #[test]
    fn serves_index_html_on_get_root() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral");
        let addr = server.server_addr().to_ip().expect("ip addr");
        let server = Arc::new(server);
        let state = Arc::new(ServerState::new(Config::default()));

        let server_clone = server.clone();
        let state_clone = state.clone();
        thread::spawn(move || run(server_clone, state_clone));

        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 200"), "response: {}", response);
        assert!(response.contains("Tag Editor"), "missing title");
        assert!(response.contains("text/html"), "wrong content type");
        // Smoke check that the splitter element exists — the JS resize logic depends on it.
        assert!(response.contains("splitter-left"), "missing splitter element");

        // Hint to the worker thread: dropping the server triggers shutdown after the next
        // accept call. Test threads exit cleanly when the test process does.
        server.unblock();
    }

    /// Performs a single HTTP request against an ephemeral server and returns the
    /// raw response (status + headers + body) as a string.
    fn http_get(path: &str) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral");
        let addr = server.server_addr().to_ip().expect("ip addr");
        let server = Arc::new(server);
        let state = Arc::new(ServerState::new(Config::default()));

        let server_clone = server.clone();
        let state_clone = state.clone();
        thread::spawn(move || run(server_clone, state_clone));

        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            path
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        server.unblock();
        response
    }

    /// Verifies the served CSS contains the truncation rule that drives the file-name
    /// ellipsis behavior on the left sidebar. If this asserts away, the asset hasn't
    /// been rebuilt after editing app.css.
    #[test]
    fn css_carries_truncation_and_splitter_rules() {
        let response = http_get("/static/app.css");
        assert!(response.starts_with("HTTP/1.1 200"), "response: {}", response);
        assert!(response.contains("text/css"), "wrong content type");
        assert!(
            response.contains("text-overflow: ellipsis"),
            "missing ellipsis rule"
        );
        assert!(response.contains(".splitter"), "missing splitter rule");
        assert!(
            response.contains(".thumbs-mode"),
            "missing thumbnail grid class"
        );
        assert!(
            response.contains("min-width: 0"),
            "missing min-width:0 (truncation in grid layout)"
        );
    }

    /// Verifies the served JS exports the helpers we wired up for the recent UI work.
    #[test]
    fn js_carries_splitter_and_close_logic() {
        let response = http_get("/static/app.js");
        assert!(response.starts_with("HTTP/1.1 200"), "response: {}", response);
        assert!(response.contains("application/javascript"), "wrong content type");
        assert!(response.contains("initSplitter"), "missing initSplitter");
        assert!(response.contains("closeImage"), "missing closeImage");
        assert!(
            response.contains("thumbs-mode"),
            "missing thumbs-mode class toggle"
        );
        assert!(
            response.contains("Escape"),
            "missing Escape key binding"
        );
        assert!(
            response.contains("openSlideshowPopup"),
            "missing slideshow popup wiring"
        );
        assert!(
            response.contains("initFloatingDrag"),
            "missing floating drag handler"
        );
    }

    /// The page must contain the floating slideshow popup container so the JS can
    /// show/hide it on demand.
    #[test]
    fn html_contains_floating_slideshow_popup() {
        let response = http_get("/");
        assert!(response.starts_with("HTTP/1.1 200"), "response: {}", response);
        assert!(response.contains("id=\"slideshow-popup\""), "missing popup container");
        assert!(response.contains("id=\"slideshow-open\""), "missing open trigger");
        assert!(response.contains("id=\"slideshow-popup-header\""), "missing drag handle");
    }

    /// `Cache-Control: no-store` on static assets prevents the browser from serving a
    /// stale copy after the binary is rebuilt.
    #[test]
    fn static_assets_have_no_store_cache_header() {
        for path in ["/", "/static/app.css", "/static/app.js"] {
            let response = http_get(path);
            assert!(
                response.to_ascii_lowercase().contains("cache-control: no-store"),
                "{} missing cache-control: {}",
                path,
                response
            );
        }
    }

    #[test]
    fn returns_404_for_unknown_route() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral");
        let addr = server.server_addr().to_ip().expect("ip addr");
        let server = Arc::new(server);
        let state = Arc::new(ServerState::new(Config::default()));

        let server_clone = server.clone();
        let state_clone = state.clone();
        thread::spawn(move || run(server_clone, state_clone));

        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream
            .write_all(b"GET /no-such-path HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 404"), "response: {}", response);

        server.unblock();
    }

    #[test]
    fn returns_hotkeys_json() {
        use std::collections::HashMap;

        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral");
        let addr = server.server_addr().to_ip().expect("ip addr");
        let server = Arc::new(server);

        let mut config = Config::default();
        let mut hk = HashMap::new();
        hk.insert("1".to_string(), "cat".to_string());
        hk.insert("2".to_string(), "dog".to_string());
        config.hotkey_tags = hk;
        let state = Arc::new(ServerState::new(config));

        let server_clone = server.clone();
        let state_clone = state.clone();
        thread::spawn(move || run(server_clone, state_clone));

        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream
            .write_all(b"GET /api/hotkeys HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 200"), "response: {}", response);
        assert!(response.contains("\"1\":\"cat\""), "missing cat: {}", response);
        assert!(response.contains("\"2\":\"dog\""), "missing dog: {}", response);

        server.unblock();
    }
}
