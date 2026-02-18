use bevy::prelude::*;
use bevy_symbios_ground::HeightMapMeshBuilder;

use crate::core::config::{CurrentHeightMap, DirtyMesh, TerrainConfig};

/// Marker component for the primary terrain mesh entity.
#[derive(Component)]
pub struct TerrainMesh;

/// Spawn a placeholder flat plane until the first generation completes.
pub fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dirty: ResMut<DirtyMesh>,
) {
    let placeholder = meshes.add(
        Plane3d::default()
            .mesh()
            .size(256.0, 256.0)
            .subdivisions(1)
            .build(),
    );
    commands.spawn((
        Mesh3d(placeholder),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.55, 0.25),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::default(),
        TerrainMesh,
    ));

    // Kick off the first generation immediately
    dirty.0 = true;
}

/// Rebuild the terrain mesh whenever a new heightmap has been generated.
pub fn rebuild_terrain(
    mut query: Query<(&mut Mesh3d, &mut Transform), With<TerrainMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    current_hm: Res<CurrentHeightMap>,
    config: Res<TerrainConfig>,
    mut dirty_mesh: ResMut<DirtyMesh>,
) {
    if !dirty_mesh.0 {
        return;
    }
    let Some(hm) = &current_hm.0 else { return };

    let mesh = HeightMapMeshBuilder::new()
        .with_uv_tile_size(config.cell_scale * 4.0)
        .build(hm);

    for (mut mesh3d, mut transform) in &mut query {
        // Update the existing asset buffer in-place so the old allocation is
        // reused and never orphaned.  Only fall back to a new handle if the
        // asset has somehow been removed from the store already.
        if let Some(existing) = meshes.get_mut(&mesh3d.0) {
            *existing = mesh.clone();
        } else {
            mesh3d.0 = meshes.add(mesh.clone());
        }
        // Mesh vertices are already in world space [0, world_w] × [0, world_d];
        // no translation needed.
        transform.translation = Vec3::ZERO;
    }

    dirty_mesh.0 = false;
}
