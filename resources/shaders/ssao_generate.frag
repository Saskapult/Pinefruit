#version 450

layout(location = 0) in vec2 tc;
layout(location = 0) out vec4 outAttatchment0;

layout(set=0, binding=0) uniform Camera {
	float near;
	float far;
	float fovy;
	float aspect;
	vec4 position;
	mat4 rotation;
	mat4 view;
	mat4 view_i;
	mat4 projection;
	mat4 projection_i;
	mat4 view_projection;
} camera;

layout(set=1, binding=0) uniform SSAORenderSettings {
	float tile_scale;
    float contrast;
    float bias;
	float radius;
} settings;

layout(set=1, binding=1) uniform SSAOKernel {
	vec3 samples[64];
} kernel;

layout(set = 1, binding = 2) uniform texture2D noise_texture;
layout(set = 1, binding = 3) uniform sampler noise_sampler;

layout(set = 2, binding = 0) uniform texture2D depth_texture;
layout(set = 2, binding = 1) uniform sampler depth_sampler;

// View space position
// https://mynameismjp.wordpress.com/2009/03/10/reconstructing-position-from-depth/
vec3 position_from_depth(vec2 pfdtc) {
    float depth = texture(sampler2D(depth_texture, depth_sampler), pfdtc).r;
    float x = pfdtc.x * 2.0 - 1.0;
    float y = (1.0 - pfdtc.y) * 2.0 - 1.0;
    vec4 projected_position = vec4(x, y, depth, 1.0);
    vec4 position_view_space = camera.projection_i * projected_position;
    return position_view_space.xyz / position_view_space.w;
}

vec3 normal_from_depth(vec2 nfdtc) {
	vec3 p0 = position_from_depth(nfdtc);
	vec3 p1 = position_from_depth(nfdtc + vec2(0.001, 0.0)); 
	vec3 p2 = position_from_depth(nfdtc + vec2(0.0, 0.001)); 

	vec3 normal = cross(p1 - p0, p2 - p0); // dx, dy
	return normalize(normal);
	// return normalize(cross(dFdx(p0),dFdy(p0)));
}

void main() {
	float depth = texture(sampler2D(depth_texture, depth_sampler), tc).r;
	if (depth > (1.0-0.00001) || depth < (0.0+0.00001)) {
		outAttatchment0 = vec4(1.0, 0.0, 0.0, 0.0);
		return;
	}

	// vec3 noise_vec = texture(sampler2D(noise_texture, noise_sampler), tc * settings.tile_scale).xyz;
	// noise_vec = noise_vec * vec3(2.0, 2.0, 0.0) - vec3(1.0, 1.0, 0.0);

	vec3 noise_vec = vec3(0.1, 0.1, 0.0);
    vec3 normal = normal_from_depth(tc);
    vec3 position = position_from_depth(tc);

    vec3 tangent = normalize(noise_vec - normal * dot(noise_vec, normal));
    vec3 bitangent = cross(normal, tangent);
    mat3 tbn = mat3(tangent, bitangent, normal);

    float occlusion = 0.0;
    for (int i = 0; i < 64; i++) {
		// I've removed tbn for now
        vec3 sample_pos_vs = position + tbn * kernel.samples[i] * settings.radius;
		// vec3 sample_pos_vs = position + tbn * vec3(0.0, 0.0, float(i) / 64.0) * settings.radius;
		// vec3 sample_pos_vs = position + normal * settings.radius;

        // Project sample position
		vec4 sample_pos_ndc = camera.projection * vec4(sample_pos_vs, 1.0); // view -> clip
		sample_pos_ndc.xyz /= sample_pos_ndc.w; // persp divide
		
		// Get sample texture coords
		vec2 sample_uv = sample_pos_ndc.xy * vec2(0.5, -0.5) + 0.5;
		
		// Find the recorded depth value for that sample
		// float recorded_depth = texture(sampler2D(depth_texture, depth_sampler), sample_uv).r;
		// float sample_depth = sample_pos_ndc.z;
		float recorded_depth = position_from_depth(sample_uv).z;
		float sample_depth = sample_pos_vs.z;

		float range_check = smoothstep(0.0, 1.0, settings.radius / abs(sample_depth - recorded_depth));
		occlusion += (recorded_depth >= (sample_depth - settings.bias) ? 0.0 : 1.0) * range_check;
    }

    occlusion = 1.0 - (occlusion / float(64));
	occlusion = pow(occlusion, settings.contrast);

    outAttatchment0 = vec4(occlusion, 0.0, 0.0, 0.0);
}
