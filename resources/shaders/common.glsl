
struct Camera {
	vec4 pos;
	mat4 rot;

	mat4 view_proj;
	mat4 proj;
	mat4 inv_proj;

	
	float fovy; // radians
	float near; // 1.0 / tan(fovy/2)
};


