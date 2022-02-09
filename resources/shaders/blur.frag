#version 450

layout(location = 0) in vec2 tc;

layout(location = 0) out float result;

layout(set = 0, binding = 0) uniform texture2D ttexture;
layout(set = 0, binding = 1) uniform sampler ssampler;

void main() {

	vec2 texelSize = vec2(1.0, 1.0) / textureSize(sampler2D(ttexture, ssampler), 0);
    
    int s = 4;
    float sum = 0.0;
    for (int x = -s; x < s; x++) {
        for (int y = -s; y < s; y++) {
            vec2 offset = vec2(float(x), float(y)) * texelSize;
            sum += texture(sampler2D(ttexture, ssampler), tc + offset).r;
        }
    }
	result = sum / ((float(s)*2.0-1.0) * (float(s)*2.0-1.0));
    // result = texture(sampler2D(ttexture, ssampler), tc).r;
}
