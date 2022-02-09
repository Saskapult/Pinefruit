#version 440

layout(set=1, binding=0)
uniform Camera {
	vec4 pos;
	mat4 view_proj;
	mat4 inv_proj;
};


// Vertex input
layout(location = 0) in vec3 position;
layout(location = 1) in vec2 tex_coords;
layout(location = 2) in vec3 normal;
layout(location = 3) in uint material;

// Instance input
layout(location = 4) in vec4 model_matrix_0;
layout(location = 5) in vec4 model_matrix_1;
layout(location = 6) in vec4 model_matrix_2;
layout(location = 7) in vec4 model_matrix_3;

// Vertex output
layout(location = 0) out vec2 o_tc;
layout(location = 1) out vec3 o_normal;
layout(location = 2) flat out uint o_material;

void main() {
	mat4 model_matrix = mat4(
		model_matrix_0,
		model_matrix_1,
		model_matrix_2,
		model_matrix_3
	);
	
	mat4 mvp = view_proj * model_matrix;

	o_normal = vec3(mvp * vec4(normal, 0.0));
	o_tc = tex_coords;
	o_material = material;
    gl_Position = mvp * vec4(position, 1.0);
}
