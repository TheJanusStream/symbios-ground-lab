//! Custom `MaterialExtension` that performs splat-based terrain blending in
//! the fragment shader instead of baking to a CPU texture.
//!
//! The extension binds:
//! - a RGBA splat weight map (terrain UV space, one texel per heightmap cell)
//! - four tiling albedo textures (one per layer)
//! - four tiling tangent-space normal maps (one per layer)
//! - a uniform carrying `tile_scale` and `enabled`
//!
//! Bind-group slot layout (group 2, slots 100 +):
//! ```text
//! 100/101  weight_map + sampler
//! 102/103  layer_albedo_0 + sampler
//! 104/105  layer_albedo_1 + sampler
//! 106/107  layer_albedo_2 + sampler
//! 108/109  layer_albedo_3 + sampler
//! 110/111  layer_normal_0 + sampler
//! 112/113  layer_normal_1 + sampler
//! 114/115  layer_normal_2 + sampler
//! 116/117  layer_normal_3 + sampler
//! 118      SplatUniforms uniform
//! ```

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

// Path to the WGSL shader inside the `assets/` folder.
const SPLAT_SHADER_PATH: &str = "shaders/splat.wgsl";

/// Combined `tile_scale` + `enabled` flag sent to the GPU as a single
/// 16-byte-aligned uniform.
#[derive(Debug, Clone, Default, ShaderType)]
pub struct SplatUniforms {
    /// How many times the tiling textures repeat across the terrain.
    pub tile_scale: f32,
    /// Non-zero enables splat blending; zero uses the base StandardMaterial
    /// colour unchanged.
    pub enabled: u32,
    /// World-space UV scale for the Rock triplanar projection.
    /// Equals `tile_scale / world_extent` so the rock texture tiles at the
    /// same density as the top-down layers.
    pub triplanar_scale: f32,
    /// Controls how sharply the triplanar blend transitions between axes.
    /// Higher values (e.g. 4–8) tighten the blend seams; 1.0 is fully linear.
    pub triplanar_sharpness: f32,
}

/// The [`MaterialExtension`] that drives the splat shader.
///
/// All nine textures are non-optional `Handle<Image>`.  When a handle points
/// to an asset that has not yet been loaded, Bevy's `AsBindGroup` machinery
/// automatically substitutes a 1×1 white fallback image so the bind group
/// creation never fails.  Set `uniforms.enabled = 0` to suppress the splat
/// logic while textures are still loading.
#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct SplatExtension {
    /// Splat weight map (terrain UV space, RGBA8Unorm).
    #[texture(100)]
    #[sampler(101)]
    pub weight_map: Handle<Image>,

    #[texture(102)]
    #[sampler(103)]
    pub layer_albedo_0: Handle<Image>,

    #[texture(104)]
    #[sampler(105)]
    pub layer_albedo_1: Handle<Image>,

    #[texture(106)]
    #[sampler(107)]
    pub layer_albedo_2: Handle<Image>,

    #[texture(108)]
    #[sampler(109)]
    pub layer_albedo_3: Handle<Image>,

    #[texture(110)]
    #[sampler(111)]
    pub layer_normal_0: Handle<Image>,

    #[texture(112)]
    #[sampler(113)]
    pub layer_normal_1: Handle<Image>,

    #[texture(114)]
    #[sampler(115)]
    pub layer_normal_2: Handle<Image>,

    #[texture(116)]
    #[sampler(117)]
    pub layer_normal_3: Handle<Image>,

    #[uniform(118)]
    pub uniforms: SplatUniforms,
}

impl MaterialExtension for SplatExtension {
    fn fragment_shader() -> ShaderRef {
        SPLAT_SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        SPLAT_SHADER_PATH.into()
    }
}

// ---------------------------------------------------------------------------
// Type alias + resource
// ---------------------------------------------------------------------------

/// Convenience alias for the full extended-material type used by the terrain.
pub type SplatTerrainMaterial = ExtendedMaterial<StandardMaterial, SplatExtension>;

/// Resource holding the terrain entity's `SplatTerrainMaterial` handle so
/// the material pipeline systems can update it without querying the mesh.
#[derive(Resource)]
pub struct SplatMaterialHandle(pub Handle<SplatTerrainMaterial>);
