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
//! 4. [`collect_texture_results`] stores the GPU image handles from each
//!    completed layer into [`MaterialState`], then despawns the temporary
//!    entity.  No bytes are copied to the CPU — the images stay on the GPU.
//! 5. [`apply_splat_material`] runs once `splat_dirty` is set and all layer
//!    handles are available: it generates a [`SplatMapper`] weight map,
//!    uploads it as a GPU texture, merges the four per-layer images into two
//!    [`texture_2d_array`] assets (albedo + normal), and updates the terrain's
//!    [`SplatTerrainMaterial`] extension with the three texture handles.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_symbios_texture::async_gen::{PendingTexture, TextureReady};
use symbios_ground::splat::SplatMapper;

use crate::core::{
    config::{CurrentHeightMap, TerrainConfig},
    material_config::{MaterialConfig, MaterialState, MaterialStatus},
};
use crate::visuals::splat_material::{SplatMaterialHandle, SplatUniforms};

// ---------------------------------------------------------------------------
// Marker component
// ---------------------------------------------------------------------------

/// Tags a [`PendingTexture`] entity with its splat-channel index (0–3).
#[derive(Component)]
pub struct TextureLayerIndex(pub usize);

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Watches [`CurrentHeightMap`] for changes; sets `splat_dirty` so the
/// weight map is regenerated with fresh data.
///
/// Texture-dirty is set explicitly by the UI (via [`MaterialState`]) when the
/// user actually modifies a texture parameter, avoiding the Bevy
/// `ResMut`-change-detection pitfall that would fire every frame.
///
/// During erosion visualisation the heightmap snapshot is published every N
/// frames. Regenerating the full splat weight map on each of those publishes
/// is expensive (512×512 SplatMapper + GPU texture upload), so we suppress
/// splat updates while the viz is running and allow one final update once it
/// completes.
pub fn detect_material_dirty(
    current_hm: Res<CurrentHeightMap>,
    mut mat_state: ResMut<MaterialState>,
    viz: Res<crate::core::config::ErosionVizState>,
) {
    // A new heightmap means splat weights must be recomputed, but we can
    // reuse the already-generated procedural textures.
    // Skip during active erosion viz — allow through only on the final frame
    // when viz.enabled has just been set to false (step_erosion_viz runs
    // earlier in the same .chain() so the flag is already updated).
    if current_hm.is_changed()
        && current_hm.0.is_some()
        && !mat_state.textures_dirty
        && !viz.enabled
    {
        mat_state.splat_dirty = true;
    }
}

/// Spawns four async [`PendingTexture`] tasks (grass, dirt, rock, snow) when
/// `mat_state.textures_dirty` is set.
///
/// If `texture_debounce_pending` is set (slider drag in progress), the timer
/// is ticked each frame and `textures_dirty` is only set once it expires,
/// preventing abandoned zombie tasks from saturating the thread pool.
pub fn start_texture_tasks(
    mut commands: Commands,
    mat_config: Res<MaterialConfig>,
    mut mat_state: ResMut<MaterialState>,
    pending: Query<Entity, With<TextureLayerIndex>>,
    time: Res<Time>,
) {
    if mat_state.texture_debounce_pending {
        mat_state.texture_debounce_timer.tick(time.delta());
        if mat_state.texture_debounce_timer.just_finished() {
            mat_state.texture_debounce_pending = false;
            mat_state.textures_dirty = true;
        }
    }

    if !mat_state.textures_dirty {
        return;
    }
    if !mat_config.enabled {
        mat_state.textures_dirty = false;
        // Clear any spinner the UI may be showing from a generation that was
        // in-flight when the user disabled splat materials.
        mat_state.status = MaterialStatus::Idle;
        return;
    }

    // Cancel any lingering layer entities from a previous run.
    for entity in &pending {
        commands.entity(entity).despawn();
    }

    // Clear accumulated layer handles.
    mat_state.layer_albedo = [None, None, None, None];
    mat_state.layer_normal = [None, None, None, None];
    mat_state.status = MaterialStatus::GeneratingTextures;

    let sz = mat_config.texture_size;

    let e0 = commands
        .spawn((
            PendingTexture::ground(mat_config.grass.clone(), sz, sz),
            TextureLayerIndex(0),
        ))
        .id();
    let e1 = commands
        .spawn((
            PendingTexture::ground(mat_config.dirt.clone(), sz, sz),
            TextureLayerIndex(1),
        ))
        .id();
    let e2 = commands
        .spawn((
            PendingTexture::rock(mat_config.rock.clone(), sz, sz),
            TextureLayerIndex(2),
        ))
        .id();
    let e3 = commands
        .spawn((
            PendingTexture::ground(mat_config.snow.clone(), sz, sz),
            TextureLayerIndex(3),
        ))
        .id();

    // Record the IDs of the freshly-spawned tasks so that
    // `collect_texture_results` can skip still-alive entities from the
    // previous (now-despawned-but-deferred) generation.
    mat_state.current_texture_entities = Some([e0, e1, e2, e3]);

    mat_state.textures_dirty = false;
}

