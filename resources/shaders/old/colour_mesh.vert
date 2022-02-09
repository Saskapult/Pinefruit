#version 440

layout(set=0, binding=0)
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

// Instance MM input
layout(location = 4) in vec4 model_matrix_0;
layout(location = 5) in vec4 model_matrix_1;
layout(location = 6) in vec4 model_matrix_2;
layout(location = 7) in vec4 model_matrix_3;

// Instance colour input
layout(location = 8) in vec3 colour;

// Vertex output
layout(location = 0) out vec3 o_colour;

void main() {
	mat4 model_matrix = mat4(
		model_matrix_0,
		model_matrix_1,
		model_matrix_2,
		model_matrix_3
	);
	
	mat4 mvp = view_proj * model_matrix;

	o_colour = colour;
    gl_Position = mvp * vec4(position, 1.0);
}
