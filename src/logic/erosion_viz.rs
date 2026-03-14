//! Real-time erosion visualisation.
//!
//! [`start_erosion_viz`] generates the base (uneroded) heightmap on a
//! background thread.  [`poll_viz_init`] picks up the result and enables the
//! visualisation.  [`step_erosion_viz`] runs every frame, spawning and stepping
//! droplets and publishing periodic heightmap snapshots so the terrain mesh
//! updates while the simulation runs.  On completion a thermal relaxation pass
//! is applied to match the background generator's output.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::tasks::futures_lite::future;
use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

use crate::core::config::{
    CurrentHeightMap, DirtyFlags, DirtyMesh, ErosionVizState, TerrainConfig, VizDroplet,
};
use crate::core::material_config::MaterialState;
use crate::core::urban_config::{CurrentBuildingLots, CurrentRoadGraph, UrbanConfig};
use crate::logic::generation::generate_base_heightmap;
use symbios_ground::ThermalErosion;

/// Kick off the erosion visualisation.
///
/// Generates the base (uneroded) heightmap on the `AsyncComputeTaskPool` so
/// the main thread (and therefore the UI and camera) never block.  The viz is
/// not `enabled` yet — `poll_viz_init` sets that once the task completes.
pub fn start_erosion_viz(config: &TerrainConfig, u_cfg: &UrbanConfig, state: &mut ErosionVizState) {
    let mut cfg_no_erosion = config.clone();
    cfg_no_erosion.erosion_enabled = false;
    cfg_no_erosion.thermal_enabled = false;

    let u_cfg = u_cfg.clone();
    let pool = AsyncComputeTaskPool::get();
    let t = pool.spawn(async move { generate_base_heightmap(&cfg_no_erosion, &u_cfg) });
    state.init_task = Some(t);

    // Pre-populate everything except the heightmap so the step system is ready
    // the moment the init task delivers it.
    state.heightmap = None;
    state.rng = Pcg64Mcg::seed_from_u64(config.seed);
    state.active.clear();
    state.completed = 0;
    state.total = config.erosion_drops;
    state.config = config.clone();
    state.enabled = false; // set to true by poll_viz_init once the task finishes
}

/// Poll the async base-heightmap task spawned by `start_erosion_viz`.
/// When it completes, store the result, publish it to `CurrentHeightMap` so
/// the mesh immediately snaps to the un-eroded base state (preventing droplet
/// gizmos from floating above the old eroded terrain for the first N frames),
/// and enable the visualisation.
///
/// Also drains any abandoned init tasks (tasks detached by "Stop Viz" while
/// still running). Polling them to completion here, rather than dropping the
/// handles, ensures the thread pool slot is reclaimed before a new
/// visualisation is permitted to start.
pub fn poll_viz_init(
    mut viz: ResMut<ErosionVizState>,
    mut current_hm: ResMut<CurrentHeightMap>,
    mut current_rg: ResMut<CurrentRoadGraph>,
    mut current_lots: ResMut<CurrentBuildingLots>,
    mut dirty_mesh: ResMut<DirtyMesh>,
    mut mat_state: ResMut<MaterialState>,
) {
    // Drain abandoned tasks: retain only those not yet complete.
    viz.abandoned_init_tasks
        .retain_mut(|t| future::block_on(future::poll_once(t)).is_none());

    let Some(ref mut t) = viz.init_task else {
        return;
    };
    if let Some((hm, rg, lots)) = future::block_on(future::poll_once(t)) {
        current_hm.0 = Some(hm.clone());
        current_rg.0 = rg;
        current_lots.0 = lots;
        dirty_mesh.0 = true;
        // Trigger a splat weight-map regeneration for the un-eroded base
        // heightmap. This must be set explicitly because viz.enabled is about
        // to become true, which causes detect_material_dirty to suppress
        // heightmap-triggered splat updates for the rest of the viz.
        mat_state.splat_dirty = true;
        viz.heightmap = Some(hm);
        viz.init_task = None;
        viz.enabled = true;
    }
}

