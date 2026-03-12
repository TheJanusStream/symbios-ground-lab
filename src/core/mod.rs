//! Shared resources, configuration types, and runtime state.
//!
//! - [`config`] — [`TerrainConfig`](config::TerrainConfig), dirty flags,
//!   [`ErosionVizState`](config::ErosionVizState), and all other ECS resources
//!   that cross module boundaries.
//! - [`material_config`] — [`MaterialConfig`](material_config::MaterialConfig)
//!   (splat rules + texture parameters) and
//!   [`MaterialState`](material_config::MaterialState) (async pipeline progress).
//! - [`urban_config`] — [`UrbanConfig`](urban_config::UrbanConfig) (tensor-field
//!   road generation, lot subdivision, road rendering) and associated runtime
//!   state ([`CurrentRoadGraph`](urban_config::CurrentRoadGraph),
//!   [`CurrentBuildingLots`](urban_config::CurrentBuildingLots),
//!   [`RoadMaterialState`](urban_config::RoadMaterialState)).
//! - [`architecture_config`] — [`ArchitectureConfig`](architecture_config::ArchitectureConfig)
//!   (CGA grammar, building material configs) and
//!   [`ArchitectureMaterialState`](architecture_config::ArchitectureMaterialState)
//!   (debounced texture regeneration).

pub mod architecture_config;
pub mod config;
pub mod material_config;
pub mod urban_config;
