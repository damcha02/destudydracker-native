//! The network worker (Stage 22a, brief §7, §39, §51): one thread, two bounded queues.
//!
//! - **One thread**, spawned lazily by the first request. With no Social identity nothing is
//!   ever submitted, so the thread does not exist (and no socket is opened).
//! - **Bounded queues**: API calls (cap [`API_QUEUE_CAP`]) always run before image fetches (cap
//!   [`IMAGE_QUEUE_CAP`]). A full API queue refuses the new request (`NetError::Cancelled`,
//!   reported to the caller at once); a full image queue drops its *oldest* waiting fetch (the
//!   one least likely to still be on screen), whose reply receives `Cancelled`.
//! - **Cancellation**: a request may carry a [`CancelToken`]; a cancelled request that has not
//!   started is skipped, and one that finishes after cancellation reports `Cancelled` instead of
//!   its result. (Navigation cancels; the controllers *also* discard stale replies by generation.)
//! - **Delivery**: the reply closure runs on the worker thread; the app's closures only hand the
//!   result to `slint::invoke_from_event_loop`, so all state changes happen on the UI thread.
//! - **Shutdown**: queued work is dropped, the thread is woken and joined if it finishes within
//!   [`SHUTDOWN_WAIT`]; a request already on the wire can take at most its own timeout, and the
//!   process does not wait for it beyond that bound (the thread ends with the process).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use super::endpoint::Origin;
use super::http::{ApiRequest, HttpResponse, NetError, Priority};
use super::transport::Transport;

pub const API_QUEUE_CAP: usize = 32;
pub const IMAGE_QUEUE_CAP: usize = 48;
pub const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

pub type Reply = Box<dyn FnOnce(Result<HttpResponse, NetError>) + Send>;

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub struct Job {
    pub request: ApiRequest,
    pub cancel: Option<CancelToken>,
    pub reply: Reply,
    /// Stage 22b: bounded local work instead of a request (decoding/encoding a picked image off
    /// the UI thread). It runs in the API queue on this same thread; `reply` is not called (the
    /// closure delivers its own result). Never network I/O.
    pub local: Option<Box<dyn FnOnce() + Send>>,
}

impl Job {
    fn cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(CancelToken::is_cancelled)
    }
}

#[derive(Default)]
struct Queues {
    api: VecDeque<Job>,
    images: VecDeque<Job>,
    shutdown: bool,
    exited: bool,
}

#[derive(Default)]
pub struct Stats {
    pub submitted: AtomicU64,
    pub completed: AtomicU64,
    pub failed: AtomicU64,
    pub cancelled: AtomicU64,
    pub refused_full: AtomicU64,
    pub dropped_images: AtomicU64,
    pub threads_spawned: AtomicU64,
    pub max_depth: AtomicU64,
    pub in_flight: AtomicU64,
}

/// A snapshot for diagnostics / stress assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatsSnapshot {
    pub submitted: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub refused_full: u64,
    pub dropped_images: u64,
    pub threads_spawned: u64,
    pub max_depth: u64,
    pub queued: u64,
    pub in_flight: u64,
}

struct Shared {
    queues: Mutex<Queues>,
    wake: Condvar,
    exited: Condvar,
    stats: Stats,
}

