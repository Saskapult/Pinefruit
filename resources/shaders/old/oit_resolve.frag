#version 450

layout(location = 0) in vec2 tc;

layout(location = 0) out vec4 colour;

layout(set = 1, binding = 0) uniform texture2D accum;
layout(set = 1, binding = 1) uniform texture2D reveal;
layout(set = 1, binding = 2) uniform sampler ssampler;

void main() {
	vec4 accum =  vec4(texture(sampler2D(reveal, ssampler), tc).rgb, 1.0);
	float reveal = texture(sampler2D(reveal, ssampler), tc).r;
	
	colour = vec4(accum.rgb / max(accum.a, 1e-5), reveal);
}
