//! Custom `MaterialExtension` that performs splat-based terrain blending in
//! the fragment shader instead of baking to a CPU texture.
//!
//! The extension binds:
//! - a RGBA splat weight map (terrain UV space, one texel per heightmap cell)
//! - one tiling albedo texture array (4 layers: Grass, Dirt, Rock, Snow)
//! - one tiling normal map texture array (4 layers)
//! - a uniform carrying `tile_scale`, `enabled`, `triplanar_scale`, and `triplanar_sharpness`
//!
//! Using texture arrays instead of 8 discrete texture bindings reduces the
//! active texture unit count from 9 down to 3, safely fitting within the
//! WebGL 2 minimum guarantee of 16 texture image units even when Bevy's
//! StandardMaterial pipeline and any global shadow maps are included.
//!
//! Bind-group slot layout (group 2, slots 100 +):
//! ```text
//! 100/101  weight_map + sampler
//! 102/103  albedo_array (texture_2d_array, 4 layers) + sampler
//! 104/105  normal_array (texture_2d_array, 4 layers) + sampler
//! 106      SplatUniforms uniform
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
/// The two texture arrays (`albedo_array`, `normal_array`) each contain 4
/// layers in order: Grass (0), Dirt (1), Rock (2), Snow (3).
///
/// All handles are non-optional.  When a handle points to an asset that has
/// not yet been loaded, Bevy's `AsBindGroup` machinery automatically
/// substitutes a 1×1 white fallback image so the bind group creation never
/// fails.  Set `uniforms.enabled = 0` to suppress the splat logic while
/// arrays are being built.
#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct SplatExtension {
    /// Splat weight map (terrain UV space, RGBA8Unorm).
    #[texture(100)]
    #[sampler(101)]
    pub weight_map: Handle<Image>,

    /// Albedo texture array — 4 layers (Grass=0, Dirt=1, Rock=2, Snow=3), sRGB.
    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    pub albedo_array: Handle<Image>,

    /// Normal map texture array — 4 layers, linear.
    #[texture(104, dimension = "2d_array")]
    #[sampler(105)]
    pub normal_array: Handle<Image>,

    #[uniform(106)]
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
