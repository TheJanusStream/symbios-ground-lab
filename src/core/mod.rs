//! Shared resources, configuration types, and runtime state.
//!
//! - [`config`] — [`TerrainConfig`](config::TerrainConfig), dirty flags,
//!   [`ErosionVizState`](config::ErosionVizState), and all other ECS resources
//!   that cross module boundaries.
//! - [`material_config`] — [`MaterialConfig`](material_config::MaterialConfig)
//!   (splat rules + texture parameters) and
//!   [`MaterialState`](material_config::MaterialState) (async pipeline progress).

pub mod config;
pub mod material_config;
pub mod urban_config;
