#version 450

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


layout(set=1, binding=0) uniform ChunkAcceleratorInfo {
	uint extent;
	uint chunk_st;
} voxel_info;
layout(set=1, binding=1) readonly buffer OctreeVolumeData {
	uint contents[];
} voxel_data;
layout(set=1, binding=2) readonly buffer ColourData {
	uint colours[];
} colour_data;


layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 outAttatchment0;


void to_node(uint data, out uint content, out uint node, out uint leaf) {
	leaf = (data & 0xFF000000) >> 24;
	node = (data & 0x00FF0000) >> 16;
	content = (data & 0x0000FFFF) >> 0;
	return;
}


vec4 to_colour_rgba8(uint data) {
	vec4 colour;
	colour.r = float((data & 0xFF000000) >> 24);
	colour.g = float((data & 0x00FF0000) >> 16);
	colour.b = float((data & 0x0000FF00) >> 8);
	colour.a = float((data & 0x000000FF) >> 0);
	colour /= 255.0;
	return colour;
}


// Fetches the index of the chunk at this position
// Returns:
// - zero if not in bounds
// - one if chunk not loaded
// - index+2
uint chunk_at(ivec3 position) {
	bool out_of_bounds = any(greaterThan(abs(position), ivec3(voxel_info.extent / 2)));
	if (out_of_bounds) {
		return 0;
	}

	// uvec3 i = uvec3(position + ivec3(voxel_info.extent / 2));
	uvec3 i = uvec3(position + ivec3(voxel_info.extent / 2));
	uint index = i.x * voxel_info.extent * voxel_info.extent + i.y * voxel_info.extent + i.z;

	// Will already be 0 or index+1 so we just add one to conformt to our spec
	return voxel_data.contents[voxel_info.chunk_st + index] + 1;
}


// Finds the offset for this octant
uint octant_offset(uint octant, uint leaf, uint node, uint leaf_size) {
	// Unlike in rust, this will be 0x..FF if octant = 8
	uint preceding_mask = 255 - (0xFF >> octant);
	uint prededing_leaves = bitCount(leaf & preceding_mask);
	uint prededing_nodes = bitCount(node & preceding_mask);
	uint offset = prededing_leaves * leaf_size + prededing_nodes;
	return offset;
}


// Finds the content of an octree at some index with some position
// could have out has_leaf to signify having hit something!
uint octree_get(uint index, ivec3 vpos) {
	bool in_bounds = (all(lessThanEqual(vpos, ivec3(15))) && all(greaterThanEqual(vpos, ivec3(0))));
	if (!in_bounds) return 0;

	// Read root
	uint content;
	uint node;
	uint leaf;
	to_node(voxel_data.contents[index], content, node, leaf);

	// Find next octant
	uint hel = 16 / 2;
	bvec3 octant_cmp = greaterThanEqual(vpos, ivec3(hel));
	vpos -= ivec3(octant_cmp) * ivec3(hel);
	uint octant = uint(octant_cmp.x) * 4 + uint(octant_cmp.y) * 2 + uint(octant_cmp.z);
	uint octant_mask = 1 << (7 - octant);
	
	// Move to next octant
	bool has_leaf = (leaf & octant_mask) != 0;
	bool has_node = (node & octant_mask) != 0;
	index += 1 + octant_offset(octant, leaf, node, 1); // root content is always 0

	while(has_node && hel != 0) {
		index += 1 * int(has_leaf); // Skip leaf

		// Read node
		to_node(voxel_data.contents[index], content, node, leaf);

		// Find next octant
		hel /= 2;
		octant_cmp = greaterThanEqual(vpos, ivec3(hel));
		vpos -= ivec3(octant_cmp) * ivec3(hel);
		octant = uint(octant_cmp.x) * 4 + uint(octant_cmp.y) * 2 + uint(octant_cmp.z);
		octant_mask = 1 << (7 - octant);

		// Move to next octant
		has_leaf = (leaf & octant_mask) != 0;
		has_node = (node & octant_mask) != 0;
		index += content + 1 + octant_offset(octant, leaf, node, 1);		
	}

	// This could be branchless because of multiplication
	if (has_leaf) {
		return voxel_data.contents[index] + 1;
	}
	else {
		return 0;
	}
}


uint chunk_get(uint index, ivec3 vpos) {
	bool in_bounds = (all(lessThanEqual(vpos, ivec3(15))) && all(greaterThanEqual(vpos, ivec3(0))));
	if (!in_bounds) return 0;

	index += vpos.x * 16 * 16 + vpos.y * 16 + vpos.z;
	return voxel_data.contents[index];
}


