#version 450

#extension GL_EXT_nonuniform_qualifier : require

layout(location = 0) in vec2 tc;
layout(location = 1) in vec3 normal;
layout(location = 2) nonuniformEXT flat in uint material;

layout(location = 0) out vec4 o_Color;

layout(set = 0, binding = 0) uniform texture2D ttextures[];
layout(set = 0, binding = 1) uniform sampler ssampler;

void main() {
    o_Color = vec4(texture(sampler2D(ttextures[material], ssampler), tc).rgb, 1.0);
}
