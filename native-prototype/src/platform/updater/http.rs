//! HTTPS client for the updater, on Windows' own **WinHTTP** (system certificate store, system
//! proxy settings, the OS TLS stack). No TLS or HTTP crate is linked into the binary.
//!
//! Safety properties: only `https://` URLs are accepted (loopback `http://127.0.0.1` only when a
//! test constructs the client with `allow_insecure_loopback`); no credentials in URLs; WinHTTP's
//! default redirect policy (HTTPS may not redirect to HTTP) is kept - GitHub release downloads
//! redirect from `github.com` to `objects.githubusercontent.com` over HTTPS; every read is bounded
//! by the caller's `max_bytes`; timeouts are set so a stalled server cannot hang the worker.

use super::service::Fetcher;
use super::UpdateError;
use crate::platform::win_util::wide;
use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a handle returned by a WinHttp* open/connect call, closed exactly once.
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// A parsed `http(s)://host[:port]/path`.
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedUrl {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    pub path_and_query: String,
}

pub fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return None;
    };
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().ok()?),
        None => (authority, if secure { 443 } else { 80 }),
    };
    if host.is_empty() {
        return None;
    }
    let path_and_query = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    Some(ParsedUrl {
        secure,
        host: host.to_string(),
        port,
        path_and_query,
    })
}

pub struct WinHttpFetcher {
    allow_insecure_loopback: bool,
}

impl WinHttpFetcher {
    pub fn new() -> Self {
        Self {
            allow_insecure_loopback: false,
        }
    }

    /// Test-only: permit plain `http://127.0.0.1`.
    #[cfg(test)]
    pub fn for_loopback_tests() -> Self {
        Self {
            allow_insecure_loopback: true,
        }
    }
}

impl Default for WinHttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

fn fetch_err(what: &str) -> UpdateError {
    // SAFETY: thread-local last-error read.
    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    UpdateError::Fetch(format!("{what} (Windows error {code})"))
}

impl Fetcher for WinHttpFetcher {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, UpdateError> {
        let parsed = parse_url(url).ok_or_else(|| UpdateError::InsecureUrl(url.to_string()))?;
        if !parsed.secure && !(self.allow_insecure_loopback && parsed.host == "127.0.0.1") {
            return Err(UpdateError::InsecureUrl(url.to_string()));
        }
        let agent = wide(&format!(
            "StudyTracker-Native/{} updater",
            env!("CARGO_PKG_VERSION")
        ));
        let host = wide(&parsed.host);
        let verb = wide("GET");
        let path = wide(&parsed.path_and_query);
        // SAFETY: standard WinHTTP call sequence. Every pointer references a live, NUL-terminated
        // local; every handle is wrapped in `Handle` so it is closed on all paths; the response
        // buffer length passed to WinHttpReadData never exceeds the buffer.
        unsafe {
            let session = Handle(WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                std::ptr::null(),
                std::ptr::null(),
                0,
            ));
            if session.0.is_null() {
                return Err(fetch_err("WinHttpOpen failed"));
            }
            WinHttpSetTimeouts(session.0, 10_000, 10_000, 30_000, 30_000);
            let connection = Handle(WinHttpConnect(session.0, host.as_ptr(), parsed.port, 0));
            if connection.0.is_null() {
                return Err(fetch_err("could not connect"));
            }
            let flags = if parsed.secure {
                WINHTTP_FLAG_SECURE
            } else {
                0
            };
            let request = Handle(WinHttpOpenRequest(
                connection.0,
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                flags,
            ));
            if request.0.is_null() {
                return Err(fetch_err("could not open the request"));
            }
            if WinHttpSendRequest(request.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 {
                return Err(fetch_err("send failed"));
            }
            if WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0 {
                return Err(fetch_err("no response"));
            }
            let mut status: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                std::ptr::null(),
                &mut status as *mut u32 as *mut core::ffi::c_void,
                &mut size,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(fetch_err("could not read the HTTP status"));
            }
            if !(200..300).contains(&status) {
                return Err(UpdateError::Fetch(format!("HTTP {status}")));
            }
            let mut body = Vec::new();
            loop {
                let mut available: u32 = 0;
                if WinHttpQueryDataAvailable(request.0, &mut available) == 0 {
                    return Err(fetch_err("read failed"));
                }
                if available == 0 {
                    break;
                }
                let chunk = (available as usize).min(64 * 1024);
                if body.len() + chunk > max_bytes {
                    return Err(UpdateError::TooLarge { limit: max_bytes });
                }
                let start = body.len();
                body.resize(start + chunk, 0);
                let mut read: u32 = 0;
                if WinHttpReadData(
                    request.0,
                    body[start..].as_mut_ptr() as *mut core::ffi::c_void,
                    chunk as u32,
                    &mut read,
                ) == 0
                {
                    return Err(fetch_err("read failed"));
                }
                body.truncate(start + read as usize);
                if read == 0 {
                    break;
                }
            }
            Ok(body)
        }
    }
}

