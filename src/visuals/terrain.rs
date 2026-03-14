//! Terrain mesh and water volume management.
//!
//! [`spawn_terrain`] creates the initial placeholder plane with a
//! [`SplatTerrainMaterial`] and a translucent [`WaterVolume`] cuboid at
//! startup.  [`rebuild_terrain`] replaces the mesh whenever a new heightmap
//! is published to [`CurrentHeightMap`], generating Mikktspace tangents for
//! normal-map blending (skipped during active erosion visualisation).

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_symbios_ground::{HeightMapMeshBuilder, NormalMethod};

use crate::visuals::splat_material::{SplatExtension, SplatMaterialHandle, SplatTerrainMaterial};
use crate::{
    core::config::{CurrentHeightMap, DirtyFlags, DirtyMesh, TerrainConfig},
    visuals::water_material::{WaterExtension, WaterMaterial},
};

/// Marker component for the primary terrain mesh entity.
#[derive(Component)]
pub struct TerrainMesh;

/// Marker component for the water-level volume entity (translucent cuboid).
#[derive(Component)]
pub struct WaterVolume;

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
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut dirty_flags: ResMut<DirtyFlags>,
    config: Res<TerrainConfig>,
) {
    let placeholder = meshes.add(
        Plane3d::default()
            .mesh()
            .size(256.0, 256.0)
            .subdivisions(1)
            .build(),
    );

    // `AsBindGroup` for `dimension = "2d_array"` creates a `TextureViewDimension::D2Array`
    // view.  Bevy's default fallback (a 1×1 2D texture) produces a `D2` view, causing
    // a wgpu validation error before the real arrays are uploaded.  Seed the extension
    // with 1×1×4 array textures so the bind group is always valid from frame 0.
    // Four identical white/flat-normal pixels — one per splat layer — satisfy the
    // layer-count requirement without wasting memory.
    let albedo_placeholder = images.add(Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 4,
        },
        TextureDimension::D2,
        vec![255u8; 4 * 4], // 4 layers × (R G B A)
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));
    // Flat normal: (128, 128, 255, 255) = (0, 0, 1) in tangent space.
    let flat_normal_pixel: Vec<u8> = vec![128, 128, 255, 255];
    let normal_placeholder = images.add(Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 4,
        },
        TextureDimension::D2,
        flat_normal_pixel.repeat(4), // 4 layers
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    ));

    let mat_handle = materials.add(SplatTerrainMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.35, 0.55, 0.25),
            perceptual_roughness: 0.9,
            ..default()
        },
        extension: SplatExtension {
            albedo_array: albedo_placeholder,
            normal_array: normal_placeholder,
            ..default() // enabled = 0, weight_map = default handle
        },
    });
    commands.spawn((
        Mesh3d(placeholder),
        MeshMaterial3d(mat_handle.clone()),
        Transform::default(),
        TerrainMesh,
    ));
    commands.insert_resource(SplatMaterialHandle(mat_handle));

    let water_mat = water_materials.add(WaterMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(0.0, 0.4, 0.6, 0.5),
            perceptual_roughness: 0.05,
            metallic: 0.1,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        },
        extension: WaterExtension::default(),
    });

    let world_extent = (config.grid_size - 1) as f32 * config.cell_scale;
    let wl = (config.water_level * config.height_scale).max(0.001);

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(water_mat),
        Transform::from_xyz(0.0, wl / 2.0, 0.0).with_scale(Vec3::new(
            world_extent,
            wl,
            world_extent,
        )),
        WaterVolume,
    ));

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
    viz: Res<crate::core::config::ErosionVizState>,
    mut water_q: Query<&mut Transform, (With<WaterVolume>, Without<TerrainMesh>)>,
    config: Res<TerrainConfig>,
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

    // Water volume
    if let Ok(mut transform) = water_q.single_mut() {
        let wl = (config.water_level * config.height_scale).max(0.001);
        transform.translation.y = wl / 2.0;
        transform.scale.y = wl;
        transform.scale.x = (config.grid_size - 1) as f32 * config.cell_scale;
        transform.scale.z = (config.grid_size - 1) as f32 * config.cell_scale;
    }

    // Generate per-vertex tangents so the fragment shader can build the TBN
    // frame for tangent-space normal-map blending. Skip during active erosion
    // viz: the splat material isn't updated mid-viz so correct tangents aren't
    // needed until the final rebuild when viz.enabled is already false.
    if !viz.enabled {
        mesh.generate_tangents()
            .expect("terrain mesh tangent generation failed");
    }

    // There is always exactly one TerrainMesh. iter_mut().next() lets us move
    // `mesh` by value without cloning (Mesh doesn't implement Default, so
    // mem::take is not an option).
    if let Some((mut mesh3d, mut transform)) = query.iter_mut().next() {
        // Update the existing asset buffer in-place so the old allocation is
        // reused and never orphaned.
        if let Some(existing) = meshes.get_mut(&mesh3d.0) {
            *existing = mesh;
        } else {
            mesh3d.0 = meshes.add(mesh);
        }
        let half = world_extent * 0.5;
        transform.translation = Vec3::new(-half, 0.0, -half);
    }

    dirty_mesh.0 = false;
}
