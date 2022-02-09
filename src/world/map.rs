use nalgebra::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::world::*;
use crate::render::*;
use noise::{NoiseFn, Perlin};




#[derive(Debug)]
pub struct Map {
	chunks: HashMap<[i32; 3], Chunk>,
	pub chunk_dimensions: [u32; 3],
	blocks: Arc<RwLock<BlockManager>>,
}
impl Map {
	pub fn new(chunk_dimensions: [u32; 3], blockmanager: &Arc<RwLock<BlockManager>>) -> Self {
		Self { 
			chunks: HashMap::new(), 
			chunk_dimensions, 
			blocks: blockmanager.clone(),
		}
	}

	pub fn generate(&mut self) {
		let perlin = Perlin::new();


		for cx in -4..4 {
			for cy in 0..2 {
				for cz in -4..4 {
					let mut chunk = Chunk::new(self.chunk_dimensions);
					for x in 0..self.chunk_dimensions[0] {
						for z in 0..self.chunk_dimensions[2] {
							let wx = cx * self.chunk_dimensions[0] as i32 + x as i32;
							let wz = cz * self.chunk_dimensions[2] as i32 + z as i32;
							let val = perlin.get([wx as f64 + 0.5, wz as f64 + 0.5]);
							let ylevel = 2 + ((1.0 + val) * 2.0).floor() as i32;
							//println!("y level for xz: [{}, {}] is {} ({:.4})", wx, wz, ylevel, val);
							for y in 0..self.chunk_dimensions[1] {
								let wy = cy * self.chunk_dimensions[1] as i32 + y as i32;
								let voxel = {
									if wy >= ylevel {
										Voxel::Block(0)
									} else {
										Voxel::Empty
									}
								};
								chunk.set_voxel([x as i32, y as i32, z as i32], voxel)
							}
						}
					}
					self.chunks.insert([cx as i32, cy as i32, cz as i32], chunk);
				}
			}
		}
		
		// // Testing stuff
		// let mut filled_chunk = Chunk::new_of(chunk_dimensions, Voxel::Block(0));
		// filled_chunk.set_voxel([3, 3, 0], Voxel::Empty);
		// filled_chunk.set_voxel([3, 3, 2], Voxel::Empty);
		// chunks.insert([0, 0, 0], filled_chunk.clone());
		// chunks.insert([0, 0, 1], filled_chunk.clone());
		// chunks.insert([1, 0, 0], filled_chunk.clone());

		// let empty_chunk = Chunk::new(chunk_dimensions);
		// chunks.insert([0, 0, 2], empty_chunk);
	}

	pub fn is_chunk_loaded(&self, position: [i32; 3]) -> bool {
		self.chunks.contains_key(&position)
	}

