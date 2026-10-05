//! A minimal HTTP/1.1 server on a loopback socket: enough for ureq and Chromium (keep-alive,
//! `Content-Length` bodies, CORS preflight). One thread per connection - this crate is a test
//! harness, never part of the app.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Requests larger than this are refused (the Worker's largest upload is a 5 MB feed image).
const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_HEADER_LINES: usize = 64;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// Path without the query.
    pub path: String,
    pub query: Vec<(String, String)>,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub extra_headers: Vec<(&'static str, String)>,
}

impl Response {
    pub fn json(value: &serde_json::Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json; charset=utf-8",
            body: value.to_string().into_bytes(),
            extra_headers: Vec::new(),
        }
    }

    /// The Worker's `text(message, status)`.
    pub fn text(status: u16, message: &str) -> Self {
        Self {
            status,
            content_type: "text/plain;charset=UTF-8",
            body: message.as_bytes().to_vec(),
            extra_headers: Vec::new(),
        }
    }
}

/// What the handler decides for a request (faults included).
pub enum Outcome {
    Respond(Response),
    /// Close the connection without answering (a dropped response).
    Drop,
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut content_length = 0usize;
    let mut content_type = None;
    for _ in 0..MAX_HEADER_LINES {
        let mut h = String::new();
        if reader.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            if k == "content-length" {
                content_length = v.trim().parse().ok()?;
            } else if k == "content-type" {
                content_type = Some(v.trim().to_string());
            }
        }
    }
    if content_length > MAX_BODY {
        return None;
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).ok()?;
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (
            p.to_string(),
            q.split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (percent_decode(k), percent_decode(v)))
                .collect(),
        ),
        None => (target, Vec::new()),
    };
    Some(Request {
        method,
        path,
        query,
        content_type,
        body,
    })
}

fn write_response(stream: &mut TcpStream, r: &Response) -> std::io::Result<()> {
    let reason = match r.status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    };
    let mut head = format!(
        "HTTP/1.1 {} {reason}\r\ncontent-type: {}\r\ncontent-length: {}\r\naccess-control-allow-origin: *\r\naccess-control-allow-methods: GET, POST, OPTIONS\r\naccess-control-allow-headers: content-type\r\n",
        r.status,
        r.content_type,
        r.body.len()
    );
    for (k, v) in &r.extra_headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&r.body)?;
    stream.flush()
}

pub struct Listener {
    pub port: u16,
    pub stop: Arc<AtomicBool>,
    pub connections: Arc<AtomicU64>,
    pub open_connections: Arc<AtomicU64>,
    pub max_open_connections: Arc<AtomicU64>,
    accept_thread: Option<std::thread::JoinHandle<()>>,
}

impl Listener {
    /// Binds `127.0.0.1:<port>` (0 = an OS-assigned free port) and serves until stopped.
    pub fn start<F>(port: u16, handler: F) -> std::io::Result<Self>
    where
        F: Fn(Request) -> Outcome + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(AtomicU64::new(0));
        let open = Arc::new(AtomicU64::new(0));
        let max_open = Arc::new(AtomicU64::new(0));
        let handler = Arc::new(handler);
        let (stop2, conns2, open2, max2) = (
            stop.clone(),
            connections.clone(),
            open.clone(),
            max_open.clone(),
        );
        let accept_thread = std::thread::Builder::new()
            .name("social-mock-accept".into())
            .spawn(move || {
                while !stop2.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            conns2.fetch_add(1, Ordering::Relaxed);
                            let n = open2.fetch_add(1, Ordering::Relaxed) + 1;
                            max2.fetch_max(n, Ordering::Relaxed);
                            let (h, stop3, open3) = (handler.clone(), stop2.clone(), open2.clone());
                            let _ = std::thread::Builder::new()
                                .name("social-mock-conn".into())
                                .spawn(move || {
                                    serve(stream, &*h, &stop3);
                                    open3.fetch_sub(1, Ordering::Relaxed);
                                });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(5)),
                    }
                }
            })?;
        Ok(Self {
            port,
            stop,
            connections,
            open_connections: open,
            max_open_connections: max_open,
            accept_thread: Some(accept_thread),
        })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.accept_thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve(stream: TcpStream, handler: &dyn Fn(Request) -> Outcome, stop: &AtomicBool) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let Some(request) = read_request(&mut reader) else {
            // idle keep-alive timeout or closed: poll the stop flag again unless the peer left
            if reader
                .get_ref()
                .peek(&mut [0u8; 1])
                .map(|n| n == 0)
                .unwrap_or(false)
            {
                break;
            }
            match reader.fill_buf() {
                Ok([]) => break,
                Ok(_) => continue,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue
                }
                Err(_) => break,
            }
        };
        if request.method == "OPTIONS" {
            let _ = write_response(
                &mut writer,
                &Response {
                    status: 204,
                    content_type: "text/plain",
                    body: Vec::new(),
                    extra_headers: Vec::new(),
                },
            );
            continue;
        }
        match handler(request) {
            Outcome::Respond(r) => {
                if write_response(&mut writer, &r).is_err() {
                    break;
                }
            }
            Outcome::Drop => {
                let _ = writer.shutdown(Shutdown::Both);
                break;
            }
        }
    }
}
