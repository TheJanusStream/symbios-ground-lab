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
//!    uploads it as a GPU texture, and updates the terrain's
//!    [`SplatTerrainMaterial`] extension with all nine texture handles.

use bevy::{
    asset::RenderAssetUsages,
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

/// Harvests completed [`TextureReady`] results: stores the GPU image handles
/// directly in [`MaterialState`] (no CPU copy), then despawns the layer
/// entity.  When all four layers are ready, `splat_dirty` is set.
pub fn collect_texture_results(
    mut commands: Commands,
    ready_textures: Query<(Entity, &TextureLayerIndex, &TextureReady)>,
    mut mat_state: ResMut<MaterialState>,
) {
    for (entity, layer_idx, ready) in &ready_textures {
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

/// Generates the [`SplatMapper`] weight map from the current heightmap,
/// uploads it as a GPU texture, and updates the terrain's
/// [`SplatTerrainMaterial`] with all layer handles so the fragment shader
/// can blend them per-pixel.
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

    // --- Splat weight map ---------------------------------------------------
    let hs = terrain_config.height_scale;
    let mapper = SplatMapper::new([
        mat_config.rules[0].to_splat_rule(hs),
        mat_config.rules[1].to_splat_rule(hs),
        mat_config.rules[2].to_splat_rule(hs),
        mat_config.rules[3].to_splat_rule(hs),
    ]);
    let weight_map = mapper.generate(hm);

    // Flatten [u8; 4] per pixel into a contiguous byte slice.
    let wm_bytes: Vec<u8> = weight_map
        .data
        .iter()
        .flat_map(|p| p.iter().copied())
        .collect();

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

        mat.extension.layer_albedo_0 =
            mat_state.layer_albedo[0].clone().unwrap_or_default();
        mat.extension.layer_albedo_1 =
            mat_state.layer_albedo[1].clone().unwrap_or_default();
        mat.extension.layer_albedo_2 =
            mat_state.layer_albedo[2].clone().unwrap_or_default();
        mat.extension.layer_albedo_3 =
            mat_state.layer_albedo[3].clone().unwrap_or_default();

        mat.extension.layer_normal_0 =
            mat_state.layer_normal[0].clone().unwrap_or_default();
        mat.extension.layer_normal_1 =
            mat_state.layer_normal[1].clone().unwrap_or_default();
        mat.extension.layer_normal_2 =
            mat_state.layer_normal[2].clone().unwrap_or_default();
        mat.extension.layer_normal_3 =
            mat_state.layer_normal[3].clone().unwrap_or_default();

        mat.extension.uniforms = SplatUniforms {
            tile_scale: mat_config.tile_scale,
            enabled: 1,
            _pad_a: 0,
            _pad_b: 0,
        };
    }

    mat_state.splat_dirty = false;
    mat_state.status = MaterialStatus::Ready;
}
