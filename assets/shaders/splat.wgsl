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
    /// World-space UV scale for the Rock triplanar projection.
    /// Equals tile_scale / world_extent so density matches the top-down layers.
    triplanar_scale: f32,
    /// Blend sharpness for the triplanar axis transition (k >= 1; 4 is good).
    triplanar_sharpness: f32,
}

// ---------------------------------------------------------------------------
// Triplanar helpers (used for the Rock layer only)
// ---------------------------------------------------------------------------

/// Compute per-axis blend weights from the world-space surface normal.
/// |normal| is assumed to be unit length; k sharpens the transition seam.
fn triplanar_weights(world_normal: vec3<f32>, k: f32) -> vec3<f32> {
    var w = pow(abs(world_normal), vec3<f32>(k));
    return w / (w.x + w.y + w.z + 0.0001);
}

/// Sample a texture using triplanar world-space projection and return vec4.
/// Three lookups — YZ, XZ, XY planes — are blended by `weights`.
fn triplanar_albedo(
    tex: texture_2d<f32>,
    samp: sampler,
    world_pos: vec3<f32>,
    scale: f32,
    weights: vec3<f32>,
) -> vec4<f32> {
    let col_x = textureSample(tex, samp, fract(world_pos.zy * scale));
    let col_y = textureSample(tex, samp, fract(world_pos.xz * scale));
    let col_z = textureSample(tex, samp, fract(world_pos.xy * scale));
    return col_x * weights.x + col_y * weights.y + col_z * weights.z;
}

/// Sample a normal map using triplanar projection and return packed rgb vec3.
/// The result is in the same [0, 1] packed space as a regular normal-map
/// sample, so it can be blended with other layers before decoding.
fn triplanar_normal(
    tex: texture_2d<f32>,
    samp: sampler,
    world_pos: vec3<f32>,
    scale: f32,
    weights: vec3<f32>,
) -> vec3<f32> {
    let n_x = textureSample(tex, samp, fract(world_pos.zy * scale)).rgb;
    let n_y = textureSample(tex, samp, fract(world_pos.xz * scale)).rgb;
    let n_z = textureSample(tex, samp, fract(world_pos.xy * scale)).rgb;
    return n_x * weights.x + n_y * weights.y + n_z * weights.z;
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
        let raw_weights = textureSample(splat_weight_map, splat_weight_sampler, in.uv);

        // Normalise weights so they always sum to exactly 1.  SplatMapper rules
        // can overlap or leave gaps, causing the raw sum to differ from 1.  A
        // non-unit sum corrupts the normal decode step: encoded normals are in
        // [0, 1] and decoded via `* 2 - 1`.  If the blended vector drifts
        // toward (0.5, 0.5, 0.5) the decoded result is (0, 0, 0), and
        // normalising a zero vector produces NaN that infects the whole pixel.
        let weight_sum = max(raw_weights.r + raw_weights.g + raw_weights.b + raw_weights.a, 0.0001);
        let weights = raw_weights / weight_sum;

        // Tiled UV wraps at each tile boundary (used by grass, dirt, snow).
        let tiled_uv = fract(in.uv * splat_uniforms.tile_scale);

        // Triplanar data for the Rock layer — computed once, shared by albedo
        // and normal sampling below.
        let world_pos = in.world_position.xyz;
        let tp_weights = triplanar_weights(
            in.world_normal,
            splat_uniforms.triplanar_sharpness,
        );

        // --- Albedo blend ---------------------------------------------------
        // Grass, Dirt, Snow use standard top-down tiled UVs (1 lookup each).
        // Rock uses triplanar world-space projection (3 lookups) to eliminate
        // texture stretching on steep cliff faces.
        let a0 = textureSample(layer_albedo_0, layer_albedo_0_sampler, tiled_uv);
        let a1 = textureSample(layer_albedo_1, layer_albedo_1_sampler, tiled_uv);
        let a2 = triplanar_albedo(
            layer_albedo_2, layer_albedo_2_sampler,
            world_pos, splat_uniforms.triplanar_scale, tp_weights,
        );
        let a3 = textureSample(layer_albedo_3, layer_albedo_3_sampler, tiled_uv);

        pbr_input.material.base_color =
            a0 * weights.r + a1 * weights.g + a2 * weights.b + a3 * weights.a;

        // --- Normal-map blend -----------------------------------------------
        // Sample packed tangent-space normals ([0, 1] range) and blend before
        // decoding.  Blending pre-decode is equivalent to blending post-decode
        // when the weights are normalised (unit sum, linear transform).
        // Rock normals use triplanar sampling for the same reason as albedo.
#ifdef VERTEX_TANGENTS
        let n0 = textureSample(layer_normal_0, layer_normal_0_sampler, tiled_uv).rgb;
        let n1 = textureSample(layer_normal_1, layer_normal_1_sampler, tiled_uv).rgb;
        let n2 = triplanar_normal(
            layer_normal_2, layer_normal_2_sampler,
            world_pos, splat_uniforms.triplanar_scale, tp_weights,
        );
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