	// Mesh a chunk with respect to those around it
	// This will look bad if seen from an side without a chunk before it
	pub fn mesh_chunk(&self, position: [i32; 3]) -> Vec<(usize, Mesh)> {
		let [px, py, pz] = position;
		
		let main_chunk = self.chunk(position).expect("Tried to mesh unloaded chunk!");
		
		// Copy slices of neighbours
		// If the neighbour is not loaded, pretend there is nothing there (this is bad)
		debug!("Extracting xp slice");
		let xp_slice = match self.chunk([px+1, py, pz]) {
			Some(chunk) => {
				let mut xp = vec![Voxel::Empty; (self.chunk_dimensions[1] * self.chunk_dimensions[2]) as usize];
				for y in 0..self.chunk_dimensions[1] {
					let y_offset = y * self.chunk_dimensions[1];
					for z in 0..self.chunk_dimensions[2] {
						xp[(y_offset + z) as usize] = chunk.get_voxel([0, y as i32, z as i32]);
					}
				}
				xp
			},
			None => vec![Voxel::Block(0); (self.chunk_dimensions[1] * self.chunk_dimensions[2]) as usize],
		};
		debug!("Extracting yp slice");
		let yp_slice = match self.chunk([px, py+1, pz]) {
			Some(chunk) => {
				let mut yp = vec![Voxel::Empty; (self.chunk_dimensions[0] * self.chunk_dimensions[2]) as usize];
				for x in 0..self.chunk_dimensions[0] {
					let x_offset = x * self.chunk_dimensions[0];
					for z in 0..self.chunk_dimensions[2] {
						yp[(x_offset + z) as usize] = chunk.get_voxel([x as i32, 0, z as i32]);
					}
				}
				yp
			},
			None => vec![Voxel::Block(0); (self.chunk_dimensions[0] * self.chunk_dimensions[2]) as usize],
		};
		debug!("Extracting zp slice");
		let zp_slice = match self.chunk([px, py, pz+1]) {
			Some(chunk) => {
				let mut zp = vec![Voxel::Empty; (self.chunk_dimensions[0] * self.chunk_dimensions[1]) as usize];
				for x in 0..self.chunk_dimensions[0] {
					let x_offset = x * self.chunk_dimensions[0];
					for y in 0..self.chunk_dimensions[1] {
						zp[(x_offset + y) as usize] = chunk.get_voxel([x as i32, y as i32, 0]);
					}
				}
				zp
			},
			None => vec![Voxel::Block(0); (self.chunk_dimensions[0] * self.chunk_dimensions[1]) as usize],
		};
		let slices = [xp_slice, yp_slice, zp_slice];

		let (mut segments, _) = map_mesh(
			&main_chunk.contents, 
			self.chunk_dimensions.map(|x| x as usize),
			&slices,
			&self.blocks.read().unwrap(),
			true,
		);

		segments.drain(..).map(|(material_idx, segment)| {
			let mesh = Mesh::new(&format!("mesh of chunk {:?} material {}", position, material_idx))
				.with_positions(segment.positions)
				.with_uvs(segment.uvs)
				.with_normals(segment.normals)
				.with_indices(segment.indices);
			(material_idx as usize, mesh)
		}).collect::<Vec<_>>()
	}

	pub fn chunks_sphere(&self, centre: [i32; 3], radius: i32) -> Vec<[i32; 3]> {
		let [cx, cy, cz] = centre;
		let mut res = Vec::new();
		// Consider bounding cube
		for x in (cx-radius)..(cx+radius+1) {
			for y in (cy-radius)..(cy+radius+1) {
				for z in (cz-radius)..(cz+radius+1) {
					// If in sphere
					if x^2 + y^2 + z^2 < radius {
						res.push([x,y,z]);
					}
				}
			}
		}
		res
	}

	// Returns the positions of all chunks that should be rendered from this camera
	// There was some article showing how this can be optimized quite well, but I don't remember its name 
	pub fn chunks_view_cone(&self, _camera: Camera, _distance: u32) -> Vec<[i32; 3]> {
		todo!()
	}

	pub fn chunk(&self, chunk_position: [i32; 3]) -> Option<&Chunk> {
		if self.chunks.contains_key(&chunk_position) {
			Some(&self.chunks[&chunk_position])
		} else {
			// Make future to load the chunk?
			None
		}
	}
	pub fn chunk_mut(&mut self, chunk_position: [i32; 3]) -> Option<&mut Chunk> {
		if self.chunks.contains_key(&chunk_position) {
			let s = self.chunks.get_mut(&chunk_position).unwrap();
			Some(s)
		} else {
			None
		}
	}

	// All voxels are set through me
	// If a voxel is set on an edge the affected chunk(s) should be marked for remeshing
	// This, however, should be dealt with by the calling party and not within this function
	pub fn set_voxel_world(&mut self, world_coords: [i32; 3], voxel: Voxel) {
		let (cpos, cvpos) = self.world_chunk_voxel(world_coords);
		if let Some(chunk) = self.chunk_mut(cpos) {
			chunk.set_voxel(cvpos, voxel);
		} else {
			panic!("Tried to set a voxel in an unloaded chunk!");
		}
	}

	// Gets the coordinates of the chunk in which the wold position resides 
	pub fn world_chunk(&self, world_coords: [i32; 3]) -> [i32; 3] {
		let chunk_pos = [
			world_coords[0] / (self.chunk_dimensions[0] as i32),
			world_coords[1] / (self.chunk_dimensions[1] as i32),
			world_coords[2] / (self.chunk_dimensions[2] as i32),
		];
		chunk_pos
	}

