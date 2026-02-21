//! egui-based control panels.
//!
//! - [`panel`] — the main "Terraformer" window: grid/world settings, algorithm
//!   selection, erosion parameters, export controls, and the erosion visualisation
//!   trigger.
//! - [`material_panel`] — the "Materials" window: enable/disable splat materials,
//!   texture resolution, tile scale, and per-layer splat rules + texture generator
//!   parameters.

pub mod material_panel;
pub mod panel;
