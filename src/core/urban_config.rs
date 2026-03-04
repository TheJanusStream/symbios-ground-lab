use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use symbios_tensor::{RoadGraph, TensorConfig};

/// Configuration for tensor-field urban road generation.
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct UrbanConfig {
    pub enabled: bool,
    pub tensor: TensorConfig,
    pub road_width: f32,
    pub show_gizmos: bool,
}

impl Default for UrbanConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tensor: TensorConfig::default(),
            road_width: 2.0,
            show_gizmos: true,
        }
    }
}

/// Holds the most recently generated road graph for gizmo rendering.
#[derive(Resource, Default)]
pub struct CurrentRoadGraph(pub Option<RoadGraph>);