vec3 bl_ray(uint chunk_index, vec3 origin, vec3 direction, float tlimit, out uint out_index, out float t, inout vec3 normal) {
	out_index = 0;
	t = 0.0;

	ivec3 vpos = ivec3(floor(origin));

	ivec3 vstep = ivec3(sign(direction));
	vec3 tdelta = abs(vec3(1.0 / direction));

	vec3 tmax;
	tmax.x = direction.x < 0 ? origin.x - floor(origin.x) : 1.0 - origin.x + floor(origin.x);
	tmax.y = direction.y < 0 ? origin.y - floor(origin.y) : 1.0 - origin.y + floor(origin.y);
	tmax.z = direction.z < 0 ? origin.z - floor(origin.z) : 1.0 - origin.z + floor(origin.z);
	tmax *= tdelta;

	int iters = 0;
	bvec3 dmask;
	while (true) {
		if (iters >= 50) return vec3(0.0, 0.86, 1.0); // Debug purple
		if (t >= tlimit) return vec3(0.0, 0.0, 1.0); // Red
		iters += 1;

		bool inbounds = all(lessThanEqual(vpos, ivec3(15))) && all(greaterThanEqual(vpos, ivec3(0)));

		uint v_content = octree_get(chunk_index, vpos);
		// uint v_content = chunk_get(chunk_index, vpos);
		if (inbounds && v_content != 0) {
			out_index = v_content;
			
			// We could store the colours directly in the octree, 
			// but this is so much more *flexible*! 
			return to_colour_rgba8(colour_data.colours[v_content-1]).xyz;
		}

		dmask = lessThanEqual(tmax.xyz, min(tmax.yzx, tmax.zxy));
		normal = vec3(dmask) * vec3(-vstep); // Weird stuff happends if we move it to the result computation
		t = min(tmax.x, min(tmax.y, tmax.z));
		vpos += ivec3(vec3(dmask)) * vstep;
		tmax += vec3(dmask) * tdelta;
	}
	return vec3(0.0);
}


// Casts a ray through the high-level structure (the chunks)
// For each intersected chunk, casts another ray through its contents
// Returns index of hit block (zero or index+1) (might change to a bool and have this return a vec4 of colour)
vec3 hl_ray(vec3 origin, vec3 direction, float tlimit, out float hit, out uint index, out float t) {
	hit = 0.0;
	index = 0;
	t = 0.0;

	float vscale = float(16);
	ivec3 vpos = ivec3(floor(origin / vscale));
	ivec3 vstep = ivec3(sign(direction));
	vec3 tdelta = abs(vec3(vscale / direction));

	vec3 tmax; // (fract or 1-fract depending on direction) * vscale / abs(dir)
	vec3 o = origin / vscale;
	tmax.x = direction.x < 0 ? o.x - floor(o.x) : 1.0 - o.x + floor(o.x);
	tmax.y = direction.y < 0 ? o.y - floor(o.y) : 1.0 - o.y + floor(o.y);
	tmax.z = direction.z < 0 ? o.z - floor(o.z) : 1.0 - o.z + floor(o.z);
	tmax *= tdelta;
	
	vec3 normal = vec3(0.0);
	int iters = 0;
	bvec3 mask;
	while (true) {

		if (iters >= 100) return vec3(1.0, 0.0, 0.86); // Off white if y = 1.0
		if (t >= tlimit) return vec3(1.0, 1.0, 0.0); // Yellow
		iters += 1;

		uint chunk_res = chunk_at(vpos);
		
		if (chunk_res > 1) {
			uint chunk_index = chunk_res - 2;

			vec3 hit_location = origin + direction * t;
			vec3 chunk_location = vec3(vpos) * vscale;
			vec3 bl_origin = hit_location - chunk_location; // Position of hit relative to chunk origin
			uint bl_index;
			float bl_t;
			vec3 bl_normal = normal;
			vec3 bl_result = bl_ray(chunk_index, bl_origin, direction, min(tlimit - t, 28.0), bl_index, bl_t, bl_normal);

			if (bl_index != 0) {
				float light_angle = acos(dot(bl_normal, normalize(vec3(3.0, 5.0, 7.0))));
				float perc = 1.0 - light_angle / 3.15;

				index = bl_index;
				return bl_result * perc;
			}
		}

		mask = lessThanEqual(tmax.xyz, min(tmax.yzx, tmax.zxy));
		normal = vec3(mask) * vec3(-vstep);
		t = min(tmax.x, min(tmax.y, tmax.z));
		vpos += ivec3(vec3(mask)) * vstep;
		tmax += vec3(mask) * tdelta;
	}
}


void main() {
	vec3 origin = mod(vec3(camera.position), vec3(16.0));
	// put uv in [-1, 1] space, also adjust by aspect
	vec2 new_uv = (uv * vec2(2.0) - vec2(1.0)) * vec2(camera.aspect, -1.0);
	vec3 unrotated_direction = vec3(new_uv, tan(camera.fovy));
	vec3 direction = normalize(vec3(camera.rotation * vec4(unrotated_direction, 1.0)));

	uint index;
	float t;
	float hit;
	vec3 proto_result = hl_ray(origin, direction, float(voxel_info.extent)*16.0/2.0, hit, index, t);
	if (index == 0) {
		discard;
	}

	outAttatchment0 = vec4(proto_result, 1.0);
}
