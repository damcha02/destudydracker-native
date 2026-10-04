//! The HTTP transport (Stage 22a, decision D7): `ureq` 3 with `rustls` (ring provider, Mozilla
//! roots from `webpki-roots`), blocking, used only from the network worker thread.
//!
//! - TLS: rustls' normal certificate and hostname verification; nothing is disabled, pinned or
//!   overridden. Local mock tests use plain HTTP to a loopback address instead of weakening TLS.
//! - Finite timeouts: connect 10 s, and a per-request total (`ApiRequest::timeout`).
//! - No redirects (`max_redirects(0)`): the Worker never redirects, and following one could carry
//!   a request (and its body) to another host.
//! - Bounded bodies: a response larger than `ApiRequest::max_response` is refused while reading.
//! - Proxies: production honours the environment's proxy settings (as the WebView did with the
//!   system's); loopback test traffic never goes through a proxy.
//! - In test builds every non-loopback origin is refused before any socket is opened (the
//!   structural guarantee that `cargo test` cannot reach production).

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::endpoint::Origin;
use super::http::{ApiRequest, Body, HttpResponse, Method, NetError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub trait Transport: Send + Sync {
    fn execute(&self, origin: &Origin, request: &ApiRequest) -> Result<HttpResponse, NetError>;
}

/// Every origin a transport opened a connection to in this process (host:port, no path or
/// query), for the Stage 22 connection audit (`STUDY_NATIVE_NET_AUDIT`, the STATS line).
static CONTACTED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

pub fn contacted_origins() -> Vec<String> {
    CONTACTED
        .lock()
        .map(|s| s.iter().cloned().collect())
        .unwrap_or_default()
}

fn record_contact(origin: &Origin) {
    let key = format!("{}:{}", origin.host, origin.port);
    if let Ok(mut set) = CONTACTED.lock() {
        if set.insert(key.clone()) {
            log::info!(
                "network: first connection this run to {key} ({})",
                if origin.is_loopback() {
                    "local test server"
                } else {
                    "Social Worker"
                }
            );
        }
    }
}

/// Test builds: refuse anything that is not loopback. Release/debug app builds: allow.
pub mod guard {
    use super::Origin;

    pub fn allows(origin: &Origin) -> bool {
        if cfg!(test) {
            origin.is_loopback()
        } else {
            true
        }
    }
}

pub struct UreqTransport {
    secure: ureq::Agent,
    local: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn agent(proxy_from_env: bool) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("StudyTracker-native/{}", env!("CARGO_PKG_VERSION")))
        .max_idle_connections(2)
        .max_idle_connections_per_host(2)
        .proxy(if proxy_from_env {
            ureq::Proxy::try_from_env()
        } else {
            None
        })
        .build();
    ureq::Agent::new_with_config(config)
}

fn map_error(error: &ureq::Error) -> NetError {
    match error {
        ureq::Error::Timeout(_) => NetError::Timeout,
        ureq::Error::BodyExceedsLimit(_) | ureq::Error::LargeResponseHeader(..) => {
            NetError::Malformed
        }
        ureq::Error::Protocol(_) => NetError::Malformed,
        ureq::Error::TooManyRedirects | ureq::Error::RedirectFailed => NetError::Blocked,
        ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::Tls(_)
        | ureq::Error::Rustls(_)
        | ureq::Error::ConnectProxyFailed(_) => NetError::Offline,
        ureq::Error::Io(io) => match io.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => NetError::Timeout,
            std::io::ErrorKind::InvalidData => NetError::Malformed,
            _ => NetError::Offline,
        },
        _ => NetError::Offline,
    }
}

impl UreqTransport {
    pub fn new() -> Self {
        Self {
            secure: agent(true),
            local: agent(false),
        }
    }
}

impl Transport for UreqTransport {
    fn execute(&self, origin: &Origin, request: &ApiRequest) -> Result<HttpResponse, NetError> {
        if !guard::allows(origin) {
            return Err(NetError::Blocked);
        }
        record_contact(origin);
        let agent = if origin.is_loopback() {
            &self.local
        } else {
            &self.secure
        };
        // the full URL (with any credential-bearing query) lives only in this frame
        let url = format!("{origin}{}", request.path_and_query());
        let started = Instant::now();
        let result = match (&request.method, &request.body) {
            (Method::Get, _) => agent
                .get(&url)
                .config()
                .timeout_global(Some(request.timeout))
                .build()
                .call(),
            (Method::Post, Body::None) => agent
                .post(&url)
                .config()
                .timeout_global(Some(request.timeout))
                .build()
                .send_empty(),
            (Method::Post, Body::Json(bytes)) => agent
                .post(&url)
                .header("content-type", "application/json")
                .config()
                .timeout_global(Some(request.timeout))
                .build()
                .send(bytes.as_slice()),
            (
                Method::Post,
                Body::Multipart {
                    content_type,
                    bytes,
                },
            ) => agent
                .post(&url)
                .header("content-type", content_type.as_str())
                .config()
                .timeout_global(Some(request.timeout))
                .build()
                .send(bytes.as_slice()),
        };
        drop(url);
        let mut response = match result {
            Ok(r) => r,
            Err(error) => {
                let mapped = map_error(&error);
                log::info!(
                    "network: {:?} {} failed after {:?}: {}",
                    request.method,
                    request.log_path(),
                    started.elapsed(),
                    mapped.kind()
                );
                return Err(mapped);
            }
        };
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.chars().take(100).collect::<String>());
        let body = response
            .body_mut()
            .with_config()
            .limit(request.max_response)
            .read_to_vec()
            .map_err(|e| map_error(&e))?;
        log::info!(
            "network: {:?} {} -> {status} ({} bytes) in {:?}",
            request.method,
            request.log_path(),
            body.len(),
            started.elapsed()
        );
        Ok(HttpResponse {
            status,
            content_type,
            body,
        })
    }
}
