//! 3D road mesh generation from the tensor-field road graph.
//!
//! Converts [`CurrentRoadGraph`](crate::core::urban_config::CurrentRoadGraph)
//! into renderable Bevy meshes: intersection hubs (regular polygons) and
//! spline-sampled ribbons connecting them. Meshes are despawned and rebuilt
//! whenever the road graph or [`UrbanConfig`](crate::core::urban_config::UrbanConfig) changes.

use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::core::config::CurrentHeightMap;
use crate::core::urban_config::{CurrentRoadGraph, UrbanConfig};
use crate::visuals::road_materials::RoadMaterialHandle;

/// Marker component for despawning road mesh entities.
#[derive(Component)]
pub struct RoadMesh;

/// Rebuilds 3D road meshes when the road graph or urban config changes.
pub fn rebuild_roads(
    mut commands: Commands,
    current_rg: Res<CurrentRoadGraph>,
    config: Res<UrbanConfig>,
    hm: Res<CurrentHeightMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    existing_q: Query<Entity, With<RoadMesh>>,
    handle_q: Query<&RoadMaterialHandle>,
) {
    if !current_rg.is_changed() && !config.is_changed() {
        return;
    }

    // Despawn existing road meshes.
    for e in &existing_q {
        commands.entity(e).despawn();
    }

    if !config.enabled || !config.render_roads {
        return;
    }

    let Some(ref graph) = current_rg.0 else { return };
    let Some(ref heightmap) = hm.0 else { return };
    let Ok(road_handle) = handle_q.single() else {
        return;
    };

    let mesh_config = symbios_tensor::RoadMeshConfig {
        major_half_width: config.road_width * 0.5 * 1.5, // Major roads are 1.5x wider
        minor_half_width: config.road_width * 0.5,
        hub_sides: config.hub_segments,
        depth_bias: 0.05,
        texture_scale: 0.1,
        spline_subdivisions: config.road_resolution as u32,
    };

    let road_meshes = symbios_tensor::generate_road_meshes(graph, heightmap, &mesh_config);

    // World offset: terrain is centered at origin.
    let half_w = heightmap.world_width() * 0.5;
    let half_d = heightmap.world_depth() * 0.5;
    let offset = Vec3::new(-half_w, 0.0, -half_d);

    // Spawn hubs mesh.
    if let Some(mesh) = procedural_to_bevy(&road_meshes.hubs) {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(road_handle.0.clone()),
            Transform::from_translation(offset),
            RoadMesh,
        ));
    }

    // Spawn ribbons mesh.
    if let Some(mesh) = procedural_to_bevy(&road_meshes.ribbons) {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(road_handle.0.clone()),
            Transform::from_translation(offset),
            RoadMesh,
        ));
    }
}

/// Converts a `ProceduralMesh` into a Bevy `Mesh`.
fn procedural_to_bevy(raw: &symbios_tensor::ProceduralMesh) -> Option<Mesh> {
    if raw.vertices.is_empty() || raw.indices.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, raw.vertices.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, raw.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, raw.uvs.clone());
    mesh.insert_indices(Indices::U32(raw.indices.clone()));

    if let Err(e) = mesh.generate_tangents() {
        warn!("Road mesh tangent generation failed: {e}");
    }

    Some(mesh)
}
