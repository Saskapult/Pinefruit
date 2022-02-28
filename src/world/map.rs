use nalgebra::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::world::*;
use crate::render::*;
use crate::mesh::*;
use thiserror::Error;




#[derive(Error, Debug)]
pub enum MapError {
	#[error("hey this chunk isn't loaded")]
	ChunkUnloaded,
}



#[derive(Debug)]
pub struct Map {
	chunks: HashMap<[i32; 3], Chunk>,
	pub chunk_dimensions: [u32; 3],
	blocks: Arc<RwLock<BlockManager>>,
	blockmods: BlockMods,
}
impl Map {
	pub fn new(chunk_dimensions: [u32; 3], blockmanager: &Arc<RwLock<BlockManager>>) -> Self {
		Self { 
			chunks: HashMap::new(), 
			chunk_dimensions, 
			blocks: blockmanager.clone(),
			blockmods: HashMap::new(), 
		}
	}

	pub fn apply_chunkblockmods(&mut self, block_mods: BlockMods) {
		for (cpos, bms) in block_mods {
			if let Some(c) = self.chunk_mut(cpos) {
				for bm in bms {
					match bm.reason {
						BlockModReason::WorldGenSet(v) => c.set_voxel(bm.voxel_chunk_position, v),
						_ => todo!(),
					}
				}
			}
		}
	}

