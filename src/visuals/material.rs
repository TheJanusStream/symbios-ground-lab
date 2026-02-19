//! Splat-based terrain material pipeline.
//!
//! # Pipeline overview
//!
//! 1. [`detect_material_dirty`] watches for config/heightmap changes and sets
//!    `textures_dirty` / `splat_dirty` flags on [`MaterialState`].
//! 2. [`start_texture_tasks`] spawns four [`PendingTexture`] entities (one per
//!    splat channel) when `textures_dirty` is set.
//! 3. [`bevy_symbios_texture::SymbiosTexturePlugin`] polls those tasks and
//!    attaches [`TextureReady`] when generation completes.
//! 4. [`collect_texture_results`] harvests the raw pixel bytes, stores them
//!    in [`MaterialState`], then removes the temporary GPU images.
//! 5. [`bake_and_apply_material`] runs once `splat_dirty` is set and all
//!    layer bytes are available: it generates a [`SplatMapper`] weight map,
//!    blends the four textures on the CPU, uploads two baked images (albedo +
//!    normal), and applies them to the terrain's [`StandardMaterial`].

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_symbios_texture::async_gen::{PendingTexture, TextureReady};
use symbios_ground::splat::SplatMapper;

use crate::core::{
    config::CurrentHeightMap,
    material_config::{MaterialConfig, MaterialState, MaterialStatus, TerrainMaterialHandle},
};

// ---------------------------------------------------------------------------
// Marker component
// ---------------------------------------------------------------------------

/// Tags a [`PendingTexture`] entity with its splat-channel index (0–3).
#[derive(Component)]
pub struct TextureLayerIndex(pub usize);

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Watches [`CurrentHeightMap`] for changes; sets `splat_dirty` so the bake
/// step runs with fresh weights.
///
/// Texture-dirty is set explicitly by the UI (via [`MaterialState`]) when the
/// user actually modifies a texture parameter, avoiding the Bevy
/// `ResMut`-change-detection pitfall that would fire every frame.
pub fn detect_material_dirty(
    current_hm: Res<CurrentHeightMap>,
    mut mat_state: ResMut<MaterialState>,
) {
    // A new heightmap means splat weights must be recomputed, but we can
    // reuse the already-generated procedural textures.
    if current_hm.is_changed() && current_hm.0.is_some() && !mat_state.textures_dirty {
        mat_state.splat_dirty = true;
    }
}

/// Spawns four async [`PendingTexture`] tasks (grass, dirt, rock, snow) when
/// `mat_state.textures_dirty` is set.
pub fn start_texture_tasks(
    mut commands: Commands,
    mat_config: Res<MaterialConfig>,
    mut mat_state: ResMut<MaterialState>,
    pending: Query<Entity, With<TextureLayerIndex>>,
) {
    if !mat_state.textures_dirty {
        return;
    }
    if !mat_config.enabled {
        mat_state.textures_dirty = false;
        return;
    }

    // Cancel any lingering layer entities from a previous run.
    for entity in &pending {
        commands.entity(entity).despawn();
    }

    // Clear accumulated layer data.
    mat_state.layer_albedo = [None, None, None, None];
    mat_state.layer_normal = [None, None, None, None];
    mat_state.layer_tex_size = mat_config.texture_size;
    mat_state.status = MaterialStatus::GeneratingTextures;

    let sz = mat_config.texture_size;

    commands.spawn((
        PendingTexture::ground(mat_config.grass.clone(), sz, sz),
        TextureLayerIndex(0),
    ));
    commands.spawn((
        PendingTexture::ground(mat_config.dirt.clone(), sz, sz),
        TextureLayerIndex(1),
    ));
    commands.spawn((
        PendingTexture::rock(mat_config.rock.clone(), sz, sz),
        TextureLayerIndex(2),
    ));
    commands.spawn((
        PendingTexture::ground(mat_config.snow.clone(), sz, sz),
        TextureLayerIndex(3),
    ));

    mat_state.textures_dirty = false;
}

