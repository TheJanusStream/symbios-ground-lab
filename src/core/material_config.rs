//! Splat-material configuration and async pipeline state.
//!
//! [`MaterialConfig`] holds per-layer splat rules (height/slope thresholds),
//! procedural texture generator configs (grass, dirt, rock, snow), and global
//! settings (texture resolution, tile scale).  [`MaterialState`] tracks the
//! async texture generation → GPU upload pipeline, including debounce timers
//! to avoid saturating the thread pool during continuous slider drags.

use bevy::prelude::*;
use bevy_symbios_texture::ground::GroundConfig;
use bevy_symbios_texture::rock::RockConfig;
use symbios_ground::splat::SplatRule;

/// Per-layer splat rule parameters (height/slope thresholds for a texture layer).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SplatRuleParams {
    pub height_min: f32,
    pub height_max: f32,
    pub slope_min: f32,
    pub slope_max: f32,
    pub sharpness: f32,
}

impl SplatRuleParams {
    /// Convert to a [`SplatRule`] with height thresholds scaled to world-space.
    ///
    /// `height_min`/`height_max` are stored in normalised [0, 1] but the
    /// `SplatMapper` operates on raw heightmap values (i.e. [0, height_scale]).
    ///
    /// The UI presents independent sliders for min/max with no cross-widget
    /// validation, so a user can drag min above max. Normalise the pair here
    /// so downstream code always receives a well-ordered range.
    pub fn to_splat_rule(&self, height_scale: f32) -> SplatRule {
        let h_lo = self.height_min.min(self.height_max);
        let h_hi = self.height_min.max(self.height_max);
        let s_lo = self.slope_min.min(self.slope_max);
        let s_hi = self.slope_min.max(self.slope_max);
        SplatRule::new(
            (h_lo * height_scale, h_hi * height_scale),
            (s_lo, s_hi),
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
            tile_scale: 64.0,
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
                    height_max: 1.0,
                    slope_min: 0.0,
                    slope_max: 0.60,
                    sharpness: 2.0,
                },
                // B — Rock: steep slopes (up to near-vertical; slope is
                // gradient magnitude so 1.0 = 45°, 10.0 covers cliffs).
                SplatRuleParams {
                    height_min: 0.0,
                    height_max: 1.0,
                    slope_min: 0.25,
                    slope_max: 10.0,
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

/// Tracks progress of the async texture generation → GPU upload pipeline.
///
/// The previous CPU-bake step has been replaced by a fragment-shader approach:
/// generated textures are kept as GPU image handles and passed directly to the
/// `SplatExtension` material.
#[derive(Resource)]
pub struct MaterialState {
    /// GPU handle for the albedo texture of each layer (filled as async tasks
    /// complete; `None` until the layer is ready).  These individual images are
    /// retained so the texture arrays can be rebuilt whenever only the splat
    /// weights change (heightmap update) without re-running texture generation.
    pub layer_albedo: [Option<Handle<Image>>; 4],
    /// GPU handle for the normal map of each layer.
    pub layer_normal: [Option<Handle<Image>>; 4],

    /// Handle to the splat weight map currently bound to the terrain material.
    /// `None` until the first splat application completes.
    pub weight_map: Option<Handle<Image>>,

    /// Handle to the 4-layer albedo texture array currently bound to the terrain
    /// material.  `None` until the first splat application completes.
    pub albedo_array: Option<Handle<Image>>,

    /// Handle to the 4-layer normal texture array currently bound to the terrain
    /// material.  `None` until the first splat application completes.
    pub normal_array: Option<Handle<Image>>,

    /// `true` when procedural texture parameters changed and textures must be
    /// re-generated from scratch.
    pub textures_dirty: bool,
    /// `true` when only the splat weights need rebuilding (heightmap or splat
    /// rule change) without regenerating procedural textures.
    pub splat_dirty: bool,

    /// Debounce state for texture regeneration: prevents a slider drag from
    /// saturating the thread pool with abandoned generation tasks.
    pub texture_debounce_pending: bool,
    pub texture_debounce_timer: Timer,

    /// Display status shown in the Material GUI.
    pub status: MaterialStatus,

    /// Entity IDs of the four [`PendingTexture`] tasks spawned in the current
    /// generation.  `collect_texture_results` uses this to skip entities that
    /// were despawned-but-not-yet-removed (deferred commands) from a previous
    /// run, preventing the stale-`TextureReady` race condition.
    pub current_texture_entities: Option<[Entity; 4]>,
}

impl Default for MaterialState {
    fn default() -> Self {
        Self {
            layer_albedo: [None, None, None, None],
            layer_normal: [None, None, None, None],
            weight_map: None,
            albedo_array: None,
            normal_array: None,
            // Start dirty so textures are generated on the first frame.
            textures_dirty: true,
            splat_dirty: false,
            status: MaterialStatus::Idle,
            texture_debounce_pending: false,
            texture_debounce_timer: Timer::from_seconds(0.4, TimerMode::Once),
            current_texture_entities: None,
        }
    }
}

impl MaterialState {
    /// Returns `true` when every layer has both an albedo and a normal-map
    /// handle available — i.e. all four async texture tasks have completed.
    pub fn all_layers_ready(&self) -> bool {
        self.layer_albedo.iter().all(|d| d.is_some())
            && self.layer_normal.iter().all(|d| d.is_some())
    }
}

/// Display state for the material pipeline, shown in the Materials panel.
#[derive(Default, Clone, PartialEq)]
pub enum MaterialStatus {
    /// No pipeline activity. Shown on startup and after a completed or
    /// cancelled generation.
    #[default]
    Idle,
    /// Async texture tasks are in flight. The panel shows a spinner and the
    /// count of completed layers.
    GeneratingTextures,
    /// All four layers are generated and the splat material is applied to the
    /// terrain.
    Ready,
}