/// Harvests completed [`TextureReady`] results: stores the GPU image handles
/// directly in [`MaterialState`] (no CPU copy), then despawns the layer
/// entity.  When all four layers are ready, `splat_dirty` is set.
pub fn collect_texture_results(
    mut commands: Commands,
    ready_textures: Query<(Entity, &TextureLayerIndex, &TextureReady)>,
    mut mat_state: ResMut<MaterialState>,
) {
    for (entity, layer_idx, ready) in &ready_textures {
        // Skip entities that belong to a previous (cancelled) generation.
        // Bevy's deferred despawn means those entities remain queryable for
        // the rest of the frame they were despawned in, which would otherwise
        // let stale TextureReady data overwrite layer_albedo and corrupt the
        // status state machine.
        if !mat_state
            .current_texture_entities
            .is_some_and(|ids| ids.contains(&entity))
        {
            continue;
        }
        let idx = layer_idx.0;
        mat_state.layer_albedo[idx] = Some(ready.0.albedo.clone());
        mat_state.layer_normal[idx] = Some(ready.0.normal.clone());
        commands.entity(entity).despawn();
    }

    // Once all layers are populated, trigger weight-map generation.
    if mat_state.all_layers_ready() && mat_state.status == MaterialStatus::GeneratingTextures {
        mat_state.splat_dirty = true;
        mat_state.status = MaterialStatus::Idle;
    }
}

// ---------------------------------------------------------------------------
// Texture array helpers
// ---------------------------------------------------------------------------

/// Reads the raw pixel data from four individual layer images and concatenates
/// them into a single flat buffer in layer-major order for a `texture_2d_array`.
///
/// Bevy / wgpu expects 2D array texture data laid out as:
/// `[Layer0_mip0…mipN, Layer1_mip0…mipN, Layer2_mip0…mipN, Layer3_mip0…mipN]`
///
/// Each layer image already contains its full mip chain (generated by
/// `bevy_symbios_texture`), so a plain concatenation of the four blobs
/// produces the correct layout.
///
/// Returns `None` if any handle is missing, any image is absent from the
/// asset store, or any image has no CPU-side data.
///
/// Returns `(data, format, width, height, mip_level_count)`.
fn collect_layer_data(
    handles: &[Option<Handle<Image>>; 4],
    images: &Assets<Image>,
) -> Option<(Vec<u8>, TextureFormat, u32, u32, u32)> {
    let first = images.get(handles[0].as_ref()?.id())?;
    let format = first.texture_descriptor.format;
    let w = first.texture_descriptor.size.width;
    let h = first.texture_descriptor.size.height;
    let mip_level_count = first.texture_descriptor.mip_level_count;
    let bytes_per_layer = first.data.as_ref()?.len();
    let mut merged = Vec::with_capacity(bytes_per_layer * 4);

    for h_opt in handles {
        let img = images.get(h_opt.as_ref()?.id())?;
        merged.extend_from_slice(img.data.as_ref()?);
    }

    Some((merged, format, w, h, mip_level_count))
}

// ---------------------------------------------------------------------------
// Main apply system
// ---------------------------------------------------------------------------