	pub fn generate(&mut self) {
		let bm = self.blocks.read().unwrap();
		
		let tgen = TerrainGenerator::new(0);
		let carver = WorleyCarver::new(0);

		for cx in -4..4 {
			for cy in -1..2 {
				for cz in -4..4 {
					let chunk = Chunk::new(self.chunk_dimensions)
						.base([cx, cy, cz], &tgen, &bm);
					//	.carve([cx, cy, cz], &carver);
					
					self.chunks.insert([cx as i32, cy as i32, cz as i32], chunk);
				}
			}
		}

		let mut grassify_mods = BlockMods::new();
		for cx in -4..4 {
			for cy in -1..2 {
				for cz in -4..4 {
					merge_blockmods(&mut grassify_mods, tgen.grassify_3d([cx, cy, cz], &self, &bm));
				}
			}
		}
		drop(bm);
		self.apply_chunkblockmods(grassify_mods);

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
			None => vec![Voxel::Empty; (self.chunk_dimensions[1] * self.chunk_dimensions[2]) as usize],
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
			None => vec![Voxel::Empty; (self.chunk_dimensions[0] * self.chunk_dimensions[2]) as usize],
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
			None => vec![Voxel::Empty; (self.chunk_dimensions[0] * self.chunk_dimensions[1]) as usize],
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
			(material_idx, mesh)
		}).collect::<Vec<_>>()
	}

	pub fn chunks_sphere(&self, centre: [i32; 3], radius: i32) -> Vec<[i32; 3]> {
		let [cx, cy, cz] = centre;
		let mut res = Vec::new();
		// Consider bounding cube
		for x in (cx-radius)..=(cx+radius) {
			for y in (cy-radius)..=(cy+radius) {
				for z in (cz-radius)..=(cz+radius) {
					if Map::within_chunks_sphere([x, y, z], centre, radius) {
						res.push([x,y,z]);
					}
				}
			}
		}
		res
	}

	#[inline]
	pub fn within_chunks_sphere(cpos: [i32; 3], centre: [i32; 3], radius: i32) -> bool {
		let x = cpos[0] - centre[0];
		let y = cpos[1] - centre[1];
		let z = cpos[2] - centre[2];
		(x.pow(2) + y.pow(2) + z.pow(2)) < radius.pow(2)
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
		debug!("world {:?} -> chunk {:?} voxel {:?} to {:?}", &world_coords, &cpos, &cvpos, &voxel);
		if let Some(chunk) = self.chunk_mut(cpos) {
			chunk.set_voxel(cvpos, voxel);
		} else {
			warn!("Tried to set a voxel in an unloaded chunk");
		}
	}

	pub fn get_voxel_world(&mut self, world_coords: [i32; 3]) -> Option<Voxel> {
		let (cpos, cvpos) = self.world_chunk_voxel(world_coords);
		// debug!("world {:?} -> chunk {:?} voxel {:?}", &world_coords, &cpos, &cvpos);
		if let Some(chunk) = self.chunk(cpos) {
			Some(chunk.get_voxel(cvpos))
		} else {
			None
		}
	}

	// Gets the coordinates of the chunk in which the wold position resides 
	pub fn world_chunk(&self, world_coords: [i32; 3]) -> [i32; 3] {
		let chunk_pos = [
			world_coords[0] / (self.chunk_dimensions[0] as i32) - if world_coords[0] < 0 { 1 } else { 0 },
			world_coords[1] / (self.chunk_dimensions[1] as i32) - if world_coords[1] < 0 { 1 } else { 0 },
			world_coords[2] / (self.chunk_dimensions[2] as i32) - if world_coords[2] < 0 { 1 } else { 0 },
		];
		chunk_pos
	}

	// Gets the coordinates of the chunk and the coordinates of the voxel within the chunk in which the wold position resides 
	pub fn world_chunk_voxel(&self, world_coords: [i32; 3]) -> ([i32; 3], [i32; 3]) {
		let chunk_pos = self.world_chunk(world_coords);
		let mut chunk_voxel_pos = [
			world_coords[0] % (self.chunk_dimensions[0] as i32),
			world_coords[1] % (self.chunk_dimensions[1] as i32),
			world_coords[2] % (self.chunk_dimensions[2] as i32),
		];
		chunk_voxel_pos.iter_mut().zip(self.chunk_dimensions.iter()).for_each(|(v, cs)| {
			if *v < 0 {
				*v = *cs as i32 + *v;
			}
		});
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
	pub fn point_chunk(&self, point: &Vector3<f32>) -> [i32; 3] {
		let chunk_pos = [
			point[0].floor() as i32 / self.chunk_dimensions[0] as i32,
			point[1].floor() as i32 / self.chunk_dimensions[1] as i32,
			point[2].floor() as i32 / self.chunk_dimensions[2] as i32,
		];
		chunk_pos
	}

	// Gets the coordinates of the chunk and the coordinates of the voxel within the chunk in which the point resides 
	pub fn point_chunk_voxel(&self, point: &Vector3<f32>) -> ([i32; 3], [i32; 3]) {
		let chunk_pos = self.point_chunk(point);
		let chunk_voxel_pos = [
			point[0].floor() as i32 % self.chunk_dimensions[0] as i32,
			point[1].floor() as i32 % self.chunk_dimensions[1] as i32,
			point[2].floor() as i32 % self.chunk_dimensions[2] as i32,
		];
		(chunk_pos, chunk_voxel_pos)
	}

	// Gets the coordinates of the voxel in the world in which the point resides
	pub fn point_world_voxel(&self, point: &Vector3<f32>) -> [i32; 3] {
		let world_voxel_pos = [
			point[0].floor() as i32,
			point[1].floor() as i32,
			point[2].floor() as i32,
		];
		world_voxel_pos
	}

	// Todo: return the exact hit position
	// Todo: return the normal as well
	// Todo: turn it into an iterator
	pub fn voxel_ray(
		&self, 
		origin: &Vector3<f32>, 
		direction: &Vector3<f32>,
		_t_min: f32,
		t_max: f32,
	) -> Vec<[i32; 3]> {

		// https://stackoverflow.com/questions/12367071/how-do-i-initialize-the-t-variables-in-a-fast-voxel-traversal-algorithm-for-ray
		// 1.3 -> 0.3
		// -1.7 -> 0.3
		fn frac(f: f32) -> f32 {
			if f > 0.0 { 
				f.fract()
			} else {
				f - f.floor()
			}
		}

		/// Find the smallest positive t such that s+t*ds is an integer.
		fn intbound(s: f32, ds: f32) -> f32 {
			if ds < 0.0 {
				intbound(-s, -ds)
			} else {
				(1.0 - (s % 1.0)) / ds
			}
		}

		// Origin voxel (should be int)
		let mut vx = origin[0].floor() as i32;
		let mut vy = origin[1].floor() as i32; 
		let mut vz = origin[2].floor() as i32;

		// Direction of cast
		let direction = direction.normalize();
		let dx = direction[0]; 
		let dy = direction[1]; 
		let dz = direction[2];
		
		// Direction to increment when stepping (should be int)
		let v_step_x = dx.signum() as i32;
		let v_step_y = dy.signum() as i32;
		let v_step_z = dz.signum() as i32;

		// The change in t when taking a step (always positive)
		// How far in terms of t we can travel before reaching another voxel in (direction)
		// Todo: account for zeros
		let t_delta_x = 1.0 / dx.abs();
		let t_delta_y = 1.0 / dy.abs();
		let t_delta_z = 1.0 / dz.abs();

		// Distance along line to next voxel border of (direction)
		// let mut t_max_x = intbound(origin[0], dx);
		// let mut t_max_y = intbound(origin[1], dy);
		// let mut t_max_z = intbound(origin[2], dz);
		let mut t_max_x = t_delta_x * frac(origin[0]);
		let mut t_max_y = t_delta_y * frac(origin[1]);
		let mut t_max_z = t_delta_z * frac(origin[2]);

		if t_delta_x == 0.0 && t_delta_y == 0.0 && t_delta_z == 0.0 {
			panic!()
		}

		let mut t = 0.0;
		let mut results = Vec::new();
		while t < t_max {
			results.push([vx, vy, vz]);

			if t_max_x < t_max_y {
				// Closer to x boundary than y
				if t_max_x < t_max_z {
					// Closer to x boundary than z, closest to x boundary
					vx += v_step_x;
					t_max_x += t_delta_x;
					t += t_delta_x;
				} else {
					// Closer to z than x, closest to z
					vz += v_step_z;
					t_max_z += t_delta_z;
					t += t_delta_z;
				}
			} else {
				// Closer to y boundary than x
				if t_max_y < t_max_z {
					// Closer to y boundary than z, closest to y boundary
					vy += v_step_y;
					t_max_y += t_delta_y;
					t += t_delta_y;
				} else {
					// Closer to z than y, closest to z
					vz += v_step_z;
					t_max_z += t_delta_z;
					t += t_delta_z;
				}
			}
		}

		results
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
	Vec<(usize, ChunkMeshSegment)>, 	// Vec<(material idx, mesh data)>
	Vec<ModelInstance>,
) {
	#[inline]
	fn append_face(segment: &mut ChunkMeshSegment, position: [usize; 3], direction: &Direction) {
		let [px, py, pz] = position;

		// Indices
		let l = segment.positions.len() as u16;
		match direction {
			Direction::Xn | Direction::Yp | Direction::Zn => {
				REVERSE_QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));
				// QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));
			},
			Direction::Xp | Direction::Yn | Direction::Zp => {
				QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));
				// REVERSE_QUAD_INDICES.iter().for_each(|index| segment.indices.push(l + *index));
			},
		}

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
		let vertex_position_offset = Vector3::new(px as f32, py as f32, pz as f32);
		quad_positions.iter().for_each(|p| {
			let vertex_position = vertex_position_offset + p;
			segment.positions.push(vertex_position.into());
		});
	}

	let mut mesh_parts = HashMap::new();
	let models = Vec::new();

	let [x_size, y_size, z_size] = chunk_size;
	let x_multiplier = y_size * z_size;
	let y_multiplier = z_size;
	let z_multiplier = 1;

	const DIRECTIONS_VECTORS: &[(Direction, [i32; 3])] = &[
		(Direction::Xp, [1, 0, 0]), 
		(Direction::Yp, [0, 1, 0]), 
		(Direction::Zp, [0, 0, 1]), 
	];
	for (direction, direction_vector) in DIRECTIONS_VECTORS {
		trace!("Meshing {:?}", direction);
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
					let z_offset = z * z_multiplier;

					// Get 'a' and 'b' blocks to compare
					let a = chunk_contents[x_offset + y_offset + z_offset];
					let bx = x + dvx;
					let by = y + dvy;
					let bz = z + dvz;
					let b = {
						// These *should* already be cache-optimized, so don't worry about that
						if bx == x_size {
							neighbour_slices[0][by*x_size + bz]
						} else if by == y_size {
							neighbour_slices[1][bx*y_size + bz]
						} else if bz == z_size {
							neighbour_slices[2][bx*z_size + by]
						} else {
							chunk_contents[bx*x_multiplier + by*y_multiplier + bz*z_multiplier]
						}
					};

					// Are they transparent?
					// Currently this just checks if they are empty
					// Todo: Record if either empty and if either transparent
					// Todo: test if should generate transparent face
					// Todo: Make specific to block face
					let a_index = match a {
						Voxel::Empty => None, 
						Voxel::Block(idx) => Some(idx),
					};
					let a_empty = a_index.is_none();
					let b_index = match b {
						Voxel::Empty => None, 
						Voxel::Block(idx) => Some(idx),
					};
					let b_empty = b_index.is_none();

					if a_empty != b_empty {

						// Slice faces forward
						// a opaque b transparent -> make positive face for a at a
						if !a_empty && b_empty {
							// Find existing mesh segment or create new one
							let a_block = blockmap.index(a_index.unwrap() as usize);
							let material_id = match direction {
								Direction::Xp => a_block.xp_material_idx,
								Direction::Yp => a_block.yp_material_idx,
								Direction::Zp => a_block.zp_material_idx,
								Direction::Xn => a_block.xn_material_idx,
								Direction::Yn => a_block.yn_material_idx,
								Direction::Zn => a_block.zn_material_idx,
							};
							let mesh_part = {
								if mesh_parts.contains_key(&material_id) {
									mesh_parts.get_mut(&material_id).unwrap()
								} else {
									mesh_parts.insert(material_id, ChunkMeshSegment::new());
									mesh_parts.get_mut(&material_id).unwrap()
								}
							};
							append_face(mesh_part, [x,y,z], direction);
						}

						// Slice faces backward
						// a transparent b opaque -> make negative face for b at b
						if a_empty && !b_empty {
							let b_block = blockmap.index(b_index.unwrap() as usize);
							let material_id = match direction.flip() {
								Direction::Xp => b_block.xp_material_idx,
								Direction::Yp => b_block.yp_material_idx,
								Direction::Zp => b_block.zp_material_idx,
								Direction::Xn => b_block.xn_material_idx,
								Direction::Yn => b_block.yn_material_idx,
								Direction::Zn => b_block.zn_material_idx,
							};
							let mesh_part = {
								if mesh_parts.contains_key(&material_id) {
									mesh_parts.get_mut(&material_id).unwrap()
								} else {
									mesh_parts.insert(material_id, ChunkMeshSegment::new());
									mesh_parts.get_mut(&material_id).unwrap()
								}
							};
							append_face(mesh_part, [bx,by,bz], &direction.flip());
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

const XP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(1.0, 0.0, 0.0),
	Vector3::new(1.0, 0.0, 1.0),
	Vector3::new(1.0, 1.0, 1.0),
	Vector3::new(1.0, 1.0, 0.0),
];
const YP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(1.0, 1.0, 0.0),
	Vector3::new(0.0, 1.0, 0.0),
	Vector3::new(0.0, 1.0, 1.0),
	Vector3::new(1.0, 1.0, 1.0),
];
const ZP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(1.0, 0.0, 1.0),
	Vector3::new(0.0, 0.0, 1.0),
	Vector3::new(0.0, 1.0, 1.0),
	Vector3::new(1.0, 1.0, 1.0),
];
const XN_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(0.0, 0.0, 0.0),
	Vector3::new(0.0, 0.0, 1.0),
	Vector3::new(0.0, 1.0, 1.0),
	Vector3::new(0.0, 1.0, 0.0),
];
const YN_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(1.0, 0.0, 0.0),
	Vector3::new(0.0, 0.0, 0.0),
	Vector3::new(0.0, 0.0, 1.0),
	Vector3::new(1.0, 0.0, 1.0),
];
const ZN_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(1.0, 0.0, 0.0),
	Vector3::new(0.0, 0.0, 0.0),
	Vector3::new(0.0, 1.0, 0.0),
	Vector3::new(1.0, 1.0, 0.0),
];

const QUAD_UVS: [[f32; 2]; 4] = [
	[1.0, 1.0],
	[0.0, 1.0],
	[0.0, 0.0],
	[1.0, 0.0],
];
const QUAD_INDICES: [u16; 6] = [
	0, 1, 2,
	2, 3, 0, 
];
const REVERSE_QUAD_INDICES: [u16; 6] = [
	2, 1, 0,
	0, 3, 2, 
];
