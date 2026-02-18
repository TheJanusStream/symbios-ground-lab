use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::tasks::futures_lite::future;
use symbios_ground::HeightMap;
use symbios_ground::{FbmNoise, HydraulicErosion, TerrainGenerator, ThermalErosion};

use crate::core::config::{CurrentHeightMap, DirtyFlags, DirtyMesh, GenerationTask, TerrainConfig};

/// Spawns an async task to generate the terrain when `DirtyFlags::terrain` is set.
pub fn start_generation(
    config: Res<TerrainConfig>,
    mut dirty: ResMut<DirtyFlags>,
    mut task: ResMut<GenerationTask>,
) {
    if !dirty.terrain || task.0.is_some() {
        return;
    }
    dirty.terrain = false;

    let cfg = config.clone();
    let pool = AsyncComputeTaskPool::get();
    let t = pool.spawn(async move { generate_heightmap(&cfg) });
    task.0 = Some(t);
}

/// Polls the async generation task and, when done, stores the result.
pub fn poll_generation(
    mut task: ResMut<GenerationTask>,
    mut current_hm: ResMut<CurrentHeightMap>,
    mut dirty_mesh: ResMut<DirtyMesh>,
) {
    let Some(ref mut t) = task.0 else { return };
    if let Some(hm) = future::block_on(future::poll_once(t)) {
        current_hm.0 = Some(hm);
        dirty_mesh.0 = true;
        task.0 = None;
    }
}

// ---------------------------------------------------------------------------
// Pure terrain generation (runs on a thread pool worker)
// ---------------------------------------------------------------------------

pub fn generate_heightmap(cfg: &TerrainConfig) -> HeightMap {
    let size = (cfg.grid_size as usize).max(2);
    let mut hm = HeightMap::new(size, size, cfg.cell_scale);

    // FBM noise
    let fbm = FbmNoise {
        seed: cfg.seed,
        octaves: cfg.octaves,
        persistence: cfg.persistence,
        lacunarity: cfg.lacunarity,
        base_frequency: cfg.base_frequency,
    };
    fbm.generate(&mut hm);
    hm.normalize();

    // Scale to desired height
    for v in hm.data_mut() {
        *v *= cfg.height_scale;
    }

    // Hydraulic erosion
    if cfg.erosion_enabled {
        let erosion = HydraulicErosion {
            seed: cfg.seed,
            num_drops: cfg.erosion_drops,
            inertia: cfg.inertia,
            erosion_rate: cfg.erosion_rate,
            deposition_rate: cfg.deposition_rate,
            evaporation_rate: cfg.evaporation_rate,
            capacity_factor: cfg.capacity_factor,
            ..HydraulicErosion::new(cfg.seed)
        };
        erosion.erode(&mut hm);
    }

    // Thermal erosion
    if cfg.thermal_enabled {
        ThermalErosion::new()
            .with_iterations(cfg.thermal_iterations)
            .with_talus_angle(cfg.thermal_talus_angle)
            .erode(&mut hm);
    }

    hm
}
