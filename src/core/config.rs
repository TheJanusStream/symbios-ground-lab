use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::tasks::Task;
use rand_pcg::Pcg64Mcg;
use serde::{Deserialize, Serialize};
use symbios_ground::HeightMap;

/// Which base terrain generation algorithm to use.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum GeneratorKind {
    /// Fractional Brownian Motion: layers of coherent noise summed across
    /// multiple octaves, each scaled by persistence and lacunarity. Produces
    /// smooth, naturalistic terrain with controllable detail and roughness.
    #[default]
    FbmNoise,
    /// Diamond-Square mid-point displacement. Recursively subdivides the grid
    /// and perturbs mid-points by a decreasing random offset, producing
    /// realistic ridge-and-valley topology. The `ds_roughness` parameter
    /// controls how quickly the perturbation amplitude shrinks across levels.
    DiamondSquare,
    /// Voronoi-cell terracing. Assigns each grid cell to its nearest seed
    /// point and quantises the resulting distance field into a fixed number of
    /// stepped terraces, producing a stylised, plateaued appearance.
    VoronoiTerracing,
}

/// All user-facing terrain generation parameters.
/// This is the single source of truth – the UI is a reflection of this state.
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct TerrainConfig {
    // Grid / world
    pub grid_size: u32,
    pub cell_scale: f32,
    pub height_scale: f32,

    // Algorithm selection
    pub generator_kind: GeneratorKind,

    // Seed shared by all generators
    pub seed: u64,

    // FBM noise params
    pub octaves: u32,
    pub persistence: f32,
    pub lacunarity: f32,
    pub base_frequency: f32,

    // Diamond Square params
    pub ds_roughness: f32,

    // Voronoi Terracing params
    pub voronoi_num_seeds: u32,
    pub voronoi_num_terraces: u32,

    // Hydraulic erosion
    pub erosion_enabled: bool,
    pub erosion_drops: u32,
    pub inertia: f32,
    pub erosion_rate: f32,
    pub deposition_rate: f32,
    pub evaporation_rate: f32,
    pub capacity_factor: f32,

    // Thermal erosion
    pub thermal_enabled: bool,
    pub thermal_iterations: u32,
    pub thermal_talus_angle: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            grid_size: 512,
            cell_scale: 1.0,
            height_scale: 40.0,

            seed: 42,
            octaves: 6,
            persistence: 0.5,
            lacunarity: 2.0,
            base_frequency: 4.0,

            generator_kind: GeneratorKind::FbmNoise,

            ds_roughness: 0.5,

            voronoi_num_seeds: 64,
            voronoi_num_terraces: 8,

            erosion_enabled: true,
            erosion_drops: 50_000,
            inertia: 0.05,
            erosion_rate: 0.3,
            deposition_rate: 0.3,
            evaporation_rate: 0.02,
            capacity_factor: 8.0,

            thermal_enabled: true,
            thermal_iterations: 1,
            thermal_talus_angle: 0.05,
        }
    }
}

/// Signals that the terrain must be re-generated.
#[derive(Resource, Default)]
pub struct DirtyFlags {
    pub terrain: bool,
}

/// Debounce timer so rapid UI changes don't spam generation.
#[derive(Resource)]
pub struct TerrainDebounce {
    pub pending: bool,
    pub timer: Timer,
}

impl Default for TerrainDebounce {
    fn default() -> Self {
        Self {
            pending: false,
            timer: Timer::from_seconds(0.4, TimerMode::Once),
        }
    }
}

/// Holds the most recently generated heightmap, ready for meshing.
#[derive(Resource, Default)]
pub struct CurrentHeightMap(pub Option<HeightMap>);

/// Async generation task in-flight.
#[derive(Resource, Default)]
pub struct GenerationTask(pub Option<Task<(HeightMap, Option<symbios_tensor::RoadGraph>)>>);

/// Signals that the terrain mesh needs to be rebuilt from `CurrentHeightMap`.
#[derive(Resource, Default)]
pub struct DirtyMesh(pub bool);

/// Status message shown in the UI for exports.
#[derive(Resource, Default)]
pub enum ExportStatus {
    #[default]
    Idle,
    /// A heavy export (e.g. OBJ) is running on a background thread.
    Exporting,
    Done(String),
    Error(String),
}

/// Async OBJ export task in-flight (mirrors the GenerationTask pattern).
#[derive(Resource, Default)]
pub struct ExportTask(pub Option<bevy::tasks::Task<Result<String, String>>>);

// ---------------------------------------------------------------------------
// Erosion visualisation
// ---------------------------------------------------------------------------

/// State for the real-time droplet visualisation (issue #5).
#[derive(Resource)]
pub struct ErosionVizState {
    /// Whether the step-by-step visualisation is active.
    pub enabled: bool,
    /// In-flight async task that generates the base (uneroded) heightmap before
    /// the visualisation can start.  `None` once init completes or is cancelled.
    pub init_task: Option<Task<HeightMap>>,
    /// Tasks that were detached by "Stop Viz" while still running.  Dropping a
    /// `Task` handle does NOT cancel the underlying thread; the CPU work keeps
    /// running until the pool slot is released naturally.  Keeping the handles
    /// here lets `poll_viz_init` drain them to completion before a new
    /// visualisation is allowed to start, capping concurrent init tasks at one.
    pub abandoned_init_tasks: Vec<Task<HeightMap>>,
    /// Working copy of the heightmap being eroded in real-time.
    pub heightmap: Option<HeightMap>,
    /// Deterministic RNG for spawning droplets.
    pub rng: Pcg64Mcg,
    /// Active droplets being stepped this frame.
    pub active: Vec<VizDroplet>,
    /// How many drops have been completed so far.
    pub completed: u32,
    /// Total drops to simulate (copied from config when visualisation starts).
    pub total: u32,
    /// Steps simulated per frame per droplet batch.
    pub steps_per_frame: u32,
    /// Drops spawned per frame.
    pub drops_per_frame: u32,
    /// Publish the heightmap snapshot to `CurrentHeightMap` every N frames to
    /// avoid triggering a full splat-map rebuild on every single frame.
    pub publish_every_n_frames: u32,
    /// Frame counter for the publish throttle.
    pub frame_counter: u32,
    /// Cached config copy for erosion parameters.
    pub config: super::config::TerrainConfig,
}

impl Default for ErosionVizState {
    fn default() -> Self {
        use rand::SeedableRng;
        Self {
            enabled: false,
            init_task: None,
            abandoned_init_tasks: Vec::new(),
            heightmap: None,
            rng: Pcg64Mcg::seed_from_u64(0),
            active: Vec::new(),
            completed: 0,
            total: 0,
            steps_per_frame: 8,
            drops_per_frame: 32,
            publish_every_n_frames: 15,
            frame_counter: 0,
            config: TerrainConfig::default(),
        }
    }
}

/// A single in-flight droplet during visualisation.
#[derive(Clone)]
pub struct VizDroplet {
    pub px: f32,
    pub pz: f32,
    pub dir_x: f32,
    pub dir_z: f32,
    pub vel: f32,
    pub water: f32,
    pub sediment: f32,
    pub steps_left: u32,
    /// Recent positions for gizmo trail.
    pub trail: VecDeque<Vec2>,
}
