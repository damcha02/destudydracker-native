//! `social-mock` CLI: serves a synthetic world on 127.0.0.1 for manual runs and parity captures.
//!
//!   social-mock [--port 47811] [--seed demo|demo-submitted|self-only|empty]
//!               [--now 2026-10-04T12:00:00+02:00] [--latency-ms 0]
//!               [--write-credentials <native data dir>]
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
    let world = match arg(&args, "--seed").as_deref() {
        Some("empty") => seed::empty(now),
        Some("self-only") => seed::self_only(now),
        Some("demo-submitted") => seed::demo(now, true),
        _ => seed::demo(now, false),
    };
    let server = MockServer::start(port, world).expect("bind 127.0.0.1");
    seed::bind_origin(&mut server.world.lock().unwrap());
    if let Some(ms) = arg(&args, "--latency-ms").and_then(|v| v.parse().ok()) {
        server.set_latency(Duration::from_millis(ms));
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
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
