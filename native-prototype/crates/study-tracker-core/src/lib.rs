//! Renderer-independent Study Tracker domain logic.
//!
//! This crate must remain free of GUI, WebView, networking, async runtime,
//! persistence I/O, and platform API dependencies.

pub mod academic;
pub mod appearance;
pub mod break_room;
pub mod dashboard;
pub mod timer;
