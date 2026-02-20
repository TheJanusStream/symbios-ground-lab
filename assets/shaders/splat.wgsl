// Fragment shader for splat-based terrain materials.
//
// Replaces the CPU bake step: the weight map and four layer textures
// (albedo + normal) are bound directly and blended per-pixel on the GPU.
//
// UVs on the terrain mesh span [0, 1] across the full terrain, so:
//   - The weight map is sampled at those UVs (one texel per heightmap cell).
//   - The layer textures are sampled at `fract(uv * tile_scale)` to tile them.
//
// When `splat_uniforms.enabled == 0` the splat logic is bypassed and the base
// StandardMaterial colour is passed through unchanged (useful for the disabled
// state and while textures are still loading).
//
// NOTE: Bevy 0.18 places the material bind group at group 3 (MATERIAL_BIND_GROUP = 3).
// All bindings use @group(#{MATERIAL_BIND_GROUP}) so this is correct regardless
// of the Bevy version.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{
        apply_pbr_lighting,
        main_pass_post_lighting_processing,
        calculate_tbn_mikktspace,
        apply_normal_mapping,
    },
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
}
#endif

// ---------------------------------------------------------------------------
// Extension bindings (slots 100+ are reserved for material extensions).
// ---------------------------------------------------------------------------

/// RGBA weight map — one texel per heightmap cell, full-terrain coverage.
/// Channels: R = grass, G = dirt, B = rock, A = snow.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var splat_weight_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var splat_weight_sampler: sampler;

/// Layer 0 (grass) albedo — tiling, sRGB.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var layer_albedo_0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var layer_albedo_0_sampler: sampler;

/// Layer 1 (dirt) albedo — tiling, sRGB.
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var layer_albedo_1: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var layer_albedo_1_sampler: sampler;

/// Layer 2 (rock) albedo — tiling, sRGB.
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var layer_albedo_2: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var layer_albedo_2_sampler: sampler;

/// Layer 3 (snow) albedo — tiling, sRGB.
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var layer_albedo_3: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var layer_albedo_3_sampler: sampler;

/// Layer 0 (grass) tangent-space normal map — tiling, linear.
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var layer_normal_0: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var layer_normal_0_sampler: sampler;

/// Layer 1 (dirt) tangent-space normal map — tiling, linear.
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var layer_normal_1: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(113) var layer_normal_1_sampler: sampler;

/// Layer 2 (rock) tangent-space normal map — tiling, linear.
@group(#{MATERIAL_BIND_GROUP}) @binding(114) var layer_normal_2: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(115) var layer_normal_2_sampler: sampler;

/// Layer 3 (snow) tangent-space normal map — tiling, linear.
@group(#{MATERIAL_BIND_GROUP}) @binding(116) var layer_normal_3: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(117) var layer_normal_3_sampler: sampler;

struct SplatUniforms {
    /// How many times the tiling textures repeat across the terrain.
    tile_scale: f32,
    /// Non-zero enables splat blending; zero passes through the base material.
    enabled: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(118) var<uniform> splat_uniforms: SplatUniforms;

// ---------------------------------------------------------------------------
// Fragment entry point
// ---------------------------------------------------------------------------

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Start from standard PBR state (reads base_color, roughness, etc. from
    // the StandardMaterial uniform; no textures are set on the base so N is
    // the interpolated vertex normal and base_color is the uniform value).
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    if splat_uniforms.enabled != 0u {
        // Sample splat weights from the terrain-space UV ([0, 1] → full mesh).
        let weights = textureSample(splat_weight_map, splat_weight_sampler, in.uv);

        // Tiled UV wraps at each tile boundary.
        let tiled_uv = fract(in.uv * splat_uniforms.tile_scale);

        // --- Albedo blend ---------------------------------------------------
        let a0 = textureSample(layer_albedo_0, layer_albedo_0_sampler, tiled_uv);
        let a1 = textureSample(layer_albedo_1, layer_albedo_1_sampler, tiled_uv);
        let a2 = textureSample(layer_albedo_2, layer_albedo_2_sampler, tiled_uv);
        let a3 = textureSample(layer_albedo_3, layer_albedo_3_sampler, tiled_uv);

        pbr_input.material.base_color =
            a0 * weights.r + a1 * weights.g + a2 * weights.b + a3 * weights.a;

        // --- Normal-map blend -----------------------------------------------
        // Sample packed tangent-space normals ([0, 1] range) and blend before
        // decoding.  Blending pre-decode is equivalent to blending post-decode
        // because the weights sum to 1 and the transform is linear.
#ifdef VERTEX_TANGENTS
        let n0 = textureSample(layer_normal_0, layer_normal_0_sampler, tiled_uv).rgb;
        let n1 = textureSample(layer_normal_1, layer_normal_1_sampler, tiled_uv).rgb;
        let n2 = textureSample(layer_normal_2, layer_normal_2_sampler, tiled_uv).rgb;
        let n3 = textureSample(layer_normal_3, layer_normal_3_sampler, tiled_uv).rgb;

        let blended_n = n0 * weights.r + n1 * weights.g + n2 * weights.b + n3 * weights.a;

        // Reconstruct the TBN frame from the vertex tangent and apply Mikktspace
        // normal mapping (decode + transform to world space).
        let tbn = calculate_tbn_mikktspace(in.world_normal, in.world_tangent);
        pbr_input.N = apply_normal_mapping(
            0u,       // no special flags: full 3-component RGB, no Y-flip
            tbn,
            false,    // terrain is single-sided
            is_front,
            blended_n,
        );
#endif // VERTEX_TANGENTS
    }

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
