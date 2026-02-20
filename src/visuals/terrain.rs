use bevy::prelude::*;
use bevy_symbios_ground::{HeightMapMeshBuilder, NormalMethod};

use crate::core::config::{CurrentHeightMap, DirtyFlags, DirtyMesh};
use crate::visuals::splat_material::{SplatExtension, SplatMaterialHandle, SplatTerrainMaterial};

/// Marker component for the primary terrain mesh entity.
#[derive(Component)]
pub struct TerrainMesh;

/// Spawn a placeholder flat plane until the first generation completes.
///
/// The terrain uses [`SplatTerrainMaterial`] from the start with
/// `extension.uniforms.enabled = 0` so the base StandardMaterial green colour
/// shows through while procedural textures are still generating.  The
/// [`SplatMaterialHandle`] resource is inserted so the material pipeline can
/// update the extension later.
pub fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SplatTerrainMaterial>>,
    mut dirty_flags: ResMut<DirtyFlags>,
) {
    let placeholder = meshes.add(
        Plane3d::default()
            .mesh()
            .size(256.0, 256.0)
            .subdivisions(1)
            .build(),
    );
    let mat_handle = materials.add(SplatTerrainMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.35, 0.55, 0.25),
            perceptual_roughness: 0.9,
            ..default()
        },
        extension: SplatExtension::default(), // enabled = 0, all handles invalid
    });
    commands.spawn((
        Mesh3d(placeholder),
        MeshMaterial3d(mat_handle.clone()),
        Transform::default(),
        TerrainMesh,
    ));
    commands.insert_resource(SplatMaterialHandle(mat_handle));

    // Kick off the first generation immediately.
    dirty_flags.terrain = true;
}

/// Rebuild the terrain mesh whenever a new heightmap has been generated.
///
/// Tangents are generated after each rebuild so the splat fragment shader can
/// use the Mikktspace TBN frame for normal-map blending.
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

    // UVs must span [0, 1] across the whole terrain so the splat weight map
    // (one pixel per heightmap cell, full-terrain coverage) maps exactly once
    // over the mesh.
    let world_extent = (hm.width() - 1) as f32 * hm.scale();
    let mut mesh = HeightMapMeshBuilder::new()
        .with_normal_method(NormalMethod::AreaWeighted)
        .with_uv_tile_size(world_extent)
        .build(hm);

    // Generate per-vertex tangents so the fragment shader can build the TBN
    // frame for tangent-space normal-map blending.
    mesh.generate_tangents()
        .expect("terrain mesh tangent generation failed");

    for (mut mesh3d, mut transform) in &mut query {
        // Update the existing asset buffer in-place so the old allocation is
        // reused and never orphaned.
        if let Some(existing) = meshes.get_mut(&mesh3d.0) {
            *existing = mesh.clone();
        } else {
            mesh3d.0 = meshes.add(mesh.clone());
        }
        transform.translation = Vec3::ZERO;
    }

    dirty_mesh.0 = false;
}
