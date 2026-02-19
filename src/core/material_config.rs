use bevy::prelude::*;
use bevy_symbios_texture::ground::GroundConfig;
use bevy_symbios_texture::rock::RockConfig;
use symbios_ground::splat::SplatRule;

/// Per-layer splat rule parameters (height/slope thresholds for a texture layer).
#[derive(Clone, Debug)]
pub struct SplatRuleParams {
    pub height_min: f32,
    pub height_max: f32,
    pub slope_min: f32,
    pub slope_max: f32,
    pub sharpness: f32,
}

impl SplatRuleParams {
    pub fn to_splat_rule(&self) -> SplatRule {
        SplatRule::new(
            (self.height_min, self.height_max),
            (self.slope_min, self.slope_max),
            self.sharpness,
        )
    }
}

/// All user-facing material configuration.
///
/// Bevy change detection on this resource drives the material regeneration
/// pipeline automatically — no manual dirty flags needed for the UI.
#[derive(Resource, Clone)]
pub struct MaterialConfig {
    /// Whether splat-based materials are applied to the terrain.
    pub enabled: bool,
    /// Resolution of each generated procedural texture (square).
    pub texture_size: u32,
    /// How many times the tiling textures repeat across the terrain.
    pub tile_scale: f32,

    /// Splat rules for channels R, G, B, A (Grass, Dirt, Rock, Snow).
    pub rules: [SplatRuleParams; 4],

    /// Layer 0 (R channel) — Grass.
    pub grass: GroundConfig,
    /// Layer 1 (G channel) — Dirt / soil.
    pub dirt: GroundConfig,
    /// Layer 2 (B channel) — Rock.
    pub rock: RockConfig,
    /// Layer 3 (A channel) — Snow / ice.
    pub snow: GroundConfig,
}

impl Default for MaterialConfig {
    fn default() -> Self {
        let grass = GroundConfig {
            seed: 1,
            macro_scale: 2.5,
            macro_octaves: 4,
            micro_scale: 10.0,
            micro_octaves: 3,
            micro_weight: 0.3,
            color_dry: [0.30, 0.48, 0.15],
            color_moist: [0.14, 0.28, 0.07],
            normal_strength: 1.5,
        };

        let snow = GroundConfig {
            seed: 99,
            macro_scale: 4.0,
            macro_octaves: 3,
            micro_scale: 12.0,
            micro_octaves: 3,
            micro_weight: 0.4,
            color_dry: [0.95, 0.95, 0.98],
            color_moist: [0.80, 0.82, 0.88],
            normal_strength: 0.8,
        };

        Self {
            enabled: true,
            texture_size: 512,
            tile_scale: 8.0,
            rules: [
                // R — Grass: low altitude, gentle slope
                SplatRuleParams {
                    height_min: 0.0,
                    height_max: 0.45,
                    slope_min: 0.0,
                    slope_max: 0.30,
                    sharpness: 4.0,
                },
                // G — Dirt: mid altitude, any slope
                SplatRuleParams {
                    height_min: 0.30,
                    height_max: 0.65,
                    slope_min: 0.0,
                    slope_max: 0.60,
                    sharpness: 2.0,
                },
                // B — Rock: steep slopes
                SplatRuleParams {
                    height_min: 0.0,
                    height_max: 1.0,
                    slope_min: 0.25,
                    slope_max: 1.0,
                    sharpness: 3.0,
                },
                // A — Snow: high altitude, gentle slope
                SplatRuleParams {
                    height_min: 0.70,
                    height_max: 1.0,
                    slope_min: 0.0,
                    slope_max: 0.35,
                    sharpness: 4.0,
                },
            ],
            grass,
            dirt: GroundConfig::default(),
            rock: RockConfig::default(),
            snow,
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Tracks progress of the async texture generation → CPU bake → GPU upload pipeline.
#[derive(Resource)]
pub struct MaterialState {
    /// Raw albedo pixels (RGBA8 sRGB) for each of the 4 layers, filled in as
    /// async tasks complete.
    pub layer_albedo: [Option<Vec<u8>>; 4],
    /// Raw normal pixels (RGBA8 Unorm) for each layer.
    pub layer_normal: [Option<Vec<u8>>; 4],
    /// Side length of each layer texture (all layers share the same size).
    pub layer_tex_size: u32,

    /// Handle to the baked albedo image currently applied to the terrain.
    pub baked_albedo: Option<Handle<Image>>,
    /// Handle to the baked normal image currently applied to the terrain.
    pub baked_normal: Option<Handle<Image>>,

    /// `true` when procedural texture parameters changed and textures must be
    /// re-generated from scratch.
    pub textures_dirty: bool,
    /// `true` when only the splat weights need rebuilding (heightmap or splat
    /// rule change) without regenerating procedural textures.
    pub splat_dirty: bool,

    /// Display status shown in the Material GUI.
    pub status: MaterialStatus,
}

impl Default for MaterialState {
    fn default() -> Self {
        Self {
            layer_albedo: [None, None, None, None],
            layer_normal: [None, None, None, None],
            layer_tex_size: 0,
            baked_albedo: None,
            baked_normal: None,
            // Start dirty so textures are generated on the first frame.
            textures_dirty: true,
            splat_dirty: false,
            status: MaterialStatus::Idle,
        }
    }
}

impl MaterialState {
    pub fn all_layers_ready(&self) -> bool {
        self.layer_albedo.iter().all(|d| d.is_some())
    }
}

#[derive(Default, Clone, PartialEq)]
pub enum MaterialStatus {
    #[default]
    Idle,
    GeneratingTextures,
    Ready,
}

/// Stores the terrain's `StandardMaterial` handle so the material systems can
/// update textures without querying the mesh entity every frame.
#[derive(Resource)]
pub struct TerrainMaterialHandle(pub Handle<StandardMaterial>);
