//! `social-mock` CLI: serves a synthetic world on 127.0.0.1 for manual runs and parity captures.
//!
//!   social-mock [--port 47811] [--seed demo|demo-submitted|self-only|empty]
//!               [--now 2026-10-04T12:00:00+02:00] [--latency-ms 0]
//!               [--write-credentials <native data dir>] [--fault <path>=<500|malformed|drop|delay:ms>[*n]]
//!               [--squad none] [--owner] [--announcement]   (Stage 22b)
//!
//! `--write-credentials` writes the synthetic self identity, bound to the local-test endpoint
//! class, into a native profile directory (never a production identity). Ctrl-C stops it.

use std::time::Duration;

use social_mock::{seed, MockServer};

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = arg(&args, "--port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let now = arg(&args, "--now")
        .and_then(|s| study_tracker_core::social::SocialTimestamp::parse(&s))
        .map(|t| t.0)
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64)
        });
    let mut world = match arg(&args, "--seed").as_deref() {
        Some("empty") => seed::empty(now),
        Some("self-only") => seed::self_only(now),
        Some("demo-submitted") => seed::demo(now, true),
        _ => seed::demo(now, false),
    };
    // Stage 22b: feed and squads on the demo worlds; `--squad none` leaves the user squadless,
    // `--owner` gives the user production's owner tag (owner panels), `--announcement` adds one
    if world.users.len() > 1 {
        seed::add_22b(&mut world, arg(&args, "--squad").as_deref() != Some("none"));
    }
    if args.iter().any(|a| a == "--owner") {
        if let Some(u) = world.users.get_mut(seed::SELF_ID) {
            u.friend_code = social_mock::world_22b::OWNER_CODE.into();
        }
    }
    if args.iter().any(|a| a == "--announcement") {
        world.x.announcements.push((
            "synthetic-announcement-1".into(),
            "Exam season tips".into(),
            "Synthetic announcement for local testing.".into(),
            None,
            true,
        ));
    }
    let server = MockServer::start(port, world).expect("bind 127.0.0.1");
    seed::bind_origin(&mut server.world.lock().unwrap());
    if let Some(ms) = arg(&args, "--latency-ms").and_then(|v| v.parse().ok()) {
        server.set_latency(Duration::from_millis(ms));
    }
    // `--fault <path>=<kind>[*n]` (repeatable): the next n (default 1) requests to <path> get
    // `500`/any status, `malformed` (200 with a non-JSON body), `drop` (connection closed) or
    // `delay:<ms>`. Manual counterparts of the faults the tests script through `MockServer::fault`.
    for (i, a) in args.iter().enumerate() {
        if a != "--fault" {
            continue;
        }
        let Some((path, spec)) = args.get(i + 1).and_then(|s| s.split_once('=')) else {
            continue;
        };
        let (kind, n) = match spec.split_once('*') {
            Some((k, n)) => (k, n.parse().unwrap_or(1)),
            None => (spec, 1),
        };
        let fault = match kind {
            "malformed" => social_mock::Fault::Body(b"<html>not json".to_vec()),
            "drop" => social_mock::Fault::Drop,
            k if k.starts_with("delay:") => {
                social_mock::Fault::Delay(Duration::from_millis(k[6..].parse().unwrap_or(1000)))
            }
            k => social_mock::Fault::Status(k.parse().unwrap_or(500), b"server error".to_vec()),
        };
        for _ in 0..n {
            server.fault(path, fault.clone());
        }
    }
    if let Some(dir) = arg(&args, "--write-credentials") {
        let path = std::path::Path::new(&dir).join("social-credentials.json");
        std::fs::create_dir_all(&dir).expect("data dir");
        let text = format!(
            "{{\n  \"version\": 1,\n  \"endpoint\": \"local-test\",\n  \"userId\": \"{}\",\n  \"deviceSecret\": \"{}\"\n}}\n",
            seed::SELF_ID,
            seed::SELF_SECRET
        );
        std::fs::write(&path, text).expect("write credentials");
        println!(
            "wrote synthetic local-test credentials to {}",
            path.display()
        );
    }
    println!("SOCIAL_MOCK {}", server.url());
    // the request log (method and path only, never a query or body) as it grows
    let mut printed = 0;
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let log = server.log();
        for (method, path) in &log[printed.min(log.len())..] {
            println!("REQ {method} {path}");
        }
        printed = log.len();
    }
}