	// Gets the coordinates of the chunk and the coordinates of the voxel within the chunk in which the wold position resides 
	pub fn world_chunk_voxel(&self, world_coords: [i32; 3]) -> ([i32; 3], [i32; 3]) {
		let chunk_pos = [
			world_coords[0] / (self.chunk_dimensions[0] as i32),
			world_coords[1] / (self.chunk_dimensions[1] as i32),
			world_coords[2] / (self.chunk_dimensions[2] as i32),
		];
		let chunk_voxel_pos = [
			world_coords[0] % (self.chunk_dimensions[0] as i32),
			world_coords[1] % (self.chunk_dimensions[1] as i32),
			world_coords[2] % (self.chunk_dimensions[2] as i32),
		];
		(chunk_pos, chunk_voxel_pos)
	}

	// Gets the point the chunk should be rendered at relative to the world
	pub fn chunk_point(&self, chunk: [i32; 3]) -> Vector3<f32> {
		Vector3::new(
			(chunk[0] * self.chunk_dimensions[0] as i32) as f32,
			(chunk[1] * self.chunk_dimensions[1] as i32) as f32,
			(chunk[2] * self.chunk_dimensions[2] as i32) as f32,
		)
	}

	// Gets the coordinates of the chunk in which this point resides
	pub fn point_chunk(&self, point: Vector3<f32>) -> [i32; 3] {
		let chunk_pos = [
			(point[0] / (self.chunk_dimensions[0] as f32)).floor() as i32,
			(point[1] / (self.chunk_dimensions[1] as f32)).floor() as i32,
			(point[2] / (self.chunk_dimensions[2] as f32)).floor() as i32,
		];
		chunk_pos
	}

	// Gets the coordinates of the chunk and the coordinates of the voxel within the chunk in which the point resides 
	pub fn point_chunk_voxel(&self, point: Vector3<f32>) -> ([i32; 3], [i32; 3]) {
		let chunk_pos = [
			(point[0] / (self.chunk_dimensions[0] as f32)).floor() as i32,
			(point[1] / (self.chunk_dimensions[1] as f32)).floor() as i32,
			(point[2] / (self.chunk_dimensions[2] as f32)).floor() as i32,
		];
		let chunk_voxel_pos = [
			(point[0].floor() as u32 % self.chunk_dimensions[0]) as i32,
			(point[1].floor() as u32 % self.chunk_dimensions[1]) as i32,
			(point[2].floor() as u32 % self.chunk_dimensions[2]) as i32,
		];
		(chunk_pos, chunk_voxel_pos)
	}

	// Gets the coordinates of the voxel in the world in which the point resides
	pub fn point_world_voxel(&self, point: Vector3<f32>) -> [i32; 3] {
		let world_voxel_pos = [
			point[0].floor() as i32,
			point[1].floor() as i32,
			point[2].floor() as i32,
		];
		world_voxel_pos
	}
}



