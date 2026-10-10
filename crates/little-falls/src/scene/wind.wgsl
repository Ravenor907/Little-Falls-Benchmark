// Wind for foliage: the standard mesh vertex shader plus a sway offset.
// Leaves flutter in place; plants bend more the higher a vertex sits
// above the plant's base (local height).
//
// Used without a depth prepass (see materials.rs), so moving leaves can
// never disagree with a prepass depth buffer.

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
    forward_io::{Vertex, VertexOutput},
}

struct WindParams {
    // Overall sway in meters.
    strength: f32,
    // Sway for vertices at the base (leaves: 1, plants: 0).
    base: f32,
    // Extra sway per meter of local height.
    height_scale: f32,
    // Speed of the fast flutter component.
    flutter: f32,
}

@group(2) @binding(100) var<uniform> wind: WindParams;

fn sway(world: vec3<f32>, local_height: f32) -> vec3<f32> {
    let t = globals.time;
    let phase = dot(world.xz, vec2<f32>(0.21, 0.17)) + world.y * 0.1;
    // Gusts: a slow swell modulating everything.
    let gust = 0.65 + 0.35 * sin(t * 0.35 + world.x * 0.02);
    let amp = wind.strength * gust * (wind.base + wind.height_scale * max(local_height, 0.0));
    let main = sin(t * 1.7 + phase);
    let fast = sin(t * wind.flutter + phase * 2.7) * 0.35;
    // Mostly along the wind direction (down the ravine), a little across.
    let dir = normalize(vec3<f32>(0.35, 0.0, 0.94));
    let side = vec3<f32>(-dir.z, 0.0, dir.x);
    return dir * (main + fast) * amp + side * sin(t * 1.1 + phase * 1.3) * amp * 0.4
        + vec3<f32>(0.0, 1.0, 0.0) * fast * amp * 0.3;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif

#ifdef VERTEX_POSITIONS
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    world = vec4<f32>(world.xyz + sway(world.xyz, vertex.position.y), 1.0);
    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index,
    );
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index,
        world_from_local[3],
    );
#endif
    return out;
}
