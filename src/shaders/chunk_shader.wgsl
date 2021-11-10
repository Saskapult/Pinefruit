
// [block] means this corresponds to a buffer
// group is the bind group
// binding is the binding number of a resource in a bind group

[[block]]
struct Camera {
    pos: vec4<f32>;
    view_proj: mat4x4<f32>;        // projection_matrix * view_matrix
    inv_proj: mat4x4<f32>;        // from screen to camera
};
[[group(1), binding(0)]]
var<uniform> camera: Camera;



//
// Vertex shader
//

struct VertexInput {
    [[location(0)]] position: vec3<f32>; // Vertex position in model space
    [[location(1)]] colour: vec3<f32>;
    [[location(2)]] tex_coords: vec2<f32>;
    [[location(3)]] normal: vec3<f32>;
};
// struct InstanceInput {
//     // Model matrix (Where is model in world)
//     [[location(6)]] model_matrix_0: vec4<f32>;
//     [[location(7)]] model_matrix_1: vec4<f32>;
//     [[location(8)]] model_matrix_2: vec4<f32>;
//     [[location(9)]] model_matrix_3: vec4<f32>;
// };
struct VertexOutput {
	[[builtin(position)]] clip_position: vec4<f32>; // Position in clipping coords
    [[location(0)]] tex_coords: vec2<f32>;          // Texture coord for the fragment
    [[location(1)]] normal: vec3<f32>;              // The vertex normal
};

[[stage(vertex)]]
fn vs_main(
    model: VertexInput,
    //instance: InstanceInput,
) -> VertexOutput {
    // let model_matrix = mat4x4<f32>(
    //     instance.model_matrix_0,
    //     instance.model_matrix_1,
    //     instance.model_matrix_2,
    //     instance.model_matrix_3,
    // );

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.tex_coords = model.tex_coords;
    out.normal = model.normal;
    return out;
}



//
// Fragment shader
//

// Retreive the texture and the sampler
[[group(0), binding(0)]] var t_diffuse: texture_2d_array<f32>;
[[group(0), binding(1)]] var s_diffuse: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords, i32(1));

    let result = object_color.xyz;

    return vec4<f32>(result, object_color.a);

}
