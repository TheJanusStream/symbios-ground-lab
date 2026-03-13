use crate::core::config::{
    CurrentHeightMap, DirtyFlags, DirtyMesh, GenerationTask, GeneratorKind, TerrainConfig,
};
use crate::core::urban_config::{CurrentBuildingLots, CurrentRoadGraph, UrbanConfig};
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::tasks::futures_lite::future;
use symbios_ground::HeightMap;
use symbios_ground::{
    DiamondSquare, FbmNoise, HydraulicErosion, TerrainGenerator, ThermalErosion, VoronoiTerracing,
};

/// Spawns an async task to generate the terrain when `DirtyFlags::terrain` is set.
pub fn start_generation(
    config: Res<TerrainConfig>,
    urban_config: Res<UrbanConfig>,
    mut dirty: ResMut<DirtyFlags>,
    mut task: ResMut<GenerationTask>,
) {
    if !dirty.terrain || task.0.is_some() {
        return;
    }
    dirty.terrain = false;

    let cfg = config.clone();
    let u_cfg = urban_config.clone();
    let pool = AsyncComputeTaskPool::get();
    let t = pool.spawn(async move { generate_terrain(&cfg, &u_cfg) });
    task.0 = Some(t);
}

/// Polls the async generation task and, when done, stores the result.
pub fn poll_generation(
    mut task: ResMut<GenerationTask>,
    mut current_hm: ResMut<CurrentHeightMap>,
    mut current_rg: ResMut<CurrentRoadGraph>,
    mut current_lots: ResMut<CurrentBuildingLots>,
    mut dirty_mesh: ResMut<DirtyMesh>,
) {
    let Some(ref mut t) = task.0 else { return };
    if let Some((hm, rg, lots)) = future::block_on(future::poll_once(t)) {
        current_hm.0 = Some(hm);
        current_rg.0 = rg;
        current_lots.0 = lots;
        dirty_mesh.0 = true;
        task.0 = None;
    }
}

// ---------------------------------------------------------------------------
// Pure terrain generation (runs on a thread pool worker)
// ---------------------------------------------------------------------------

type GenerationResult = (
    HeightMap,
    Option<symbios_tensor::RoadGraph>,
    Vec<symbios_tensor::BuildingLot>,
);

/// Generates a heightmap, optionally a road graph and building lots, then
/// carves roads into the terrain before erosion passes run.
pub fn generate_terrain(cfg: &TerrainConfig, u_cfg: &UrbanConfig) -> GenerationResult {
    let (mut hm, road_graph, lots) = generate_heightmap_inner(cfg, u_cfg);

    // Calculate absolute water level based on the height scale
    let absolute_water_level = cfg.water_level * cfg.height_scale;

    // Hydraulic erosion (acts on carved terrain when roads are enabled)
    if cfg.erosion_enabled {
        let erosion = HydraulicErosion {
            seed: cfg.seed,
            num_drops: cfg.erosion_drops,
            inertia: cfg.inertia,
            erosion_rate: cfg.erosion_rate,
            deposition_rate: cfg.deposition_rate,
            evaporation_rate: cfg.evaporation_rate,
            capacity_factor: cfg.capacity_factor,
            water_level: absolute_water_level,
            ..HydraulicErosion::new(cfg.seed)
        };
        erosion.erode(&mut hm);
    }

    // Thermal erosion
    if cfg.thermal_enabled {
        ThermalErosion::new()
            .with_iterations(cfg.thermal_iterations)
            .with_talus_angle(cfg.thermal_talus_angle)
            .with_water_level(absolute_water_level)
            .with_underwater_talus_angle(0.01)
            .erode(&mut hm);
    }

    (hm, road_graph, lots)
}

/// Generates the base heightmap (pre-erosion). Used by erosion viz as well.
pub fn generate_base_heightmap(cfg: &TerrainConfig, u_cfg: &UrbanConfig) -> GenerationResult {
    generate_heightmap_inner(cfg, u_cfg)
}

fn generate_heightmap_inner(cfg: &TerrainConfig, u_cfg: &UrbanConfig) -> GenerationResult {
    let size = (cfg.grid_size as usize).max(2);
    let mut hm = HeightMap::new(size, size, cfg.cell_scale);

    // Base terrain generation — dispatched by algorithm choice
    match cfg.generator_kind {
        GeneratorKind::FbmNoise => {
            FbmNoise {
                seed: cfg.seed,
                octaves: cfg.octaves,
                persistence: cfg.persistence,
                lacunarity: cfg.lacunarity,
                base_frequency: cfg.base_frequency,
            }
            .generate(&mut hm);
            hm.normalize();
        }
        GeneratorKind::DiamondSquare => {
            DiamondSquare::new(cfg.seed, cfg.ds_roughness).generate(&mut hm);
        }
        GeneratorKind::VoronoiTerracing => {
            VoronoiTerracing::new(
                cfg.seed,
                cfg.voronoi_num_seeds.max(1) as usize,
                cfg.voronoi_num_terraces.max(1) as usize,
            )
            .generate(&mut hm);
        }
    }

    // Scale to desired height (common to all generators)
    for v in hm.data_mut() {
        *v *= cfg.height_scale;
    }

    // Urban road generation & carving (before erosion)
    let mut road_graph = None;
    let mut lots = Vec::new();
    if u_cfg.enabled {
        // 1. Convert percentage water_level to absolute world height
        let absolute_water = cfg.water_level * cfg.height_scale;

        let mut t_cfg = u_cfg.tensor.clone();
        t_cfg.water_level = absolute_water; // Inject into TensorConfig

        // 2. Trace the initial network (Water-aware)
        match symbios_tensor::generate_roads(&hm, &t_cfg) {
            Ok(mut graph) => {
                // 3. Rationalize the network (straighten & smooth)
                if u_cfg.rationalize.enabled {
                    symbios_tensor::rationalize_graph(&mut graph, &hm, &u_cfg.rationalize);
                }

                // 4. Extract Blocks and Lots
                symbios_tensor::extract_blocks(&mut graph);
                lots = symbios_tensor::extract_lots(&graph, &hm, absolute_water, &u_cfg.lot);

                // 4. SYNAPTIC PRUNING: Gut the empty roads!
                symbios_tensor::prune_unused_roads(&mut graph, &lots);

                // 5. Carve terrain (only the surviving active roads will carve!)
                let road_surface = symbios_tensor::carve_roads(
                    &graph,
                    &mut hm,
                    u_cfg.road_width,
                    u_cfg.road_blend_radius,
                );
                symbios_tensor::carve_lots(
                    &lots,
                    &mut hm,
                    u_cfg.lot_blend_radius,
                    Some(&road_surface),
                );

                road_graph = Some(graph);
            }
            Err(e) => {
                bevy::log::warn!("Road generation skipped: {e}");
            }
        }
    }

    (hm, road_graph, lots)
}
