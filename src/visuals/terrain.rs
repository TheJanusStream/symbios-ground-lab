use bevy::prelude::*;
use bevy_symbios_ground::{HeightMapMeshBuilder, NormalMethod};

use crate::core::config::{CurrentHeightMap, DirtyMesh};
use crate::core::material_config::TerrainMaterialHandle;

/// Marker component for the primary terrain mesh entity.
#[derive(Component)]
pub struct TerrainMesh;

/// Spawn a placeholder flat plane until the first generation completes.
///
/// Also inserts a [`TerrainMaterialHandle`] resource so the material systems
/// can update the terrain's [`StandardMaterial`] at runtime.
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
    let mat_handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.55, 0.25),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Mesh3d(placeholder),
        MeshMaterial3d(mat_handle.clone()),
        Transform::default(),
        TerrainMesh,
    ));
    commands.insert_resource(TerrainMaterialHandle(mat_handle));

    // Kick off the first generation immediately.
    dirty.0 = true;
}

/// Rebuild the terrain mesh whenever a new heightmap has been generated.
pub fn rebuild_terrain(
    mut query: Query<(&mut Mesh3d, &mut Transform), With<TerrainMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    current_hm: Res<CurrentHeightMap>,
    mut dirty_mesh: ResMut<DirtyMesh>,
) {
    if !dirty_mesh.0 {
        return;
    }
    let Some(hm) = &current_hm.0 else { return };

    // UVs must span [0, 1] across the whole terrain so the baked splat
    // texture (one pixel per heightmap cell, full-terrain coverage) maps
    // exactly once over the mesh.  Using the world extent as the tile size
    // achieves this: u = world_x / world_extent ∈ [0, 1].
    let world_extent = (hm.width() - 1) as f32 * hm.scale();
    let mesh = HeightMapMeshBuilder::new()
        .with_normal_method(NormalMethod::AreaWeighted)
        .with_uv_tile_size(world_extent)
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
