#version 450

layout(location = 0) in vec2 tc;
layout(location = 0) out vec4 outAttatchment0;

layout(set = 0, binding = 0) uniform texture2D input_texture;
layout(set = 0, binding = 1) uniform texture2D occlusion_texture;
layout(set = 0, binding = 2) uniform sampler ssampler;

void main() {
    vec4 base = texture(sampler2D(input_texture, ssampler), tc);
	float occlusion = texture(sampler2D(occlusion_texture, ssampler), tc).r;

    // outAttatchment0 = vec4(base.rgb * occlusion, base.a);
    outAttatchment0 = vec4(occlusion, occlusion, occlusion, base.a);
    // outAttatchment0 = vec4(texture(sampler2D(occlusion_texture, ssampler), tc).xyz, base.a);
}
