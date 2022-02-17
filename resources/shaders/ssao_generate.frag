#version 450

#define KERNEL_SIZE 16

layout(set=0, binding=0)
uniform CameraUniform {
	vec4 pos;
	mat4 view_proj;
	mat4 proj;
	mat4 inv_proj;
} Camera;

layout(set=0, binding=1)
uniform SSAOUniform {
	float radius;
	float bias;
	float contrast;
	vec2 noise_scale;
	vec3[KERNEL_SIZE] kernel;
} SSAO;

layout(set = 0, binding = 2) uniform texture2D depth_texture;
layout(set = 0, binding = 3) uniform texture2D ssao_noise;
layout(set = 0, binding = 4) uniform sampler ssampler;

layout(location = 0) in vec2 tc;

layout(location = 0) out float occlusion;


// https://mynameismjp.wordpress.com/2009/03/10/reconstructing-position-from-depth/
// This part is definitely correct
vec3 position_from_depth(float depth, vec2 tc) {
	// Get x/w and y/w from the viewport position
	float x = tc.x * 2.0 - 1.0;
	float y = (1.0 - tc.y) * 2.0 - 1.0;
	vec4 projected_position = vec4(x, y, depth, 1.0);
	// Transform by the inverse projection matrix
	vec4 position_view_space = Camera.inv_proj * projected_position;
	// Divide by w to get the view-space position
	return position_view_space.xyz / position_view_space.w;
}


vec3 normal_from_depth(float depth, vec2 tc) {
	const vec2 offset1 = vec2(0.0, 0.001);
	const vec2 offset2 = vec2(0.001, 0.0);

	vec2 tc1 = tc + offset1;
	float depth1 = texture(sampler2D(depth_texture, ssampler), tc1).r;
	
	vec2 tc2 = tc + offset2;
	float depth2 = texture(sampler2D(depth_texture, ssampler), tc2).r;
	
	vec3 p1 = vec3(offset1, depth1 - depth);
	vec3 p2 = vec3(offset2, depth2 - depth);
	vec3 normal = cross(p1, p2);
	// normal.z = -normal.z;
	return normalize(normal);
}


vec3 normal_from_z_depth(float z, vec2 tc) {
	const vec2 offset1 = vec2(0.0, 0.001);
	const vec2 offset2 = vec2(0.001, 0.0);

	vec2 tc1 = tc + offset1;
	float depth1 = texture(sampler2D(depth_texture, ssampler), tc1).r;
	float z1 = position_from_depth(depth1, tc1).z;
	
	vec2 tc2 = tc + offset2;
	float depth2 = texture(sampler2D(depth_texture, ssampler), tc2).r;
	float z2 = position_from_depth(depth2, tc2).z;
	
	vec3 p1 = vec3(offset1, z1 - z);
	vec3 p2 = vec3(offset2, z2 - z);
	vec3 normal = cross(p1, p2);
	normal.z = -normal.z;
	return normalize(normal);
}


void main() {
	
	float depth = texture(sampler2D(depth_texture, ssampler), tc).r;
	if (depth > (1.0-0.00001) || depth < (0.0+0.00001)) {
		occlusion = 1.0;
		return;
	}
	vec3 position_vs = position_from_depth(depth, tc);
	vec3 normal = normal_from_depth(depth, tc);
	//vec3 normal = normal_from_z_depth(position_vs.z, tc);

	vec2 noise_scale = vec2(800.0, 600.0) / 4.0;
	//vec3 random_vec = vec3(1.0, 0.0, 0.0);
	vec3 random_vec = texture(sampler2D(ssao_noise, ssampler), tc * noise_scale).rgb;// * 2.0 - 1.0;

	// Make TBN matrix
    vec3 tangent = normalize(random_vec - normal * dot(random_vec, normal));
    vec3 bitangent = cross(tangent, normal);
    mat3 tbn = mat3(tangent, bitangent, normal);

	float occlusion_accum = 0.0;
	for (int i = 0; i < KERNEL_SIZE; i++) {
		// get sample position vector in view space
		vec3 sample_vs = tbn * SSAO.kernel[i];				// reorient tangent -> view
		sample_vs = position_vs + sample_vs * SSAO.radius;	// make relative to fragment
		
		// project sample position
		vec4 sample_ndc = Camera.proj * vec4(sample_vs, 1.0);	// view -> clip
		sample_ndc.xyz /= sample_ndc.w;							// persp divide
		
		// get sample texture coords
		vec2 sample_uv = sample_ndc.xy * vec2(0.5, -0.5) + 0.5;
		
		// get sample depth
		float real_depth = texture(sampler2D(depth_texture, ssampler), sample_uv).r;
		
		// vec4 real_z = Camera.inv_proj * vec4(sample_uv, real_depth, 1.0);
		// real_z.xyz /= real_z.w;
		float real_z = position_from_depth(real_depth, sample_uv).z;
		
		// range check & accumulate:
		float range_check = smoothstep(0.0, 1.0, SSAO.radius / abs(sample_vs.z - real_z));
		occlusion_accum += (real_z <= (sample_vs.z + SSAO.bias) ? 1.0 : 0.0) * range_check;
	}

	occlusion = 1.0 - (occlusion_accum / float(KERNEL_SIZE));
	occlusion = pow(occlusion, SSAO.contrast);
}
