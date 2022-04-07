#version 450
 
layout (binding = 0, rgba8) uniform writeonly image2D resultImage;
layout (binding = 1, r32ui) uniform readonly uimage3D worldImage;
 
layout(push_constant) uniform PushConstants {
    vec3 rayOrigin;
    float viewGridTopLeftCornerX;
 
    vec3 viewGridRight;
    float viewGridTopLeftCornerY;
 
    vec3 viewGridDown;
    float viewGridTopLeftCornerZ;
 
    float viewGridDensity;
} push;


struct Voxel {
	vec4 colour;
	// bool transparent;
};


struct Hit {
	bool did_hit;
	float t;
	vec4 colour;
}


// Tests if a ray hits a bounding box
// Notes: 
//	direction_sign[i] = direction_inverse.i < 0
//	bounds is [vec3; 2] st (bounds[0] < bounds[1])
// https://people.csail.mit.edu/amy/papers/box-jgt.pdf
// https://www.researchgate.net/publication/220494140_An_Efficient_and_Robust_Ray-Box_Intersection_Algorithm
// https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-box-intersection
bool box_jgt(vec3 origin, vec3 direction, vec3 direction_inverse, ivec3 direction_sign, float t0, float t1) {
	// bounds is [vec3; 2]

	float t_min = (bounds[direction_sign.x].x - origin.x) * direction_inverse.x; 
	float t_max = (bounds[1-direction_sign.x].x - origin.x) * direction_inverse.x; 

	float t_min_y = (bounds[direction_sign.y].y - origin.y) * direction_inverse.y; 
	float t_max_y = (bounds[1-direction_sign.y].y - origin.y) * direction_inverse.y; 

	if ((t_min > t_max_y) || (t_min_y > t_max)) {
		return false;
	}
	if (t_min_y > t_min) {
		t_min = t_min_y;
	}
	if (t_max_y < t_max) {
		t_max = t_max_y;
	}

	float t_min_z = (bounds[direction_sign.z].z - origin.z) * direction_inverse.z; 
	float t_max_z = (bounds[1-direction_sign.z].z - origin.z) * direction_inverse.z; 

	if ((t_min > t_max_z) || (t_min_z > t_max)) {
		return false;
	}
	if (t_min_z > t_min) {
		t_min = t_min_z;
	}
	if (t_max_z < t_max) {
		t_max = t_max_z;
	}

	return ((t_min < t1) && (t_max > t0));
}


// https://www.shadertoy.com/view/wdSBzK
Hit amanatides_woo(vec3 origin, vec3 direction, float t_limit) {

	const float voxel_size = 1.0;

	vec3 d = normalize(direction);

	float t_min = to_AABB(origin, d);
	vec3 new_origin = origin + t_min * d;

	ivec3 v = ivec3(floor(new_origin));
	vec3 ds = sign(d);
	ivec3 v_step = ivec3(ds);

	vec3 t_delta = voxel_size / abs(d);
	
	vec3 fr = fract(origin);
	float t_max = t_delta.x * ((d.x > 0.0) ? (1.0 - fr.x): fr.x);
	float t_max_y = t_delta.y * ((d.y > 0.0) ? (1.0 - fr.y): fr.y);
	float t_max_z = t_delta.z * ((d.z > 0.0) ? (1.0 - fr.z): fr.z);

	float t = t_min;
	vec3 norm = vec3(0.0);
	while (t < t_limit) {

		// Test for hit here please
		if (hit) {
			return Hit(true, t, vec4(0.0));
		}

		if (t_max < t_max_z) {
			if (t_max < t_max_z) {
				v.x += v_step.x;
				t = t_max;
				t_max += t_delta.x;
				norm = vec3(-ds.x, 0.0, 0.0);
			}
			else {
				v.z += v_step.z;
				t = t_max_z;
				t_max_z += t_delta.z;
				norm = vec3(0.0, 0.0, -ds.z);
			}
		}
		else {
			if (t_max_y < t_max_z) {
				v.y += v_step.y;
				t = t_max_y;
				t_max_y += t_delta.y;
				norm = vec3(0.0, -ds.y, 0.0);
			}
			else {
				v.z += v_step.z;
				t = t_max_z;
				t_max_z += t_delta.z;
				norm = vec3(0.0, 0.0, -ds.z);
			}
		}
	}

	return Hit(false, 0.0, vec4(0.0));
}

 
void main() {
    vec3 viewGridTopLeftCorner = vec3(push.viewGridTopLeftCornerX, push.viewGridTopLeftCornerY, push.viewGridTopLeftCornerZ);
    vec3 samplePoint = viewGridTopLeftCorner + push.viewGridDensity * (gl_GlobalInvocationID.x * push.viewGridRight + gl_GlobalInvocationID.y * push.viewGridDown);
 
    vec3 ray = normalize(samplePoint - push.rayOrigin);
    vec3 inverseRay = 1 / ray;
 
    ivec3 voxelCoords = ivec3(floor(push.rayOrigin.x), floor(push.rayOrigin.y), floor(push.rayOrigin.z));
    ivec3 rayOrientation = ivec3(sign(ray));
    ivec3 rayPositivity = (1 + rayOrientation) >> 1;
    vec3 withinVoxelCoords = push.rayOrigin - voxelCoords;
 
    uint blockID = 0;
    int smallestComponentIndex;
    do {
        // Calculate how far each of the neighbouring voxels are
        vec3 distanceFactor = (rayPositivity - withinVoxelCoords) * inverseRay;
 
        // Pinpoint the closest voxel of the three candidates
        smallestComponentIndex = distanceFactor.x < distanceFactor.y
        ? (distanceFactor.x < distanceFactor.z ? 0 : 2)
        : (distanceFactor.y < distanceFactor.z ? 1 : 2);
 
        // Move to that voxel
        voxelCoords[smallestComponentIndex] += rayOrientation[smallestComponentIndex];
 
        // Advance the ray the distance to the closest voxel in all dimensions
        withinVoxelCoords += ray * distanceFactor[smallestComponentIndex];
 
        // The axis towards the next voxel will now have value -1 or 1. Next line resets it to 0.
        // It can be imagined as going jumping the side of the previus voxel to the side of the next one.
        withinVoxelCoords[smallestComponentIndex] = 1 - rayPositivity[smallestComponentIndex];
        
        blockID = imageLoad(worldImage, voxelCoords).r;
    } while (blockID == 0);
 
    switch (blockID) {
        case 1:
            {
            vec3 normal = vec3(0, 0, 0);
            normal[smallestComponentIndex] = rayOrientation[smallestComponentIndex];
            imageStore(resultImage, ivec2(gl_GlobalInvocationID.xy), vec4(0.0, 1.0 * dot(ray, normal), 0.0, 1.0));
            break;
            }
        case 2:
            {
            vec3 normal = vec3(0, 0, 0);
            normal[smallestComponentIndex] = rayOrientation[smallestComponentIndex];
            imageStore(resultImage, ivec2(gl_GlobalInvocationID.xy), vec4(1.0 * dot(ray, normal), 0.0, 0.0, 1.0));
            break;
            }
        default:
            imageStore(resultImage, ivec2(gl_GlobalInvocationID.xy), vec4(0.0, 0.0, 1.0, 1.0));
    }
}