#version 450


//layout(location = 0) in vec3 pos;

layout(location = 0) out vec2 o_tc;


void main() {
	//o_tc = (pos.xy + vec2(1.0)) / 2.0;
	//gl_Position = vec4(pos, 1.0);

	// https://www.saschawillems.de/blog/2016/08/13/vulkan-tutorial-on-rendering-a-fullscreen-quad-without-buffers/
	//int tc0 = gl_VertexIndex / 2; 
	// float tc0 = gl_VertexIndex == 2 ? 3.0 : -1.0; 
	int tc0 = gl_VertexIndex & 2;
	//int tc1 = gl_VertexIndex & 1; 
	//float tc1 = gl_VertexIndex == 1 ? 3.0 : -1.0; 
	int tc1 = gl_VertexIndex << 1 & 2;
	// 0 -> [0, 0]
	// 1 -> [0, 1]
	// 2 -> [1, 0]
	o_tc = vec2(tc0, tc1);
	//vec2 pos = vec2(, );
	//gl_Position = vec4(tc0 * 2.0 - 1.0, (1.0 - tc1) * 2.0 - 1.0, 0.0, 1.0);
	//gl_Position = vec4(o_tc - 1.0, 0.0, 1.0);
	gl_Position = vec4(mix(vec2(-1.0, 1.0), vec2(1.0, -1.0), o_tc), 0.0, 1.0);
}
