#version 450

layout(location = 0) in vec2 tc;
layout(location = 0) out vec4 outAttatchment0;

void main() {
    outAttatchment0 = vec4(tc, 0.0, 1.0);
}
