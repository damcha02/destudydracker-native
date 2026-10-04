//! UI-thread side of the network (Stage 22a): one lazily created [`NetWorker`] for the configured
//! endpoint, and the routing of replies back to the controller that asked.
//!
//! A controller hands an [`Outgoing`] and a handler; the worker thread performs the request,
//! applies the post-processing (image decoding) right there, and posts the reply to the UI thread
//! with `slint::invoke_from_event_loop`, where the handler (kept in a UI-thread table, never sent
//! across threads) runs. Nothing here exists until the first request: with no Social identity
//! there is no worker thread and no socket.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::net::endpoint::{Origin, SocialEndpoint};
use crate::net::http::NetError;
use crate::net::transport::UreqTransport;
use crate::net::worker::{Job, NetWorker, StatsSnapshot};
use crate::net_jobs::{NetReply, Outgoing};

type Handler = Box<dyn FnOnce(u64, NetReply)>;

struct Runtime {
    origin: Option<Origin>,
    worker: Option<Arc<NetWorker>>,
    next_id: u64,
    handlers: HashMap<u64, (u64, Handler)>,
}

thread_local! {
    static NET: RefCell<Runtime> = RefCell::new(Runtime { origin: None, worker: None, next_id: 0, handlers: HashMap::new() });
}

/// The endpoint for this run (`None`: Social is not available - no request will ever be made).
pub fn configure(endpoint: Option<&SocialEndpoint>) {
    NET.with(|n| n.borrow_mut().origin = endpoint.map(SocialEndpoint::origin));
}

fn deliver(id: u64, reply: NetReply) {
    let entry = NET.with(|n| n.borrow_mut().handlers.remove(&id));
    if let Some((token, handler)) = entry {
        handler(token, reply);
    }
}

/// Sends a controller's request; `handler(token, reply)` runs later on the UI thread.
pub fn submit(out: Outgoing, handler: impl FnOnce(u64, NetReply) + 'static) {
    let prepared = NET.with(|n| {
        let mut n = n.borrow_mut();
        n.next_id += 1;
        let id = n.next_id;
        n.handlers.insert(id, (out.token, Box::new(handler)));
        let Some(origin) = n.origin.clone() else {
            return (id, None);
        };
        if n.worker.is_none() {
            n.worker = Some(Arc::new(NetWorker::new(
                Arc::new(UreqTransport::new()),
                origin,
            )));
        }
        (id, n.worker.clone())
    });
    let (id, Some(worker)) = prepared else {
        // no endpoint: answer "cancelled" without touching the network - asynchronously, so a
        // caller holding its own state borrow is never re-entered
        let id = prepared.0;
        let _ = slint::invoke_from_event_loop(move || {
            deliver(id, NetReply::Http(Err(NetError::Cancelled)))
        });
        return;
    };
    let post = out.post;
    let job = Job {
        request: out.request,
        cancel: Some(out.cancel),
        reply: Box::new(move |result| {
            let reply = NetReply::from_result(result, post);
            let _ = slint::invoke_from_event_loop(move || deliver(id, reply));
        }),
    };
    if worker.submit(job).is_err() {
        // queue full or shutting down: report it through the normal path (asynchronously)
        let _ = slint::invoke_from_event_loop(move || {
            deliver(id, NetReply::Http(Err(NetError::Cancelled)))
        });
    }
}

pub fn stats() -> Option<StatsSnapshot> {
    NET.with(|n| n.borrow().worker.as_ref().map(|w| w.stats()))
}

/// Stops the worker (bounded wait) and drops every pending handler.
pub fn shutdown() {
    let worker = NET.with(|n| {
        let mut n = n.borrow_mut();
        n.handlers.clear();
        n.worker.take()
    });
    if let Some(w) = worker {
        w.shutdown();
    }
}

/// One line for the diagnostics STATS output.
pub fn report() -> String {
    let origins = crate::net::transport::contacted_origins();
    let origins = if origins.is_empty() {
        "none".to_string()
    } else {
        origins.join(",")
    };
    match stats() {
        None => format!("net_worker=none net_origins={origins}"),
        Some(s) => format!(
            "net_threads={} net_submitted={} net_completed={} net_failed={} net_cancelled={} net_queued={} net_max_depth={} net_origins={origins}",
            s.threads_spawned, s.submitted, s.completed, s.failed, s.cancelled, s.queued, s.max_depth
        ),
    }
}
