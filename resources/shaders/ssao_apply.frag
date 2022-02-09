#version 450


layout(set = 0, binding = 0) uniform texture2D input_texture;
layout(set = 0, binding = 1) uniform texture2D ssao_texture;
layout(set = 0, binding = 2) uniform sampler ssampler;

layout(location = 0) in vec2 tc;

layout(location = 0) out vec4 o_colour;


void main() {
	vec3 base = texture(sampler2D(input_texture, ssampler), tc).rgb;
	float occlusion = texture(sampler2D(ssao_texture, ssampler), tc).r;
	
	//o_colour = vec4(occlusion, occlusion, occlusion, 1.0);
	//o_colour = vec4(base * clamp(occlusion+0.3, 0.0, 1.0), 1.0);
	o_colour = vec4(base * occlusion, 1.0);
}
