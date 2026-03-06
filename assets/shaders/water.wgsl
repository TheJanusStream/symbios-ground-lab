#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::globals,
    forward_io::{VertexOutput, FragmentOutput},
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Get the base material properties (Color, Roughness, etc.)
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // 1. Procedural Waves via Sum-of-Sines
    let t = globals.time;
    let pos = in.world_position.xyz;
    
    // Mix overlapping sine waves to create chaotic peaks and valleys
    let wave_x = sin(pos.x * 1.5 + t * 2.0) * 0.1 
               + cos(pos.z * 0.8 - t * 1.5) * 0.05;
               
    let wave_z = cos(pos.z * 1.5 + t * 1.8) * 0.1 
               + sin(pos.x * 0.9 - t * 1.2) * 0.05;

    // 2. Perturb the normal (default upward is Y in Bevy)
    // The geometry is a flat box top, so base normal is (0, 1, 0)
    let perturbed_normal = normalize(vec3<f32>(wave_x, 1.0, wave_z));
    
    // Feed the moving normal into the PBR lighting equation
    pbr_input.N = perturbed_normal;

    // 3. Apply standard Bevy lighting
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    return out;
}