/// Generates the [`SplatMapper`] weight map from the current heightmap,
/// merges four per-layer images into two texture arrays (albedo + normal),
/// uploads both as GPU textures, and updates the terrain's
/// [`SplatTerrainMaterial`] with the three texture handles so the fragment
/// shader can blend them per-pixel.
///
/// Using texture arrays instead of eight discrete bindings keeps the active
/// texture unit count at 3, safely within the WebGL 2 minimum of 16 even
/// after Bevy's StandardMaterial and any global resources consume their share.
pub fn apply_splat_material(
    mut mat_state: ResMut<MaterialState>,
    mat_config: Res<MaterialConfig>,
    current_hm: Res<CurrentHeightMap>,
    terrain_config: Res<TerrainConfig>,
    splat_mat_handle: Option<Res<SplatMaterialHandle>>,
    mut materials: ResMut<Assets<crate::visuals::splat_material::SplatTerrainMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !mat_state.splat_dirty {
        return;
    }

    let Some(splat_handle) = &splat_mat_handle else {
        return;
    };

    // Disabled: reset extension to passthrough and use the base StandardMaterial
    // fallback colour (green grassland).
    if !mat_config.enabled {
        mat_state.splat_dirty = false;
        if let Some(mat) = materials.get_mut(&splat_handle.0) {
            mat.base.base_color = Color::srgb(0.35, 0.55, 0.25);
            mat.extension.uniforms.enabled = 0;
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

    // --- Collect per-layer image data into merged arrays --------------------
    // We read the CPU-side pixel data from the four individual layer images
    // (which retain MAIN_WORLD data because bevy_symbios_texture uses the
    // default RenderAssetUsages) and concatenate them into a flat buffer that
    // Image::new interprets as a texture_2d_array with depth_or_array_layers=4.
    //
    // The immutable borrows of `images` (via `collect_layer_data`) are released
    // before the mutable `images.add` / `images.remove` calls below.
    let Some((albedo_data, albedo_format, tex_w, tex_h, mip_level_count)) =
        collect_layer_data(&mat_state.layer_albedo, &images)
    else {
        // Layer images not yet available in the asset store — retry next frame.
        return;
    };
    let Some((normal_data, normal_format, _, _, _)) =
        collect_layer_data(&mat_state.layer_normal, &images)
    else {
        return;
    };

    // Remove old arrays to free GPU memory before uploading new ones.
    if let Some(old) = mat_state.albedo_array.take() {
        images.remove(old.id());
    }
    if let Some(old) = mat_state.normal_array.take() {
        images.remove(old.id());
    }

    let array_extent = Extent3d {
        width: tex_w,
        height: tex_h,
        depth_or_array_layers: 4,
    };

    let array_sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..Default::default()
    });

    let mut albedo_img = Image::new(
        array_extent,
        TextureDimension::D2,
        albedo_data,
        albedo_format,
        RenderAssetUsages::RENDER_WORLD,
    );
    albedo_img.texture_descriptor.mip_level_count = mip_level_count;
    albedo_img.sampler = array_sampler.clone();
    let albedo_array = images.add(albedo_img);

    let mut normal_img = Image::new(
        array_extent,
        TextureDimension::D2,
        normal_data,
        normal_format,
        RenderAssetUsages::RENDER_WORLD,
    );
    normal_img.texture_descriptor.mip_level_count = mip_level_count;
    normal_img.sampler = array_sampler;
    let normal_array = images.add(normal_img);

    mat_state.albedo_array = Some(albedo_array.clone());
    mat_state.normal_array = Some(normal_array.clone());

    // --- Splat weight map ---------------------------------------------------
    // Rules are authored in normalised [0, 1] space and scaled by height_scale.
    // Erosion can push individual vertices outside [0, height_scale] (deposits
    // above the peak, or erosion below sea level).  Clamping before weight
    // evaluation keeps biome boundaries stable: out-of-range vertices receive
    // the nearest-altitude layer instead of falling outside every rule and
    // producing pitch-black void pixels.
    // Only clone when values are actually out of range — for a normal
    // (non-eroded) heightmap this avoids a 1 MB+ main-thread allocation.
    let hs = terrain_config.height_scale;
    let mapper = SplatMapper::new([
        mat_config.rules[0].to_splat_rule(hs),
        mat_config.rules[1].to_splat_rule(hs),
        mat_config.rules[2].to_splat_rule(hs),
        mat_config.rules[3].to_splat_rule(hs),
    ]);
    let needs_clamp = hm.data().iter().any(|v| *v < 0.0 || *v > hs);
    let weight_map = if needs_clamp {
        let mut clamped_hm = (*hm).clone();
        for v in clamped_hm.data_mut() {
            *v = v.clamp(0.0, hs);
        }
        mapper.generate(&clamped_hm)
    } else {
        mapper.generate(hm)
    };

    // Reinterpret the [u8; 4]-per-pixel buffer as a flat &[u8] via a zero-copy
    // cast (same memory, no byte-by-byte iteration), then copy once into the
    // Vec<u8> that Image::new requires.  This avoids the per-element flat_map
    // overhead on large weight maps.
    let wm_bytes: Vec<u8> = bytemuck::cast_slice(&weight_map.data).to_vec();

    // Remove the previous weight map from the asset store.
    if let Some(old) = mat_state.weight_map.take() {
        images.remove(old.id());
    }

    let wm_handle = images.add(Image::new(
        Extent3d {
            width: weight_map.width as u32,
            height: weight_map.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        wm_bytes,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    ));
    mat_state.weight_map = Some(wm_handle.clone());

    // --- Update the SplatExtension on the terrain material ------------------
    if let Some(mat) = materials.get_mut(&splat_handle.0) {
        mat.base.base_color = Color::WHITE;
        mat.base.perceptual_roughness = 0.85;
        mat.base.metallic = 0.0;

        mat.extension.weight_map = wm_handle;
        mat.extension.albedo_array = albedo_array;
        mat.extension.normal_array = normal_array;

        let world_extent = (terrain_config.grid_size - 1) as f32 * terrain_config.cell_scale;
        mat.extension.uniforms = SplatUniforms {
            tile_scale: mat_config.tile_scale,
            enabled: 1,
            triplanar_scale: mat_config.tile_scale / world_extent.max(1.0),
            triplanar_sharpness: 4.0,
        };
    }

    mat_state.splat_dirty = false;
    mat_state.status = MaterialStatus::Ready;
}