// Never generate faces for negativemost blocks, they are covered by their chunks
// If not collect_transparent then don't group faces with a transparent material together, allowing them to be drawn individually (could we use instancing for this?)
struct ChunkMeshSegment {
	pub positions: Vec<[f32; 3]>, 
	pub normals: Vec<[f32; 3]>, 
	pub uvs: Vec<[f32; 2]>,
	pub indices: Vec<u16>,
}
impl ChunkMeshSegment {
	pub fn new() -> Self {
		Self {
			positions: Vec::new(),
			normals: Vec::new(),
			uvs: Vec::new(),
			indices: Vec::new(),
		}
	}
}
fn map_mesh(
	chunk_contents: &Vec<Voxel>,
	chunk_size: [usize; 3],
	neighbour_slices: &[Vec<Voxel>; 3], // xp, yp, zp
	blockmap: &BlockManager,
	_collect_transparent: bool,
) -> (
	Vec<(u32, ChunkMeshSegment)>, 	// Vec<(material id, mesh data)>
	Vec<(usize, bool)>, 	// Vec<(model id, instance)> (bool is temporary, should use instance stuff)
) {

	fn append_face(segment: &mut ChunkMeshSegment, position: [usize; 3], direction: &Direction) {
		let [x, y, z] = position;

		// Indices
		let l = segment.positions.len() as u16;
		match direction {
			Direction::Xp | Direction::Yn | Direction::Zp => {
				REVERSE_QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));
			},
			Direction::Xn | Direction::Yp | Direction::Zn => {
				QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));
			},
		}
		//QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));

		// Normals
		let normal = match direction {
			Direction::Xp => [1.0, 0.0, 0.0],
			Direction::Yp => [0.0, 1.0, 0.0],
			Direction::Zp => [0.0, 0.0, 1.0],
			Direction::Xn => [-1.0, 0.0, 0.0],
			Direction::Yn => [0.0, -1.0, 0.0],
			Direction::Zn => [0.0, 0.0, -1.0],
		};
		(0..4).for_each(|_| segment.normals.push(normal));

		// UVs
		QUAD_UVS.iter().for_each(|uv| segment.uvs.push(*uv));

		// Positions
		let quad_positions = match direction {
			Direction::Xp => XP_QUAD_VERTICES,
			Direction::Yp => YP_QUAD_VERTICES,
			Direction::Zp => ZP_QUAD_VERTICES,
			Direction::Xn => XN_QUAD_VERTICES,
			Direction::Yn => YN_QUAD_VERTICES,
			Direction::Zn => ZN_QUAD_VERTICES,
		};
		let vertex_position_offset = Vector3::new(x as f32, y as f32, z as f32);
		quad_positions.iter().for_each(|p| {
			let vertex_position = vertex_position_offset + p;
			segment.positions.push(vertex_position.into());
		});
	}

	let mut mesh_parts = HashMap::new();
	let mut models = Vec::new();

	let [x_size, y_size, z_size] = chunk_size;
	let x_multiplier = y_size * z_size;
	let y_multiplier = z_size;
	let _z_multiplier = 1;

	const DIRECTIONS_VECTORS: &[(Direction, [i32; 3])] = &[
		(Direction::Xp, [1, 0, 0]), 
		(Direction::Yp, [0, 1, 0]), 
		(Direction::Zp, [0, 0, 1]), 
	];
	for (direction, direction_vector) in DIRECTIONS_VECTORS {
		debug!("Meshing {:?}", direction);
		let dvx = direction_vector[0] as usize;
		let dvy = direction_vector[1] as usize;
		let dvz = direction_vector[2] as usize;

		for x in 0..x_size {
			let x_offset = x * x_multiplier;
			// Could we create an "a" slice and a "b" slice?
			// When ending iteration "b" becomes "a" and we only need to read the new "b"
			for y in 0..y_size {
				let y_offset = y * y_multiplier;
				for z in 0..z_size {

					// Get 'a' and 'b' blocks to compare
					let a = chunk_contents[x_offset + y_offset + z];
					let bx = x + dvx;
					let by = y + dvy;
					let bz = z + dvz;
					let b = {
						// These *should* already be cache-optimized, so don't worry about that
						// Todo: re-order so we don't do 3 comparisons in regular case
						if bx == x_size {
							neighbour_slices[0][by*x_size + bz]
						} else if by == y_size {
							neighbour_slices[1][bx*y_size + bz]
						} else if bz == z_size {
							neighbour_slices[2][bx*z_size + by]
						} else {
							chunk_contents[bx*x_multiplier + by*y_multiplier + bz]
						}
					};

					// Are they transparent? (this is crude make it better)
					// Todo: Check if specific face is opaque
					let a_index = match a {
						Voxel::Empty => None, 
						Voxel::Block(idx) => Some(idx),
					};
					let a_transparent = !a_index.is_some();
					let b_index = match b {
						Voxel::Empty => None, 
						Voxel::Block(idx) => Some(idx),
					};
					let b_transparent = !b_index.is_some();

					// If at least one of them is transparent
					if a_transparent || b_transparent {
						// Todo: test if should generate transparent face

						// a opaque b transparent
						// Make positive face for a
						if !a_transparent && b_transparent {
							// Find existing mesh segment or create new one
							let a_material_id = blockmap.index(a_index.unwrap() as usize).material_idx;
							let a_mesh_part = {
								if mesh_parts.contains_key(&a_material_id) {
									mesh_parts.get_mut(&a_material_id).unwrap()
								} else {
									mesh_parts.insert(a_material_id, ChunkMeshSegment::new());
									mesh_parts.get_mut(&a_material_id).unwrap()
								}
							};
							append_face(a_mesh_part, [x,y,z], direction);
						}
						// a transparent b opaque
						// Make negative face for b
						if a_transparent && !b_transparent {
							let b_material_id = blockmap.index(b_index.unwrap() as usize).material_idx;
							let b_mesh_part = {
								if mesh_parts.contains_key(&b_material_id) {
									mesh_parts.get_mut(&b_material_id).unwrap()
								} else {
									mesh_parts.insert(b_material_id, ChunkMeshSegment::new());
									mesh_parts.get_mut(&b_material_id).unwrap()
								}
							};
							
							append_face(b_mesh_part, [bx,by,bz], &direction.flip());
						}
					}
				}
			}
		}
	}

	let face_collections = mesh_parts.drain().collect::<Vec<_>>();

	(face_collections, models)
}



