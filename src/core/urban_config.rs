use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use symbios_tensor::{BuildingLot, LotConfig, RoadGraph, TensorConfig};

/// Configuration for tensor-field urban road generation.
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct UrbanConfig {
    pub enabled: bool,
    pub tensor: TensorConfig,
    pub road_width: f32,
    pub lot: LotConfig,
    pub show_gizmos: bool,
    pub show_block_gizmos: bool,
    pub show_block_centroids: bool,
    pub show_lot_gizmos: bool,
}

impl Default for UrbanConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tensor: TensorConfig::default(),
            road_width: 2.0,
            lot: LotConfig::default(),
            show_gizmos: true,
            show_block_gizmos: true,
            show_block_centroids: true,
            show_lot_gizmos: true,
        }
    }
}

/// Holds the most recently generated road graph for gizmo rendering.
#[derive(Resource, Default)]
pub struct CurrentRoadGraph(pub Option<RoadGraph>);

/// Holds the most recently extracted building lots.
#[derive(Resource, Default)]
pub struct CurrentBuildingLots(pub Vec<BuildingLot>);
