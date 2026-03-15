//! Urban generation configuration and runtime state.
//!
//! Holds [`UrbanConfig`] (tensor-field road parameters, lot subdivision rules,
//! road rendering options) and the runtime resources that carry the generated
//! road graph and building lots between systems.

use bevy::prelude::*;
use bevy::time::Timer;
use bevy_symbios_texture::asphalt::AsphaltConfig;
use serde::{Deserialize, Serialize};
use symbios_tensor::{
    BuildingLot, LotConfig, RationalizeConfig, RoadGraph, RoadMeshConfig, SkirtConfig, TensorConfig,
};

/// Configuration for tensor-field urban road generation.
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct UrbanConfig {
    pub enabled: bool,
    pub tensor: TensorConfig,
    /// Half-width of major roads (world units).
    pub major_half_width: f32,
    /// Half-width of minor roads (world units).
    pub minor_half_width: f32,
    /// Extra radius added to intersection hubs beyond the road half-width.
    pub curb_radius: f32,
    pub lot: LotConfig,
    pub lot_blend_radius: f32,
    pub road_blend_radius: f32,
    pub show_gizmos: bool,
    pub show_block_gizmos: bool,
    pub show_block_centroids: bool,
    pub show_lot_gizmos: bool,
    /// Render 3D road meshes (hubs + ribbons).
    pub render_roads: bool,
    /// Spline sampling density (subdivisions per graph edge).
    pub road_resolution: f32,
    /// Number of polygon sides for intersection hubs (e.g. 8 = octagon).
    pub hub_segments: u32,
    /// Material configuration for road surface texture.
    pub road_material: AsphaltConfig,
    /// Graph rationalization (RDP straightening + fillet smoothing).
    pub rationalize: RationalizeConfig,
    /// Width of embankment skirts extending from road edges (world units).
    pub skirt_width: f32,
    /// How far below terrain surface the skirt buries itself.
    pub skirt_bury_depth: f32,
}

impl Default for UrbanConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tensor: TensorConfig::default(),
            major_half_width: 3.0,
            minor_half_width: 2.0,
            curb_radius: 2.0,
            lot: LotConfig {
                max_lot_area: 1200.0,
                min_lot_area: 300.0,
                front_setback: 3.0,
                side_setback: 2.0,
                rear_setback: 4.0,
                min_width: 12.0,
                min_depth: 12.0,
            },
            lot_blend_radius: 10.0,
            road_blend_radius: 10.0,
            show_gizmos: true,
            show_block_gizmos: true,
            show_block_centroids: true,
            show_lot_gizmos: true,
            render_roads: true,
            road_resolution: 8.0,
            hub_segments: 8,
            road_material: AsphaltConfig::default(),
            rationalize: RationalizeConfig::default(),
            skirt_width: 3.0,
            skirt_bury_depth: 0.5,
        }
    }
}

impl UrbanConfig {
    /// Build a [`RoadMeshConfig`] from the current settings.
    pub fn road_mesh_config(&self) -> RoadMeshConfig {
        RoadMeshConfig {
            major_half_width: self.major_half_width,
            minor_half_width: self.minor_half_width,
            hub_sides: self.hub_segments,
            depth_bias: 0.05,
            texture_scale: 0.1,
            spline_subdivisions: self.road_resolution as u32,
            curb_radius: self.curb_radius,
            skirt: SkirtConfig {
                width: self.skirt_width,
                bury_depth: self.skirt_bury_depth,
            },
        }
    }
}

/// Runtime state for debounced road-material texture regeneration.
#[derive(Resource)]
pub struct RoadMaterialState {
    pub textures_dirty: bool,
    pub texture_debounce_pending: bool,
    pub texture_debounce_timer: Timer,
}

impl Default for RoadMaterialState {
    fn default() -> Self {
        Self {
            textures_dirty: false,
            texture_debounce_pending: false,
            texture_debounce_timer: Timer::from_seconds(0.3, TimerMode::Once),
        }
    }
}

/// Holds the most recently generated road graph for gizmo rendering.
#[derive(Resource, Default)]
pub struct CurrentRoadGraph(pub Option<RoadGraph>);

/// Holds the most recently extracted building lots.
#[derive(Resource, Default)]
pub struct CurrentBuildingLots(pub Vec<BuildingLot>);