#[derive(Debug, Copy, Clone)]
enum Direction {
	Xp,
	Xn,
	Yp,
	Yn,
	Zp,
	Zn,
}
impl Direction {
	pub fn flip(&self) -> Self {
		match self {
			Direction::Xp => Direction::Xn,
			Direction::Yp => Direction::Yn,
			Direction::Zp => Direction::Zn,
			Direction::Xn => Direction::Xp,
			Direction::Yn => Direction::Yp,
			Direction::Zn => Direction::Zp,
		}
	}
}

pub const XP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new( 0.5,  0.5, -0.5),
	Vector3::new( 0.5, -0.5, -0.5),
	Vector3::new( 0.5, -0.5,  0.5),
	Vector3::new( 0.5,  0.5,  0.5),
];
pub const YP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new( 0.5,  0.5, -0.5),
	Vector3::new(-0.5,  0.5, -0.5),
	Vector3::new(-0.5,  0.5,  0.5),
	Vector3::new( 0.5,  0.5,  0.5),
];
pub const ZP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new( 0.5, -0.5,  0.5),
	Vector3::new(-0.5, -0.5,  0.5),
	Vector3::new(-0.5,  0.5,  0.5),
	Vector3::new( 0.5,  0.5,  0.5),
];
pub const XN_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(-0.5,  0.5, -0.5),
	Vector3::new(-0.5, -0.5, -0.5),
	Vector3::new(-0.5, -0.5,  0.5),
	Vector3::new(-0.5,  0.5,  0.5),
];
pub const YN_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new( 0.5, -0.5, -0.5),
	Vector3::new(-0.5, -0.5, -0.5),
	Vector3::new(-0.5, -0.5,  0.5),
	Vector3::new( 0.5, -0.5,  0.5),
];
pub const ZN_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new( 0.5, -0.5, -0.5),
	Vector3::new(-0.5, -0.5, -0.5),
	Vector3::new(-0.5,  0.5, -0.5),
	Vector3::new( 0.5,  0.5, -0.5),
];

pub const QUAD_UVS: [[f32; 2]; 4] = [
	[1.0, 1.0],
	[1.0, 0.0],
	[0.0, 0.0],
	[0.0, 1.0],
];
pub const QUAD_INDICES: [u16; 6] = [
	0, 1, 2,
	2, 3, 0, 
];
pub const REVERSE_QUAD_INDICES: [u16; 6] = [
	2, 1, 0,
	0, 3, 2, 
];



#[cfg(test)]
mod tests {
	use super::*;
	use noise::{Perlin, NoiseFn};

	#[test]
	fn test_namething() {
		let perlin = Perlin::new();
		let vals = [
			[0.1, 2.31],
			[1.0, 2.3],
			[3.0, 2.0],
			[300.0, 20.0],
		];
		for v in vals {
			let p = perlin.get(v);
			println!("P({:?}) = {}", v, p);
		}
		assert!(true);
	}
}
