//! `social-mock` (Stage 22a): a deterministic, synthetic, loopback-only stand-in for the
//! production Social Worker, for the native client's protocol, failure, latency and stress tests
//! and for local visual-parity captures (production frontend and native app both pointed at it).
//!
//! - Loopback only (`127.0.0.1`, OS-assigned port by default). Never a production URL.
//! - Synthetic data only; nothing is written to disk (state is in memory).
//! - Deterministic: an injected clock (`set_now`), sequential ids, fixed seeds.
//! - Faults and impairments for tests: per-route status/body overrides, dropped responses,
//!   latency, and a request log (method and path only - never bodies or query strings).
//! - Not linked into the app: a separate workspace crate, used as a dev-dependency.

pub mod http;
pub mod multipart;
pub mod seed;
pub mod world;

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use world::World;

/// `new Date(ms).toISOString()`.
pub fn iso(ms: i64) -> String {
    study_tracker_core::dashboard::civil::to_iso_utc_string(
        study_tracker_core::timer::WallTimestamp::from_unix_millis(ms),
    )
}

/// A scripted misbehaviour for the next request(s) to a path.
#[derive(Debug, Clone)]
pub enum Fault {
    /// Answer with this status and raw body (content type text/plain unless the body is JSON).
    Status(u16, Vec<u8>),
    /// Answer 200 with this raw body (malformed JSON, wrong types, oversized...).
    Body(Vec<u8>),
    /// Close the connection without answering.
    Drop,
    /// Wait this long before handling (a slow server; also used for timeouts).
    Delay(Duration),
}

#[derive(Default)]
struct Script {
    /// (path, fault) pairs consumed in order by the first matching request
    faults: VecDeque<(String, Fault)>,
    latency: Duration,
    log: Vec<(String, String)>,
}

pub struct MockServer {
    pub world: Arc<Mutex<World>>,
    script: Arc<Mutex<Script>>,
    listener: http::Listener,
}

impl MockServer {
    /// Starts on `127.0.0.1:<port>` (0 = any free port) with `world`. The world's origin is set
    /// to the bound address, as the Worker derives image URLs from its own origin.
    pub fn start(port: u16, mut world: World) -> std::io::Result<Self> {
        let script = Arc::new(Mutex::new(Script::default()));
        let world_arc = Arc::new(Mutex::new(World::default()));
        let (w, s) = (world_arc.clone(), script.clone());
        let listener = http::Listener::start(port, move |req| {
            let fault = {
                let mut s = s.lock().unwrap();
                s.log.push((req.method.clone(), req.path.clone()));
                let i = s.faults.iter().position(|(p, _)| *p == req.path);
                let latency = s.latency;
                (i.and_then(|i| s.faults.remove(i)).map(|(_, f)| f), latency)
            };
            if !fault.1.is_zero() {
                std::thread::sleep(fault.1);
            }
            match fault.0 {
                Some(Fault::Drop) => return http::Outcome::Drop,
                Some(Fault::Status(status, body)) => {
                    let json = body.first() == Some(&b'{');
                    return http::Outcome::Respond(http::Response {
                        status,
                        content_type: if json {
                            "application/json; charset=utf-8"
                        } else {
                            "text/plain;charset=UTF-8"
                        },
                        body,
                        extra_headers: Vec::new(),
                    });
                }
                Some(Fault::Body(body)) => {
                    return http::Outcome::Respond(http::Response {
                        status: 200,
                        content_type: "application/json; charset=utf-8",
                        body,
                        extra_headers: Vec::new(),
                    })
                }
                Some(Fault::Delay(d)) => std::thread::sleep(d),
                None => {}
            }
            http::Outcome::Respond(w.lock().unwrap().handle(&req))
        })?;
        world.origin = format!("http://127.0.0.1:{}", listener.port);
        *world_arc.lock().unwrap() = world;
        Ok(Self {
            world: world_arc,
            script,
            listener,
        })
    }

    pub fn port(&self) -> u16 {
        self.listener.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.listener.port)
    }

    pub fn fault(&self, path: &str, fault: Fault) {
        self.script
            .lock()
            .unwrap()
            .faults
            .push_back((path.to_string(), fault));
    }

    pub fn set_latency(&self, latency: Duration) {
        self.script.lock().unwrap().latency = latency;
    }

    pub fn set_now(&self, ms: i64) {
        self.world.lock().unwrap().now_ms = ms;
    }

    /// (method, path) of every request so far.
    pub fn log(&self) -> Vec<(String, String)> {
        self.script.lock().unwrap().log.clone()
    }

    pub fn count(&self, path: &str) -> usize {
        self.script
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|(_, p)| p == path)
            .count()
    }

    pub fn clear_log(&self) {
        self.script.lock().unwrap().log.clear();
    }

    /// TCP connections accepted so far / currently open / the most open at once.
    pub fn connections(&self) -> (u64, u64, u64) {
        (
            self.listener.connections.load(Ordering::Relaxed),
            self.listener.open_connections.load(Ordering::Relaxed),
            self.listener.max_open_connections.load(Ordering::Relaxed),
        )
    }

    pub fn stop(mut self) {
        self.listener.stop();
    }
}
