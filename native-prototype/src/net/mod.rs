//! Social / Daily Skribbl networking (Stage 22a). Application layer only: `study-tracker-core`
//! never sees a URL, a header, a socket or a thread.
//!
//! ```text
//! Slint callback ─► controller (UI thread) ─► NetWorker::submit (bounded queue, never blocks)
//!                                                  │ one worker thread, lazily started
//!                                                  ▼
//!                                         Transport (ureq + rustls) ── HTTPS ──► Worker
//!                                                  │ typed result, validated DTO
//!                                                  ▼
//!                       slint::invoke_from_event_loop ─► controller ─► core state ─► UI push
//! ```
//!
//! - [`endpoint`]: the only place an origin exists. `Production` (one constant) or
//!   `Test(LocalEndpoint)`, which can only be a loopback address. No server payload can choose
//!   where a request goes.
//! - [`http`]: request/response values, the error model and log redaction.
//! - [`transport`]: the ureq/rustls client (finite timeouts, no redirects, bounded bodies,
//!   normal certificate validation) and, in test builds, a structural loopback-only guard.
//! - [`worker`]: the single network thread and its bounded queues.
//! - [`social_api`]: the 22a operations - typed wire DTOs, request builders, validation.
//! - [`images`]: the server-image URL allow-list, bounded fetch and decode, bounded cache.

pub mod device;
pub mod endpoint;
pub mod http;
pub mod images;
pub mod multipart;
pub mod social_api;
pub mod transport;
pub mod worker;

#[cfg(test)]
mod mock_conformance_tests;
