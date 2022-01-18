
// Make a colured mesh renderer
// Colour passed as uniform value?
// Push constant?

/*
Have material containing render and physics data

Have shader descriptor
Can extract material properties into a bind group matching shader descriptor

For each material not compiled
Compile with colour_mesh shader (will do nothing)
Render all with that

*/


pub enum GraphResourceType {
	Buffer,
	Texture,
}

struct GraphContext {
	buffers: Vec<wgpu::Buffer>,
	buffers_id: HashMap<String, usize>,
	textures: Vec<Texture>,
}
impl GraphContext {
	pub fn get_id(&self, id: &String, resource_type: GraphResourceType) -> Option<usize> {
		let holder = match resource_type {
			_ => &self.buffers,
		};
		if holder.contains_key(id) {
			Some(holder[id])
		} else {
			None
		}
	}
}


struct RenderGraph {
	// inputs: Vec<String>,
	// outputs: Vec<String>,
	// nodes: Vec<RenderNode>,
	// edges: Vec<RenderEdge>,
}



trait RenderNode {
	fn inputs(&self) -> Vec<(String, ResourceType)>;
	fn outputs(&self) -> Vec<(String, ResourceType)>;
	fn run(&self, context: &mut GraphContext);
}



struct ShaderNode {
	shader_idx: usize,
	inputs: Vec<(String, GraphResourceType)>,
	outputs: Vec<(String, GraphResourceType)>,
}
impl RenderNode for ShaderNode {
	fn inputs(&self) -> Vec<(String, ResourceType)> {
		self.inputs
	}
	fn outputs(&self) -> Vec<(String, ResourceType)> {
		self.outputs
	}
	fn run(&self, context: &mut GraphContext) {
		todo!()
	}
}



struct ShaderNodeSpecification {
	shader: PathBuf,
}


// Lua node?





// Vetrex output
// Fragment output
// Sample texture array
//


struct RenderEdge {
	to: usize,
	from: usize,
}


enum ResourceType {
	Texture,
	TextureArray,
	ArrayTexture,
	Buffer(u32),	// u32 for offset
	Float,
	Float3,
}


use nalgebra::*;

// Middle of voxel
fn dual_contour_vertex_simple(
	function: u32, 
	normal: Vector3<f32>, 
	coords: [i32; 3],
) -> Vector3<f32> {
	let x = coords[0] as f32 + 0.5;
	let y = coords[1] as f32 + 0.5;
	let z = coords[2] as f32 + 0.5;
	Vector3::new(x, y, z)
}

// Weird stuff might not work
fn dual_contour_vertex() -> Vector3<f32> {

}


struct Octree<
	T: Debug + Eq + PartialEq,
> {
	dimension: u32,
	root: OctreeNode<T>,
}
impl Octree {
	fn new() -> Self {
		todo!()
	}

	fn get(position: [i32; 3]) -> T {
		// Find octant?
		todo!()
	}

	fn insert(position: [i32; 3], value: T) {
		todo!()
	}
}


struct OctreeNode<
	T: Debug + Eq + PartialEq,
> {
	children: [Option<OctreeNode<T>>; 8],
}