/// Reads the "feed" and artifacts from local files: a diagnostic/test feed
/// (`STUDY_NATIVE_UPDATE_FEED_FILE`). It is honoured in every build because it cannot weaken
/// anything: the trust root stays the compiled-in key, so a local feed can at most produce
/// "up to date" or an update whose signature fails. Only `file:`-free plain paths; relative
/// artifact URLs in such a feed are resolved next to the feed file.
pub struct FileFetcher {
    pub base_dir: std::path::PathBuf,
}

impl Fetcher for FileFetcher {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, UpdateError> {
        let name = url.rsplit(['/', '\\']).next().unwrap_or(url);
        if name.is_empty() || name.contains("..") || name.contains(':') {
            return Err(UpdateError::Fetch("unusable local file name".into()));
        }
        let path = self.base_dir.join(name);
        let bytes = std::fs::read(&path)
            .map_err(|e| UpdateError::Fetch(format!("{}: {e}", path.display())))?;
        if bytes.len() > max_bytes {
            return Err(UpdateError::TooLarge { limit: max_bytes });
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn urls_parse_into_host_port_and_path() {
        assert_eq!(
            parse_url("https://github.com/o/r/releases/latest/download/latest.json"),
            Some(ParsedUrl {
                secure: true,
                host: "github.com".into(),
                port: 443,
                path_and_query: "/o/r/releases/latest/download/latest.json".into()
            })
        );
        assert_eq!(parse_url("https://h:8443/a?b=c").unwrap().port, 8443);
        assert_eq!(parse_url("https://h").unwrap().path_and_query, "/");
        assert_eq!(parse_url("https://h?x=1").unwrap().path_and_query, "/?x=1");
        for bad in [
            "",
            "ftp://h/x",
            "https://",
            "https://u:p@h/x",
            "https://h:notaport/x",
            "h/x",
            "https://:443/x",
        ] {
            assert!(parse_url(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn plain_http_to_the_outside_world_is_refused_before_any_network_activity() {
        let fetcher = WinHttpFetcher::new();
        assert!(matches!(
            fetcher.get("http://example.com/latest.json", 1024),
            Err(UpdateError::InsecureUrl(_))
        ));
        assert!(
            matches!(
                fetcher.get("http://127.0.0.1:9/latest.json", 1024),
                Err(UpdateError::InsecureUrl(_))
            ),
            "release fetchers never use http, even on loopback"
        );
        assert!(matches!(
            WinHttpFetcher::for_loopback_tests().get("http://evil.example/x", 1024),
            Err(UpdateError::InsecureUrl(_))
        ));
    }

    /// Serves `responses` (status line + body) one per connection on a loopback port.
    fn serve(responses: Vec<(u16, Vec<u8>)>) -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        (port, handle)
    }

    #[test]
    fn the_real_winhttp_client_fetches_enforces_limits_and_reports_http_errors() {
        let big = vec![7u8; 200_000];
        let (port, server) = serve(vec![
            (200, b"{\"ok\":true}".to_vec()),
            (404, b"nope".to_vec()),
            (200, big.clone()),
            (200, big.clone()),
        ]);
        let fetcher = WinHttpFetcher::for_loopback_tests();
        let base = format!("http://127.0.0.1:{port}");
        assert_eq!(
            fetcher.get(&format!("{base}/latest.json"), 1024).unwrap(),
            b"{\"ok\":true}"
        );
        assert_eq!(
            fetcher.get(&format!("{base}/missing"), 1024),
            Err(UpdateError::Fetch("HTTP 404".into()))
        );
        assert_eq!(
            fetcher.get(&format!("{base}/big"), 100_000),
            Err(UpdateError::TooLarge { limit: 100_000 }),
            "bounded read"
        );
        assert_eq!(
            fetcher.get(&format!("{base}/big"), 300_000).unwrap(),
            big,
            "a large body arrives intact across chunks"
        );
        server.join().unwrap();
        // Nothing listening any more: a clean error, not a hang or panic.
        assert!(matches!(
            fetcher.get(&format!("{base}/gone"), 1024),
            Err(UpdateError::Fetch(_))
        ));
    }

    #[test]
    fn the_file_fetcher_cannot_escape_its_directory() {
        let dir = std::env::temp_dir().join(format!("st18-filefetch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("latest.json"), b"{}").unwrap();
        let fetcher = FileFetcher {
            base_dir: dir.clone(),
        };
        assert_eq!(fetcher.get("latest.json", 10).unwrap(), b"{}");
        assert_eq!(
            fetcher.get("https://x/y/latest.json", 10).unwrap(),
            b"{}",
            "only the final component is used"
        );
        for hostile in [
            "..\\..\\windows\\win.ini",
            "C:\\windows\\win.ini",
            "a..b",
            "x:stream",
            "",
        ] {
            assert!(fetcher.get(hostile, 10).is_err(), "{hostile:?}");
        }
        assert_eq!(
            fetcher.get("latest.json", 1),
            Err(UpdateError::TooLarge { limit: 1 })
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
