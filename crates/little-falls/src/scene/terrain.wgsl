// Forest floor: blends three scanned surfaces on top of Bevy's standard
// PBR material. The base material carries the moss floor (albedo + normal
// map); damp mud near the water and bare rock on steep slopes are mixed in
// using per-vertex weights stored in the second UV channel.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct TerrainParams {
    mud_scale: f32,
    rock_scale: f32,
    macro_strength: f32,
    wet_roughness: f32,
}

@group(2) @binding(100) var<uniform> terrain: TerrainParams;
@group(2) @binding(101) var mud_tex: texture_2d<f32>;
@group(2) @binding(102) var mud_sampler: sampler;
@group(2) @binding(103) var rock_tex: texture_2d<f32>;
@group(2) @binding(104) var rock_sampler: sampler;

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_UVS_B
    let w = clamp(in.uv_b, vec2<f32>(0.0), vec2<f32>(1.0));
#else
    let w = vec2<f32>(0.0);
#endif
#ifdef VERTEX_UVS_A
    let uv = in.uv;
#else
    let uv = in.world_position.xz * 0.4;
#endif

    let mud = textureSample(mud_tex, mud_sampler, uv * terrain.mud_scale);
    let rock = textureSample(rock_tex, rock_sampler, uv * terrain.rock_scale);
    var c = pbr_input.material.base_color;
    c = mix(c, mud, w.x);
    c = mix(c, rock, w.y);

    // Large-scale brightness variation breaks up visible texture tiling.
    let p = in.world_position.xz;
    let m = value_noise(p * 0.07) * 0.6 + value_noise(p * 0.31) * 0.4;
    let k = mix(1.0 - terrain.macro_strength, 1.0 + terrain.macro_strength, m);
    pbr_input.material.base_color = vec4<f32>(c.rgb * k, c.a);

    // Damp ground near the creek is darker and glossier.
    pbr_input.material.perceptual_roughness =
        mix(pbr_input.material.perceptual_roughness, terrain.wet_roughness, w.x * 0.7);

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
