#version 450

layout(set=0, binding=0)
uniform Camera {
	float near;
	float far;
	float fovy;
	float pad;
	vec4 camera_position;
	mat4 rotation;
	mat4 view;
	mat4 view_i;
	mat4 projection;
	mat4 projection_i;
	mat4 view_projection;
};


// Vertex input
layout(location = 0) in vec3 position;

// Instance input
layout(location = 2) in vec4 model_matrix_0;
layout(location = 3) in vec4 model_matrix_1;
layout(location = 4) in vec4 model_matrix_2;
layout(location = 5) in vec4 model_matrix_3;

layout(location = 0) out vec2 tc_f;

void main() {
	mat4 model_matrix = mat4(
		model_matrix_0,
		model_matrix_1,
		model_matrix_2,
		model_matrix_3
	);
	
	mat4 mvp = view_projection * model_matrix;

    gl_Position = mvp * vec4(position, 1.0);
	tc_f = vec2(0.0, 0.0);
}
