//! Debug gizmo overlays for urban generation output.
//!
//! Renders road graph edges (major = yellow, minor = cyan), city block
//! perimeters (green outlines) with optional centroids (magenta spheres),
//! and building lot footprints (orange oriented rectangles). All gizmos are
//! drawn slightly above the terrain surface to avoid z-fighting and can be
//! individually toggled from the Urban Planner UI.

use bevy::prelude::*;

use crate::core::config::CurrentHeightMap;
use crate::core::urban_config::{CurrentBuildingLots, CurrentRoadGraph, UrbanConfig};
use symbios_tensor::{RoadType, block_centroid};

/// Draw road graph edges as colored gizmo lines above the terrain surface.
pub fn draw_road_gizmos(
    config: Res<UrbanConfig>,
    current_hm: Res<CurrentHeightMap>,
    current_rg: Res<CurrentRoadGraph>,
    mut gizmos: Gizmos,
) {
    if !config.show_gizmos || !config.enabled {
        return;
    }
    let Some(hm) = &current_hm.0 else { return };
    let Some(graph) = &current_rg.0 else { return };

    let half_w = hm.world_width() * 0.5;
    let half_d = hm.world_depth() * 0.5;

    for edge in &graph.edges {
        if !edge.active {
            continue;
        }

        let p1 = graph.nodes[edge.start as usize].position;
        let p2 = graph.nodes[edge.end as usize].position;

        // Sample height and offset slightly above terrain to avoid z-fighting
        let y1 = hm.get_height_at(p1.x, p1.y) + 0.5;
        let y2 = hm.get_height_at(p2.x, p2.y) + 0.5;

        // Convert to Bevy world space (centered at origin)
        let v1 = Vec3::new(p1.x - half_w, y1, p1.y - half_d);
        let v2 = Vec3::new(p2.x - half_w, y2, p2.y - half_d);

        let color = match edge.road_type {
            RoadType::Major => Color::srgb(1.0, 0.8, 0.1),
            RoadType::Minor => Color::srgb(0.1, 0.8, 1.0),
        };

        gizmos.line(v1, v2, color);
    }
}

/// Draw city block perimeters and centroids as colored gizmo overlays.
pub fn draw_block_gizmos(
    config: Res<UrbanConfig>,
    current_hm: Res<CurrentHeightMap>,
    current_rg: Res<CurrentRoadGraph>,
    mut gizmos: Gizmos,
) {
    if !config.enabled {
        return;
    }
    let show_outlines = config.show_block_gizmos;
    let show_centroids = config.show_block_centroids;
    if !show_outlines && !show_centroids {
        return;
    }
    let Some(hm) = &current_hm.0 else { return };
    let Some(graph) = &current_rg.0 else { return };

    let half_w = hm.world_width() * 0.5;
    let half_d = hm.world_depth() * 0.5;
    let outline_color = Color::srgb(0.2, 1.0, 0.4);
    let centroid_color = Color::srgb(1.0, 0.3, 0.8);

    for block in &graph.blocks {
        let perimeter = &block.perimeter;
        if perimeter.len() < 3 {
            continue;
        }

        if show_outlines {
            for i in 0..perimeter.len() {
                let p1 = graph.nodes[perimeter[i] as usize].position;
                let p2 = graph.nodes[perimeter[(i + 1) % perimeter.len()] as usize].position;

                let y1 = hm.get_height_at(p1.x, p1.y) + 0.6;
                let y2 = hm.get_height_at(p2.x, p2.y) + 0.6;

                let v1 = Vec3::new(p1.x - half_w, y1, p1.y - half_d);
                let v2 = Vec3::new(p2.x - half_w, y2, p2.y - half_d);

                gizmos.line(v1, v2, outline_color);
            }
        }

        if show_centroids {
            let c = block_centroid(block, graph);
            let cy = hm.get_height_at(c.x, c.y) + 0.8;
            let center = Vec3::new(c.x - half_w, cy, c.y - half_d);
            gizmos.sphere(Isometry3d::from_translation(center), 0.5, centroid_color);
        }
    }
}

/// Draw building lot footprints as oriented rectangles above the terrain.
pub fn draw_lot_gizmos(
    config: Res<UrbanConfig>,
    current_hm: Res<CurrentHeightMap>,
    current_lots: Res<CurrentBuildingLots>,
    mut gizmos: Gizmos,
) {
    if !config.enabled || !config.show_lot_gizmos {
        return;
    }
    let Some(hm) = &current_hm.0 else { return };
    if current_lots.0.is_empty() {
        return;
    }

    let half_w = hm.world_width() * 0.5;
    let half_d = hm.world_depth() * 0.5;
    let color = Color::srgb(1.0, 0.6, 0.2);

    for lot in &current_lots.0 {
        let y = hm.get_height_at(lot.position.x, lot.position.y) + 0.7;
        let center = Vec3::new(lot.position.x - half_w, y, lot.position.y - half_d);

        // Build the four corners of the oriented rectangle
        let cos = lot.rotation.cos();
        let sin = lot.rotation.sin();
        let hw = lot.width * 0.5;
        let hd = lot.depth * 0.5;

        // Local offsets (street_dir = cos/sin, inward_dir = -sin/cos)
        let corners = [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)];

        let world_corners: Vec<Vec3> = corners
            .iter()
            .map(|&(lx, ly)| {
                let wx = lx * cos - ly * sin;
                let wz = lx * sin + ly * cos;
                Vec3::new(center.x + wx, center.y, center.z + wz)
            })
            .collect();

        for i in 0..4 {
            gizmos.line(world_corners[i], world_corners[(i + 1) % 4], color);
        }
    }
}