/// Harvests completed [`TextureReady`] results: clones raw pixel bytes into
/// [`MaterialState`], removes the temporary GPU image assets, and despawns
/// the layer entity. When all four layers are ready, `splat_dirty` is set.
pub fn collect_texture_results(
    mut commands: Commands,
    ready_textures: Query<(Entity, &TextureLayerIndex, &TextureReady)>,
    images: Res<Assets<Image>>,
    mut mat_state: ResMut<MaterialState>,
) {
    for (entity, layer_idx, ready) in &ready_textures {
        let idx = layer_idx.0;

        // Clone the raw bytes before we do anything else.
        let albedo_bytes = images
            .get(&ready.0.albedo)
            .and_then(|img| img.data.as_deref())
            .map(|b| b.to_vec());

        let normal_bytes = images
            .get(&ready.0.normal)
            .and_then(|img| img.data.as_deref())
            .map(|b| b.to_vec());

        if let (Some(a), Some(n)) = (albedo_bytes, normal_bytes) {
            mat_state.layer_albedo[idx] = Some(a);
            mat_state.layer_normal[idx] = Some(n);
        } else {
            bevy::log::error!("Texture data unavailable for layer {idx} after generation");
        }

        commands.entity(entity).despawn();
    }

    // Once all layers are populated, trigger baking.
    if mat_state.all_layers_ready() && mat_state.status == MaterialStatus::GeneratingTextures {
        mat_state.splat_dirty = true;
    }
}