pub struct NetWorker {
    shared: Arc<Shared>,
    transport: Arc<dyn Transport>,
    origin: Origin,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl NetWorker {
    /// Nothing starts here: the thread is spawned by the first `submit`.
    pub fn new(transport: Arc<dyn Transport>, origin: Origin) -> Self {
        Self {
            shared: Arc::new(Shared {
                queues: Mutex::new(Queues::default()),
                wake: Condvar::new(),
                exited: Condvar::new(),
                stats: Stats::default(),
            }),
            transport,
            origin,
            thread: Mutex::new(None),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_started(&self) -> bool {
        self.thread.lock().map(|t| t.is_some()).unwrap_or(false)
    }

    pub fn stats(&self) -> StatsSnapshot {
        let s = &self.shared.stats;
        let queued = self
            .shared
            .queues
            .lock()
            .map(|q| (q.api.len() + q.images.len()) as u64)
            .unwrap_or(0);
        StatsSnapshot {
            submitted: s.submitted.load(Ordering::Relaxed),
            completed: s.completed.load(Ordering::Relaxed),
            failed: s.failed.load(Ordering::Relaxed),
            cancelled: s.cancelled.load(Ordering::Relaxed),
            refused_full: s.refused_full.load(Ordering::Relaxed),
            dropped_images: s.dropped_images.load(Ordering::Relaxed),
            threads_spawned: s.threads_spawned.load(Ordering::Relaxed),
            max_depth: s.max_depth.load(Ordering::Relaxed),
            queued,
            in_flight: s.in_flight.load(Ordering::Relaxed),
        }
    }

    /// Queues a request. Never blocks on the network. `Err(Cancelled)` when the API queue is full
    /// or the worker is shutting down (the job's reply is then dropped, not called).
    pub fn submit(&self, job: Job) -> Result<(), NetError> {
        let mut dropped: Option<Job> = None;
        {
            let mut q = self.shared.queues.lock().map_err(|_| NetError::Cancelled)?;
            if q.shutdown {
                return Err(NetError::Cancelled);
            }
            match job.request.priority {
                Priority::Api => {
                    if q.api.len() >= API_QUEUE_CAP {
                        self.shared
                            .stats
                            .refused_full
                            .fetch_add(1, Ordering::Relaxed);
                        return Err(NetError::Cancelled);
                    }
                    q.api.push_back(job);
                }
                Priority::Image => {
                    if q.images.len() >= IMAGE_QUEUE_CAP {
                        dropped = q.images.pop_front();
                    }
                    q.images.push_back(job);
                }
            }
            let depth = (q.api.len() + q.images.len()) as u64;
            self.shared
                .stats
                .max_depth
                .fetch_max(depth, Ordering::Relaxed);
            self.shared.stats.submitted.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(old) = dropped {
            self.shared
                .stats
                .dropped_images
                .fetch_add(1, Ordering::Relaxed);
            (old.reply)(Err(NetError::Cancelled));
        }
        self.ensure_thread();
        self.shared.wake.notify_one();
        Ok(())
    }

    fn ensure_thread(&self) {
        let Ok(mut slot) = self.thread.lock() else {
            return;
        };
        if slot.is_some() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let transport = Arc::clone(&self.transport);
        let origin = self.origin.clone();
        match std::thread::Builder::new()
            .name("social-net".into())
            .spawn(move || run(shared, transport, origin))
        {
            Ok(handle) => {
                self.shared
                    .stats
                    .threads_spawned
                    .fetch_add(1, Ordering::Relaxed);
                *slot = Some(handle);
            }
            Err(error) => log::warn!("network: could not start the worker thread: {error}"),
        }
    }

    /// Drops queued work, stops the thread and waits for it (bounded). Idempotent.
    pub fn shutdown(&self) {
        let pending = {
            let Ok(mut q) = self.shared.queues.lock() else {
                return;
            };
            q.shutdown = true;
            let mut pending: Vec<Job> = q.api.drain(..).collect();
            pending.extend(q.images.drain(..));
            pending
        };
        // replies are dropped unanswered: the UI is going away
        self.shared
            .stats
            .cancelled
            .fetch_add(pending.len() as u64, Ordering::Relaxed);
        drop(pending);
        self.shared.wake.notify_all();
        let Some(handle) = self.thread.lock().ok().and_then(|mut t| t.take()) else {
            return;
        };
        let finished = {
            let q = self.shared.queues.lock();
            match q {
                Ok(q) => self
                    .shared
                    .exited
                    .wait_timeout_while(q, SHUTDOWN_WAIT, |q| !q.exited)
                    .map(|(q, _)| q.exited)
                    .unwrap_or(false),
                Err(_) => false,
            }
        };
        if finished {
            let _ = handle.join();
            log::info!("network: worker thread stopped");
        } else {
            log::warn!(
                "network: a request was still on the wire at shutdown; not waiting beyond {SHUTDOWN_WAIT:?}"
            );
        }
    }
}

impl Drop for NetWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(shared: Arc<Shared>, transport: Arc<dyn Transport>, origin: Origin) {
    loop {
        let job = {
            let Ok(mut q) = shared.queues.lock() else {
                break;
            };
            loop {
                if q.shutdown {
                    q.exited = true;
                    shared.exited.notify_all();
                    return;
                }
                if let Some(job) = q.api.pop_front().or_else(|| q.images.pop_front()) {
                    break job;
                }
                q = match shared.wake.wait(q) {
                    Ok(q) => q,
                    Err(_) => return,
                };
            }
        };
        if job.cancelled() {
            shared.stats.cancelled.fetch_add(1, Ordering::Relaxed);
            (job.reply)(Err(NetError::Cancelled));
            continue;
        }
        if let Some(work) = job.local {
            shared.stats.in_flight.store(1, Ordering::Relaxed);
            work();
            shared.stats.in_flight.store(0, Ordering::Relaxed);
            shared.stats.completed.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        shared.stats.in_flight.store(1, Ordering::Relaxed);
        let result = transport.execute(&origin, &job.request);
        shared.stats.in_flight.store(0, Ordering::Relaxed);
        let result = if job.cancelled() {
            shared.stats.cancelled.fetch_add(1, Ordering::Relaxed);
            Err(NetError::Cancelled)
        } else {
            match &result {
                Ok(_) => shared.stats.completed.fetch_add(1, Ordering::Relaxed),
                Err(_) => shared.stats.failed.fetch_add(1, Ordering::Relaxed),
            };
            result
        };
        (job.reply)(result);
    }
    if let Ok(mut q) = shared.queues.lock() {
        q.exited = true;
    }
    shared.exited.notify_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::http::{ApiPath, Body, Method, Target};
    use std::sync::mpsc;

    /// A transport that answers from a closure, optionally waiting on a gate first.
    struct Fake {
        gate: Mutex<Option<mpsc::Receiver<()>>>,
        calls: AtomicU64,
    }

    impl Transport for Fake {
        fn execute(&self, _: &Origin, request: &ApiRequest) -> Result<HttpResponse, NetError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = self.gate.lock().unwrap().as_ref() {
                let _ = gate.recv_timeout(Duration::from_secs(5));
            }
            Ok(HttpResponse {
                status: 200,
                content_type: None,
                body: request.log_path().into_bytes(),
            })
        }
    }

    fn origin() -> Origin {
        Origin {
            secure: false,
            host: "127.0.0.1".into(),
            port: 1,
        }
    }

    fn req(priority: Priority) -> ApiRequest {
        ApiRequest {
            method: Method::Post,
            target: Target::Api(ApiPath::Presence),
            query: vec![],
            body: Body::None,
            max_response: 100,
            timeout: Duration::from_secs(1),
            priority,
        }
    }

    fn job(
        priority: Priority,
        tx: &mpsc::Sender<(usize, Result<HttpResponse, NetError>)>,
        n: usize,
    ) -> Job {
        let tx = tx.clone();
        Job {
            request: req(priority),
            cancel: None,
            reply: Box::new(move |r| {
                let _ = tx.send((n, r));
            }),
            local: None,
        }
    }

    #[test]
    fn no_thread_exists_until_the_first_request() {
        let w = NetWorker::new(
            Arc::new(Fake {
                gate: Mutex::new(None),
                calls: AtomicU64::new(0),
            }),
            origin(),
        );
        assert!(!w.is_started());
        assert_eq!(w.stats().threads_spawned, 0);
        let (tx, rx) = mpsc::channel();
        w.submit(job(Priority::Api, &tx, 0)).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().1.is_ok());
        assert!(w.is_started());
        for i in 1..50 {
            w.submit(job(Priority::Api, &tx, i)).unwrap();
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert_eq!(w.stats().threads_spawned, 1, "one thread for every request");
        w.shutdown();
        assert!(!w.is_started());
    }

    #[test]
    fn api_requests_run_before_images_and_queues_are_bounded() {
        let (gate_tx, gate_rx) = mpsc::channel();
        let fake = Arc::new(Fake {
            gate: Mutex::new(Some(gate_rx)),
            calls: AtomicU64::new(0),
        });
        let w = NetWorker::new(fake.clone(), origin());
        let (tx, rx) = mpsc::channel();
        // the first job occupies the thread (held at the gate)
        w.submit(job(Priority::Image, &tx, 999)).unwrap();
        while fake.calls.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
        }
        for i in 0..IMAGE_QUEUE_CAP + 5 {
            w.submit(job(Priority::Image, &tx, 100 + i)).unwrap();
        }
        // the five oldest waiting images were dropped with Cancelled, immediately
        let mut dropped = Vec::new();
        for _ in 0..5 {
            let (n, r) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(r, Err(NetError::Cancelled));
            dropped.push(n);
        }
        assert_eq!(dropped, [100, 101, 102, 103, 104]);
        for i in 0..API_QUEUE_CAP {
            w.submit(job(Priority::Api, &tx, i)).unwrap();
        }
        assert_eq!(
            w.submit(job(Priority::Api, &tx, 77)),
            Err(NetError::Cancelled),
            "full API queue refuses"
        );
        assert_eq!(w.stats().refused_full, 1);
        // release everything
        for _ in 0..(1 + API_QUEUE_CAP + IMAGE_QUEUE_CAP) {
            gate_tx.send(()).unwrap();
        }
        let order: Vec<usize> = (0..(1 + API_QUEUE_CAP + IMAGE_QUEUE_CAP))
            .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap().0)
            .collect();
        assert_eq!(order[0], 999);
        assert!(
            order[1..=API_QUEUE_CAP].iter().all(|n| *n < 100),
            "API first: {order:?}"
        );
        assert!(order[API_QUEUE_CAP + 1..].iter().all(|n| *n >= 105));
        let s = w.stats();
        assert_eq!((s.threads_spawned, s.queued, s.dropped_images), (1, 0, 5));
        assert!(s.max_depth <= (API_QUEUE_CAP + IMAGE_QUEUE_CAP) as u64);
    }

    #[test]
    fn cancelled_jobs_are_skipped_or_reported_cancelled() {
        let w = NetWorker::new(
            Arc::new(Fake {
                gate: Mutex::new(None),
                calls: AtomicU64::new(0),
            }),
            origin(),
        );
        let (tx, rx) = mpsc::channel();
        let token = CancelToken::new();
        token.cancel();
        let mut j = job(Priority::Api, &tx, 1);
        j.cancel = Some(token);
        w.submit(j).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap().1,
            Err(NetError::Cancelled)
        );
    }

    #[test]
    fn shutdown_is_bounded_idempotent_and_refuses_new_work() {
        for _ in 0..200 {
            let w = NetWorker::new(
                Arc::new(Fake {
                    gate: Mutex::new(None),
                    calls: AtomicU64::new(0),
                }),
                origin(),
            );
            let (tx, rx) = mpsc::channel();
            w.submit(job(Priority::Api, &tx, 0)).unwrap();
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let started = std::time::Instant::now();
            w.shutdown();
            assert!(started.elapsed() < SHUTDOWN_WAIT);
            w.shutdown();
            assert_eq!(
                w.submit(job(Priority::Api, &tx, 1)),
                Err(NetError::Cancelled)
            );
        }
    }

    #[test]
    fn a_request_stuck_on_the_wire_does_not_hang_shutdown_beyond_the_bound() {
        let (_gate_tx, gate_rx) = mpsc::channel::<()>();
        let fake = Arc::new(Fake {
            gate: Mutex::new(Some(gate_rx)),
            calls: AtomicU64::new(0),
        });
        let w = NetWorker::new(fake.clone(), origin());
        let (tx, _rx) = mpsc::channel();
        w.submit(job(Priority::Api, &tx, 0)).unwrap();
        while fake.calls.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
        }
        let started = std::time::Instant::now();
        w.shutdown();
        let waited = started.elapsed();
        assert!(
            waited >= SHUTDOWN_WAIT && waited < SHUTDOWN_WAIT + Duration::from_secs(2),
            "{waited:?}"
        );
    }
}
