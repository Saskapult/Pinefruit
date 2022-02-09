#version 450

layout(location = 0) in vec2 tc;

layout(location = 0) out vec4 accum;
layout(location = 1) out float reveal;

layout(set = 1, binding = 0) uniform texture2D ttexture;
layout(set = 1, binding = 1) uniform sampler ssampler;

void main() {

	vec4 colour = vec4(texture(sampler2D(ttexture, ssampler), tc).rgb, 1.0);

	float weight = max(min(1.0, max(max(colour.r, color.g), color.b) * color.a), color.a) * 
		clamp(0.03 / (1e-5 + pow(z / 200, 4.0)), 1e-2, 3e3);
	
	accum = vec4(color.rgb * color.a, color.a) * weight;
	
	reveal = color.a;
}
