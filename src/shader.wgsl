
// [block] means this corresponds to a buffer
// group is the bind group
// binding is the binding number of a resource in a bind group

// [[block]]
// struct Uniforms {
//     proj: mat4x4<f32>;      // from camera to screen
//     proj_inv: mat4x4<f32>;  // from screen to camera
//     view: mat4x4<f32>;      // from world to camera
//     cam_pos: vec4<f32>;     // camera position
// };
// [[group(0), binding(0)]]
// var<uniform> uniforms: Uniforms;

[[block]]
struct Camera {
    pos: vec4<f32>;
    view_proj: mat4x4<f32>;        // projection_matrix * view_matrix
    inv_proj: mat4x4<f32>;        // from screen to camera
};
[[group(1), binding(0)]]
var<uniform> camera: Camera;

// [[block]]
// struct Light {
//     position: vec3<f32>;
//     color: vec3<f32>;
// };
// [[group(2), binding(0)]]
// var<uniform> light: Light;




//
// Vertex shader
//

struct VertexInput {
    [[location(0)]] position: vec3<f32>;    // Vertex position in model space
    [[location(1)]] tex_coords: vec2<f32>;  // Vertex texture coords
    [[location(2)]] normal: vec3<f32>;      // Vertex normal
};
struct InstanceInput {
    // Model matrix (Where is model in world)
    [[location(5)]] model_matrix_0: vec4<f32>;
    [[location(6)]] model_matrix_1: vec4<f32>;
    [[location(7)]] model_matrix_2: vec4<f32>;
    [[location(8)]] model_matrix_3: vec4<f32>;
};
struct VertexOutput {
	[[builtin(position)]] clip_position: vec4<f32>; // Position in clipping coords
    [[location(0)]] tex_coords: vec2<f32>;          // Texture coord for the fragment
    [[location(1)]] normal: vec3<f32>;              // The vertex normal
};

[[stage(vertex)]]
fn main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    var out: VertexOutput;
    out.clip_position = camera.view_proj * model_matrix * vec4<f32>(model.position, 1.0);
    out.tex_coords = model.tex_coords;
    out.normal = model.normal;
    return out;
}



//
// Fragment shader
//

// Retreive the texture and the sampler
[[group(0), binding(0)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(1)]]
var s_diffuse: sampler;
[[group(0), binding(2)]]
var t_normal: texture_2d<f32>;
[[group(0), binding(3)]]
var s_normal: sampler;

[[stage(fragment)]]
fn main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    //let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.tex_coords);

    
    // // We don't need (or want) much ambient light, so 0.1 is fine
    // let ambient_strength = 0.1;
    // let ambient_color = light.color * ambient_strength;

    // let tangent_normal = object_normal.xyz * 2.0 - 1.0;

    // let light_dir = normalize(light.position - in.world_position);
    // let view_dir = normalize(camera.view_pos.xyz - in.world_position);
    // //let reflect_dir = reflect(-light_dir, in.world_normal);
    // let half_dir = normalize(view_dir + light_dir);

    // //let specular_strength = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);
    // let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
    // let specular_color = specular_strength * light.color;

    // let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
    // let diffuse_color = light.color * diffuse_strength;

    let result = object_color.xyz;

    return vec4<f32>(result, object_color.a);

}