/// Generates the [`SplatMapper`] weight map from the current heightmap, blends
/// the four procedural textures on the CPU, and uploads the result as a pair
/// of baked images (albedo + normal map) applied to the terrain material.
pub fn bake_and_apply_material(
    mut mat_state: ResMut<MaterialState>,
    mat_config: Res<MaterialConfig>,
    current_hm: Res<CurrentHeightMap>,
    terrain_mat_handle: Option<Res<TerrainMaterialHandle>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !mat_state.splat_dirty {
        return;
    }

    // Disabled: clear any previously baked textures and reset the material.
    if !mat_config.enabled {
        mat_state.splat_dirty = false;
        if let (Some(handle), Some(mat)) = (
            terrain_mat_handle.as_ref(),
            terrain_mat_handle
                .as_ref()
                .and_then(|h| materials.get_mut(&h.0)),
        ) {
            let _ = handle; // keep the borrow alive
            mat.base_color = Color::srgb(0.35, 0.55, 0.25);
            mat.base_color_texture = None;
            mat.normal_map_texture = None;
        }
        mat_state.status = MaterialStatus::Idle;
        return;
    }

    if !mat_state.all_layers_ready() {
        return;
    }
    let Some(hm) = &current_hm.0 else {
        return;
    };
    let Some(terrain_handle) = &terrain_mat_handle else {
        return;
    };

    // --- Splat weight map ---------------------------------------------------
    let mapper = SplatMapper::new([
        mat_config.rules[0].to_splat_rule(),
        mat_config.rules[1].to_splat_rule(),
        mat_config.rules[2].to_splat_rule(),
        mat_config.rules[3].to_splat_rule(),
    ]);
    let weight_map = mapper.generate(hm);

    let bake_w = weight_map.width as u32;
    let bake_h = weight_map.height as u32;
    let tex_size = mat_state.layer_tex_size as usize;
    let tile_scale = mat_config.tile_scale;

    // --- CPU texture bake ---------------------------------------------------
    let n = (bake_w * bake_h) as usize;
    let mut out_albedo = vec![0u8; n * 4];
    let mut out_normal = vec![0u8; n * 4];

    // Extract raw slices (all layers are present; checked by all_layers_ready).
    let al: [&[u8]; 4] = [
        mat_state.layer_albedo[0].as_deref().unwrap(),
        mat_state.layer_albedo[1].as_deref().unwrap(),
        mat_state.layer_albedo[2].as_deref().unwrap(),
        mat_state.layer_albedo[3].as_deref().unwrap(),
    ];
    let nl: [&[u8]; 4] = [
        mat_state.layer_normal[0].as_deref().unwrap(),
        mat_state.layer_normal[1].as_deref().unwrap(),
        mat_state.layer_normal[2].as_deref().unwrap(),
        mat_state.layer_normal[3].as_deref().unwrap(),
    ];

    for z in 0..bake_h as usize {
        for x in 0..bake_w as usize {
            let pixel_idx = z * bake_w as usize + x;
            let weights = weight_map.data[pixel_idx];
            let wf = [
                weights[0] as f32 / 255.0,
                weights[1] as f32 / 255.0,
                weights[2] as f32 / 255.0,
                weights[3] as f32 / 255.0,
            ];

            // Tiled UV — wraps at texture boundaries.
            let u = (x as f32 / bake_w as f32 * tile_scale).fract();
            let v = (z as f32 / bake_h as f32 * tile_scale).fract();
            let tx = (u * tex_size as f32) as usize % tex_size;
            let ty = (v * tex_size as f32) as usize % tex_size;
            let tex_pixel = (ty * tex_size + tx) * 4;

            let out_pixel = pixel_idx * 4;

            let (mut ra, mut ga, mut ba) = (0.0f32, 0.0f32, 0.0f32);
            let (mut rn, mut gn, mut bn) = (0.0f32, 0.0f32, 0.0f32);

            for layer in 0..4usize {
                let w = wf[layer];
                if w < 1e-5 {
                    continue;
                }
                ra += al[layer][tex_pixel] as f32 * w;
                ga += al[layer][tex_pixel + 1] as f32 * w;
                ba += al[layer][tex_pixel + 2] as f32 * w;

                rn += nl[layer][tex_pixel] as f32 * w;
                gn += nl[layer][tex_pixel + 1] as f32 * w;
                bn += nl[layer][tex_pixel + 2] as f32 * w;
            }

            out_albedo[out_pixel] = ra.round() as u8;
            out_albedo[out_pixel + 1] = ga.round() as u8;
            out_albedo[out_pixel + 2] = ba.round() as u8;
            out_albedo[out_pixel + 3] = 255;

            out_normal[out_pixel] = rn.round() as u8;
            out_normal[out_pixel + 1] = gn.round() as u8;
            out_normal[out_pixel + 2] = bn.round() as u8;
            out_normal[out_pixel + 3] = 255;
        }
    }

    // --- Upload to GPU ------------------------------------------------------
    // Remove old baked images before adding new ones.
    if let Some(old) = mat_state.baked_albedo.take() {
        images.remove(old.id());
    }
    if let Some(old) = mat_state.baked_normal.take() {
        images.remove(old.id());
    }

    let albedo_handle = images.add(make_baked_image(
        out_albedo,
        bake_w,
        bake_h,
        TextureFormat::Rgba8UnormSrgb,
    ));
    let normal_handle = images.add(make_baked_image(
        out_normal,
        bake_w,
        bake_h,
        TextureFormat::Rgba8Unorm,
    ));

    mat_state.baked_albedo = Some(albedo_handle.clone());
    mat_state.baked_normal = Some(normal_handle.clone());

    // Apply to terrain StandardMaterial.
    if let Some(mat) = materials.get_mut(&terrain_handle.0) {
        mat.base_color = Color::WHITE;
        mat.base_color_texture = Some(albedo_handle);
        mat.normal_map_texture = Some(normal_handle);
        mat.perceptual_roughness = 0.85;
        mat.metallic = 0.0;
    }

    mat_state.splat_dirty = false;
    mat_state.status = MaterialStatus::Ready;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_baked_image(data: Vec<u8>, width: u32, height: u32, format: TextureFormat) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    )
}
