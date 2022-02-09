#version 450

layout(set=0, binding=0)
uniform Camera {
	vec4 pos;
	mat4 view_proj;
	mat4 proj;
	mat4 inv_proj;
};


// Vertex input
layout(location = 0) in vec3 position;

// Instance input
layout(location = 1) in vec4 model_matrix_0;
layout(location = 2) in vec4 model_matrix_1;
layout(location = 3) in vec4 model_matrix_2;
layout(location = 4) in vec4 model_matrix_3;


void main() {
	mat4 model_matrix = mat4(
		model_matrix_0,
		model_matrix_1,
		model_matrix_2,
		model_matrix_3
	);
	
	mat4 mvp = view_proj * model_matrix;

    gl_Position = mvp * vec4(position, 1.0);
}
