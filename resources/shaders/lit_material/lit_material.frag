#version 450

layout(location = 0) in vec2 tc;
layout(location = 1) in float light;
layout(location = 0) out vec4 outAttatchment0;

layout(set = 0, binding = 1) uniform Time {
	float time;
};

layout(set = 1, binding = 0) uniform texture2D ttexture;
layout(set = 1, binding = 1) uniform sampler ssampler;

void main() {
    float adjusted_light = mix(1.0, light, (sin(time) + 1.0) / 2.0);
    // float adjusted_light = light;
    outAttatchment0 = vec4(texture(sampler2D(ttexture, ssampler), tc).rgb * adjusted_light, 1.0);
}
