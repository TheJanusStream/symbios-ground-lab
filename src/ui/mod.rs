//! egui-based control panels.
//!
//! - [`panel`] — the main "Terraformer" window: grid/world settings, algorithm
//!   selection, erosion parameters, export controls, and the erosion visualisation
//!   trigger.
//! - [`material_panel`] — the "Materials" window: enable/disable splat materials,
//!   texture resolution, tile scale, and per-layer splat rules + texture generator
//!   parameters.
//! - [`urban_panel`] — the "Urban Planner" window: tensor-field road generation
//!   parameters, block and lot subdivision settings, road rendering options, and
//!   road material configuration.
//! - [`architecture_panel`] — the "Architect" window: CGA grammar editor,
//!   max-buildings slider, and per-material texture configuration for building
//!   facades (brick, stucco, concrete, shingle, wood, glass, metal).

pub mod architecture_panel;
pub mod material_panel;
pub mod panel;
pub mod urban_panel;
