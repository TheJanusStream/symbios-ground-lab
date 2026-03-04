use bevy::prelude::*;

use crate::core::config::CurrentHeightMap;
use crate::core::urban_config::{CurrentRoadGraph, UrbanConfig};
use symbios_tensor::RoadType;

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
