//! Symbios Ground Lab — interactive 3D terrain generator and editor.
//!
//! The crate is split into four top-level modules that map directly to the
//! application's layers of concern:
//!
//! - [`core`] — shared resources, configuration types, and runtime state.
//! - [`logic`] — terrain generation and erosion simulation systems.
//! - [`ui`] — egui-based control panels.
//! - [`visuals`] — Bevy scene setup, mesh management, material pipeline, and export.
//!
//! Entry point: [`main`](../symbios_ground_lab/fn.main.html) in `src/main.rs`.

pub mod core;
pub mod logic;
pub mod ui;
pub mod visuals;
