//! Gizmo rendering for active erosion-visualisation droplets.
//!
//! Each droplet is drawn as a bright cyan sphere with a fading trail of smaller
//! spheres tracing its recent path across the terrain.

use bevy::prelude::*;

use crate::core::config::ErosionVizState;

/// Draw gizmos for all active droplets during erosion visualisation.
pub fn draw_droplet_gizmos(viz: Res<ErosionVizState>, mut gizmos: Gizmos) {
    if !viz.enabled {
        return;
    }
    let Some(ref hm) = viz.heightmap else { return };

    let half = (hm.width() - 1) as f32 * hm.scale() * 0.5;

    for drop in &viz.active {
        let raw_x = drop.px * hm.scale();
        let raw_z = drop.pz * hm.scale();
        let world_y = hm.get_height_at(raw_x, raw_z) + 0.5;

        // Droplet head: bright cyan sphere
        gizmos.sphere(
            Vec3::new(raw_x - half, world_y, raw_z - half),
            0.6,
            Color::srgba(0.1, 0.8, 1.0, 0.9),
        );

        // Trail: fade from cyan to transparent
        let trail_len = drop.trail.len();
        for (i, pos) in drop.trail.iter().enumerate() {
            let alpha = i as f32 / trail_len as f32;
            let tx = pos.x * hm.scale();
            let tz = pos.y * hm.scale();
            let ty = hm.get_height_at(tx, tz) + 0.2;
            gizmos.sphere(
                Vec3::new(tx - half, ty, tz - half),
                0.2,
                Color::srgba(0.3, 0.7, 1.0, alpha * 0.6),
            );
        }
    }
}
