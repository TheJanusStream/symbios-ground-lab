use bevy::prelude::*;
use bevy::time::Timer;
use bevy_symbios_texture::asphalt::AsphaltConfig;
use serde::{Deserialize, Serialize};
use symbios_tensor::{BuildingLot, LotConfig, RoadGraph, TensorConfig};

/// Configuration for tensor-field urban road generation.
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct UrbanConfig {
    pub enabled: bool,
    pub tensor: TensorConfig,
    pub road_width: f32,
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
}

impl Default for UrbanConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tensor: TensorConfig::default(),
            road_width: 2.0,
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