/// Advance the visualisation: spawn new droplets and step existing ones.
/// Called every frame while `ErosionVizState::enabled` is true.
pub fn step_erosion_viz(
    mut viz: ResMut<ErosionVizState>,
    mut current_hm: ResMut<CurrentHeightMap>,
    mut dirty_mesh: ResMut<DirtyMesh>,
    mut dirty: ResMut<DirtyFlags>,
) {
    if !viz.enabled {
        return;
    }
    if viz.heightmap.is_none() {
        return;
    }

    // Take ownership of the heightmap so we can borrow other viz fields freely.
    let mut hm = viz.heightmap.take().unwrap();
    let w = hm.width();
    let h = hm.height();

    // Snapshot immutable config data before entering the mutable section.
    let cfg = viz.config.clone();
    let steps = viz.steps_per_frame;
    let drops_per_frame = viz.drops_per_frame;
    let water_level = cfg.water_level * cfg.height_scale;

    // Spawn new droplets (up to drops_per_frame, respecting total budget).
    // Skip submerged spawn points to match HydraulicErosion behaviour.
    let remaining = viz
        .total
        .saturating_sub(viz.completed + viz.active.len() as u32);
    let to_spawn = drops_per_frame.min(remaining);
    let mut spawned = 0u32;
    let mut attempts = 0u32;
    while spawned < to_spawn && attempts < to_spawn * 4 {
        attempts += 1;
        let px: f32 = viz.rng.random::<f32>() * (w - 1) as f32;
        let pz: f32 = viz.rng.random::<f32>() * (h - 1) as f32;
        // Check height at spawn point; skip if submerged.
        let ix = px.floor() as usize;
        let iz = pz.floor() as usize;
        if ix + 1 < w && iz + 1 < h {
            let fx = px - ix as f32;
            let fz = pz - iz as f32;
            let spawn_h = hm.get(ix, iz) * (1.0 - fx) * (1.0 - fz)
                + hm.get(ix + 1, iz) * fx * (1.0 - fz)
                + hm.get(ix, iz + 1) * (1.0 - fx) * fz
                + hm.get(ix + 1, iz + 1) * fx * fz;
            if spawn_h < water_level {
                continue;
            }
        }
        viz.active.push(VizDroplet {
            px,
            pz,
            dir_x: 0.0,
            dir_z: 0.0,
            vel: 1.0,
            water: 1.0,
            sediment: 0.0,
            steps_left: 64,
            trail: VecDeque::from([Vec2::new(px, pz)]),
        });
        spawned += 1;
    }

    // Step every active droplet.
    let mut still_alive: Vec<VizDroplet> = Vec::with_capacity(viz.active.len());
    let mut newly_completed: u32 = 0;

    for mut drop in viz.active.drain(..) {
        let mut alive = true;
        for _ in 0..steps {
            if drop.steps_left == 0 || drop.water < 0.01 {
                alive = false;
                break;
            }
            drop.steps_left -= 1;

            let ix = drop.px.floor() as usize;
            let iz = drop.pz.floor() as usize;
            // After this guard: ix <= w-2, iz <= h-2, so ix+1 and iz+1 are
            // valid indices for all bilinear reads and writes below.
            if ix + 1 >= w || iz + 1 >= h {
                alive = false;
                break;
            }
            let fx = drop.px - ix as f32;
            let fz = drop.pz - iz as f32;

            let h00 = hm.get(ix, iz);
            let h10 = hm.get(ix + 1, iz);
            let h01 = hm.get(ix, iz + 1);
            let h11 = hm.get(ix + 1, iz + 1);

            let height_here = h00 * (1.0 - fx) * (1.0 - fz)
                + h10 * fx * (1.0 - fz)
                + h01 * (1.0 - fx) * fz
                + h11 * fx * fz;
            let grad_x = (h10 - h00) * (1.0 - fz) + (h11 - h01) * fz;
            let grad_z = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

            drop.dir_x = drop.dir_x * cfg.inertia - grad_x * (1.0 - cfg.inertia);
            drop.dir_z = drop.dir_z * cfg.inertia - grad_z * (1.0 - cfg.inertia);
            let len = (drop.dir_x * drop.dir_x + drop.dir_z * drop.dir_z).sqrt();
            if len < f32::EPSILON {
                alive = false;
                break;
            }
            drop.dir_x /= len;
            drop.dir_z /= len;

            let new_px = drop.px + drop.dir_x;
            let new_pz = drop.pz + drop.dir_z;
            if !new_px.is_finite()
                || !new_pz.is_finite()
                || new_px < 0.0
                || new_px >= (w - 1) as f32
                || new_pz < 0.0
                || new_pz >= (h - 1) as f32
            {
                alive = false;
                break;
            }

            let new_ix = new_px.floor() as usize;
            let new_iz = new_pz.floor() as usize;
            let nfx = new_px - new_ix as f32;
            let nfz = new_pz - new_iz as f32;
            let nh00 = hm.get(new_ix, new_iz);
            let nh10 = hm.get(new_ix + 1, new_iz);
            let nh01 = hm.get(new_ix, new_iz + 1);
            let nh11 = hm.get(new_ix + 1, new_iz + 1);
            let height_new = nh00 * (1.0 - nfx) * (1.0 - nfz)
                + nh10 * nfx * (1.0 - nfz)
                + nh01 * (1.0 - nfx) * nfz
                + nh11 * nfx * nfz;

            let delta_h = height_new - height_here;
            // Use a fixed min_slope (matches HydraulicErosion default) instead of
            // capacity_factor.recip(), which would produce +Inf when capacity_factor==0
            // and cascade into NaN throughout the droplet simulation.
            let slope = (-delta_h).max(0.01_f32);
            let capacity = (slope * drop.vel * drop.water * cfg.capacity_factor).max(0.0);

            if drop.sediment > capacity || delta_h > 0.0 {
                let deposit = if delta_h > 0.0 {
                    delta_h.min(drop.sediment)
                } else {
                    (drop.sediment - capacity) * cfg.deposition_rate
                };
                drop.sediment -= deposit;
                let (w00, w10, w01, w11) = bilinear_weights(fx, fz);
                let v = hm.get_mut(ix, iz);
                *v += deposit * w00;

                let v = hm.get_mut(ix + 1, iz);
                *v += deposit * w10;

                let v = hm.get_mut(ix, iz + 1);
                *v += deposit * w01;

                let v = hm.get_mut(ix + 1, iz + 1);
                *v += deposit * w11;
            } else {
                let erode = ((capacity - drop.sediment) * cfg.erosion_rate)
                    .min(-delta_h)
                    .max(0.0);
                drop.sediment += erode;
                let (w00, w10, w01, w11) = bilinear_weights(fx, fz);
                let v = hm.get_mut(ix, iz);
                *v = (*v - erode * w00).max(0.0);

                let v = hm.get_mut(ix + 1, iz);
                *v = (*v - erode * w10).max(0.0);

                let v = hm.get_mut(ix, iz + 1);
                *v = (*v - erode * w01).max(0.0);

                let v = hm.get_mut(ix + 1, iz + 1);
                *v = (*v - erode * w11).max(0.0);
            }

            drop.vel = (drop.vel * drop.vel + delta_h * (-9.8)).max(0.0).sqrt();
            drop.water *= 1.0 - cfg.evaporation_rate;
            drop.px = new_px;
            drop.pz = new_pz;

            drop.trail.push_back(Vec2::new(new_px, new_pz));
            if drop.trail.len() > 16 {
                drop.trail.pop_front();
            }
        }

        if alive && drop.steps_left > 0 && drop.water >= 0.01 {
            still_alive.push(drop);
        } else {
            newly_completed += 1;
        }
    }

    viz.active = still_alive;
    viz.completed += newly_completed;

    viz.heightmap = Some(hm);

    // Publish snapshot for terrain mesh rebuild every Nth frame to avoid thrashing.
    viz.frame_counter += 1;
    let should_publish = viz.frame_counter >= viz.publish_every_n_frames
        || (viz.completed >= viz.total && viz.active.is_empty());
    if should_publish {
        viz.frame_counter = 0;
        let snapshot = viz.heightmap.as_ref().unwrap().clone();
        current_hm.0 = Some(snapshot);
        dirty_mesh.0 = true;
    }

    if viz.completed >= viz.total && viz.active.is_empty() {
        // Apply thermal relaxation pass to match the background generator.
        // This runs synchronously on the main thread but is fast (~10 ms for
        // typical grid sizes) and only happens once at the end of the viz.
        if let Some(ref mut hm) = viz.heightmap.as_mut().filter(|_| cfg.thermal_enabled) {
            let absolute_water = cfg.water_level * cfg.height_scale;
            ThermalErosion::new()
                .with_iterations(cfg.thermal_iterations)
                .with_talus_angle(cfg.thermal_talus_angle)
                .with_water_level(absolute_water)
                .with_underwater_talus_angle(0.01)
                .erode(hm);
            // Publish the thermally-smoothed result.
            current_hm.0 = Some(hm.clone());
            dirty_mesh.0 = true;
        }

        viz.enabled = false;
        dirty.terrain = false;
    }
}

/// Compute the four bilinear interpolation weights for fractional offsets
/// `(fx, fz)` within a grid cell.
///
/// Returns `(w00, w10, w01, w11)` corresponding to the corners
/// `(x, z)`, `(x+1, z)`, `(x, z+1)`, `(x+1, z+1)`. The weights sum to 1.
#[inline]
fn bilinear_weights(fx: f32, fz: f32) -> (f32, f32, f32, f32) {
    (
        (1.0 - fx) * (1.0 - fz),
        fx * (1.0 - fz),
        (1.0 - fx) * fz,
        fx * fz,
    )
}
