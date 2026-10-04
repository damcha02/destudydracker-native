//! Break Room / Rest (Stage 20; Travle Stage 21): production v0.1.67's game catalog, unlock economy, pet rock,
//! achievements and the local games' rules, as renderer-independent pure Rust.
//!
//! Nothing here reads the system clock, the timezone, a random source or the filesystem: dates
//! arrive as [`crate::dashboard::civil::CivilDate`]s / ISO strings, local time through
//! [`crate::dashboard::civil::LocalClock`], randomness as explicit numbers or seed salts, and the
//! word/country tables are compiled in (`data/break_room/`, extracted verbatim from production).
//! See `native-prototype/docs/stage20-break-room.md`.

pub mod achievements;
pub mod catalog;
pub mod countries;
pub mod daily;
pub mod durak;
pub mod flaggle;
pub mod geodle;
pub mod rest;
pub mod skribbl;
pub mod state;
pub mod travle;
pub mod wordle;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod travle_tests;
