// Running water on top of Bevy's standard PBR material (which provides
// screen-space refraction through specular transmission and reflections of
// the environment). This shader adds:
// - flowing ripple normals from two scrolling noise layers
// - whitewater foam where the creek is steep, from the streak texture
// - a long exposure mode: smoothed ripples and stretched, silky foam
//
// UV x runs across the stream, UV y along the flow (scaled by speed, so a
// uniform scroll races down the falls and idles in pools). Vertex color
// alpha says how much whitewater the spot can have.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_view_bindings::globals,
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

struct WaterParams {
    long_exposure: f32,
    normal_strength: f32,
    flow_speed: f32,
    foam_strength: f32,
}

@group(2) @binding(100) var<uniform> water: WaterParams;
@group(2) @binding(101) var foam_tex: texture_2d<f32>;
@group(2) @binding(102) var foam_sampler: sampler;

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

fn ripple(p: vec2<f32>) -> f32 {
    return value_noise(p) * 0.65 + value_noise(p * 2.13 + vec2<f32>(5.2, 1.3)) * 0.35;
}

// Gradient of the ripple height field by central differences.
fn ripple_grad(p: vec2<f32>) -> vec2<f32> {
    let e = 0.06;
    let dx = ripple(p + vec2<f32>(e, 0.0)) - ripple(p - vec2<f32>(e, 0.0));
    let dy = ripple(p + vec2<f32>(0.0, e)) - ripple(p - vec2<f32>(0.0, e));
    return vec2<f32>(dx, dy) / (2.0 * e);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let t = globals.time;
    let le = water.long_exposure;
#ifdef VERTEX_UVS_A
    let flow = in.uv;
#else
    let flow = in.world_position.xz;
#endif
#ifdef VERTEX_COLORS
    let foam_amount = in.color.a;
#else
    let foam_amount = 0.5;
#endif

    // Ripples: two layers moving downstream at different rates.
    let s = water.flow_speed;
    let g1 = ripple_grad(vec2<f32>(flow.x * 5.0, flow.y * 1.6 + t * s));
    let g2 = ripple_grad(vec2<f32>(flow.x * 11.0 + 3.1, flow.y * 3.4 + t * s * 1.45));
    let grad = (g1 * 0.6 + g2 * 0.4) * water.normal_strength * (1.0 - 0.75 * le);

    let n = normalize(pbr_input.N);
    let up = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.z) > 0.9);
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);
    pbr_input.N = normalize(n + (tangent * grad.x + bitangent * grad.y) * 0.35);

    // Foam: crisp streaks and bubbles normally; long, smooth silk in long
    // exposure (the texture is sampled stretched along the flow).
    let foam_uv = mix(
        vec2<f32>(flow.x, flow.y + t * s),
        vec2<f32>(flow.x, flow.y * 0.25 + t * s * 0.3),
        le,
    );
    let tex = textureSample(foam_tex, foam_sampler, foam_uv).a;
    let crisp = smoothstep(0.35, 0.8, tex);
    let silky = smoothstep(0.05, 0.6, tex);
    let foam = clamp(foam_amount * mix(crisp, silky, le) * water.foam_strength, 0.0, 1.0);

    let deep = pbr_input.material.base_color;
    let white = vec4<f32>(0.90, 0.94, 0.96, 1.0);
    pbr_input.material.base_color = mix(deep, white, foam);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.55, foam);
    pbr_input.material.specular_transmission = pbr_input.material.specular_transmission * (1.0 - foam);
    pbr_input.material.diffuse_transmission = foam * 0.45;

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
