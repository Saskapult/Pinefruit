use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use crossbeam_channel::unbounded;
use glam::*;
use krender::MaterialKey;
use krender::prelude::Mesh;
use std::collections::HashMap;
use crate::util::KGeneration;
use crate::voxel::*;
use thiserror::Error;




#[derive(Error, Debug)]
pub enum MapError {
	#[error("this chunk was not available")]
	ChunkUnavailable,
	#[error("this chunk's positive neighbours are not available")]
	ChunkNeighboursUnavailable,
}


#[derive(Debug)]
pub struct MapChunk {
	pub chunk: Chunk,
	pub generation: KGeneration,
}
impl MapChunk {
	pub fn decompose(self) -> (Chunk, KGeneration) {
		(self.chunk, self.generation)
	}
	pub fn parts(&self) -> (&Chunk, &KGeneration) {
		(&self.chunk, &self.generation)
	}
}
// An entry in the mesh storage for a map component
#[derive(Debug)]
enum MapChunkEntry {
	UnLoaded,	// Used if chunk does not exist yet
	Loading,	// Waiting for disk data done
	Generating,	// Waiting for generation done
	Complete(MapChunk),
}
impl MapChunkEntry {
	pub fn chunk(self) -> Option<MapChunk> {
		match self {
			MapChunkEntry::Complete(c) => Some(c),
			_ => None,
		}
	}
	pub fn chunk_ref(&self) -> Option<&MapChunk> {
		match self {
			MapChunkEntry::Complete(c) => Some(c),
			_ => None,
		}
	}
	pub fn chunk_mut(&mut self) -> Option<&mut MapChunk> {
		match self {
			MapChunkEntry::Complete(c) => Some(c),
			_ => None,
		}
	}
}



/// A map is a collection of chunks which are paged and also generated.
#[derive(Debug)]
pub struct Map {
	// Both chunk and generation stage are held here because having them separate could cause two allocations in place of one
	pub chunks: HashMap<[i32; 3], MapChunkEntry>,
	
	// Can receiver from network, generation, loading
	
	

	// Records the height of the map's highest block for every xz in a chunk
	// Could be used for lighting or feature placement
	// max_heightmap: HashMap<[i32; 3], Vec<i32>>, 
	pub chunk_size: [u32; 3],
	pub seed: u32,
	pending_blockmods: HashMap<[i32; 3], Vec<BlockMod>>,
	tgen: TerrainGenerator,
}
impl Map {
	pub fn new(chunk_size: [u32; 3], seed: u32) -> Self {
		let (generated_chunk_sender, generated_chunk_receiver) = unbounded();
		Self { 
			chunks: HashMap::new(), 

			generated_chunk_sender,
			generated_chunk_receiver,
			max_generation_jobs: 32,
			cur_generation_jobs: 0,

			chunk_size, 
			seed,
			pending_blockmods: HashMap::new(), 
			tgen: TerrainGenerator::new(seed),
		}
	}

	pub fn apply_chunkblockmods(&mut self, block_mods: ChunkBlockMods) {
		let chunk_size = self.chunk_size;
		for (cpos, bms) in block_mods {
			if let Ok(c) = self.chunk_mut(cpos) {
				for bm in bms {
					match bm.reason {
						BlockModReason::WorldGenSet(v) => {
							let (_, voxel_chunk_position) = bm.position.chunk_voxel_position(chunk_size);
							c.set_voxel(voxel_chunk_position, v)
						},
						_ => todo!("Handle block mods other than world generation"),
					}
				}
			} else {
				// Add bms to pending bms
				if let Some(v) = self.pending_blockmods.get_mut(&cpos) {
					v.extend_from_slice(&bms[..]);
				} else {
					self.pending_blockmods.insert(cpos, bms);
				}
			}
		}
	}

	pub fn mark_chunk_existence(&mut self, chunk_position: [i32; 3]) -> bool {
		if self.chunks.contains_key(&chunk_position) {
			true
		} else {
			self.chunks.insert(chunk_position, MapChunkEntry::UnLoaded);
			false
		}
	}

	/// begins generation for chunks that have not been generated
	pub fn begin_chunks_generation(
		&mut self,
		blocks: &BlockManager,
	) -> Result<Vec<[i32; 3]>, GenerationError> {
		let mut queued_chunks = Vec::new();
		for (&chunk_position, mce) in self.chunks.iter() {
			if !(self.cur_generation_jobs < self.max_generation_jobs) {
				break
			}
			match mce {
				MapChunkEntry::UnLoaded => {
					// info!("Generating chunk {chunk_position:?}");
					queued_chunks.push(chunk_position);
					self.cur_generation_jobs += 1;
					let f = self.chunk_generation_function(chunk_position, blocks)?;
					let sender = self.generated_chunk_sender.clone();
					rayon::spawn(move || {
						let (c, cbms) = f();
						sender.send((chunk_position, c, cbms)).unwrap();
					});
				},
				_ => {},
			}
		}
		for chunk in queued_chunks.iter() {
			let c = self.chunks.get_mut(chunk).unwrap();
			*c = MapChunkEntry::Generating;
		}
		Ok(queued_chunks)
	}
	pub fn receive_generated_chunks(&mut self) {
		for (chunk_position, chunk, block_mods) in self.generated_chunk_receiver.try_iter().collect::<Vec<_>>() {
			// info!("Received generated chunk {chunk_position:?}");
			self.cur_generation_jobs -= 1;
			self.insert_chunk(chunk_position, chunk, KGeneration::new());
			self.apply_chunkblockmods(block_mods);
		}
	}

	// Mesh a chunk with respect to those around it
	// This will look bad if seen from an side without a chunk before it
	pub fn chunk_meshing_function(
		&self, 
		position: [i32; 3],
		blocks: &BlockManager,
	) -> Result<(impl Fn() -> (Vec<(MaterialKey, Mesh)>,), [KGeneration; 4]), MapError> {
		let [px, py, pz] = position;
		
		let (main_chunk, &main_gen) = self.chunk_map(position)
			.ok().ok_or(MapError::ChunkUnavailable)?.parts();
		
		// Copy slices of neighbours
		debug!("Extracting xp slice");
		let (xp_chunk, &xp_gen) = self.chunk_map([px+1, py, pz])
			.ok().ok_or(MapError::ChunkNeighboursUnavailable)?.parts();
		let xp_slice = (0..self.chunk_size[1]).map(|y| {
			(0..self.chunk_size[2]).map(move |z| {
				xp_chunk.get_voxel([0, y as i32, z as i32])
			})
		}).flatten().collect::<Vec<_>>();

		debug!("Extracting yp slice");
		let (yp_chunk, &yp_gen) = self.chunk_map([px, py+1, pz])
			.ok().ok_or(MapError::ChunkNeighboursUnavailable)?.parts();
		let yp_slice = (0..self.chunk_size[0]).map(|x| {
			(0..self.chunk_size[2]).map(move |z| {
				yp_chunk.get_voxel([x as i32, 0, z as i32])
			})
		}).flatten().collect::<Vec<_>>();
		
		debug!("Extracting zp slice");
		let (zp_chunk, &zp_gen) = self.chunk_map([px, py, pz+1])
			.ok().ok_or(MapError::ChunkNeighboursUnavailable)?.parts();
		let zp_slice = (0..self.chunk_size[0]).map(|x| {
			(0..self.chunk_size[1]).map(move |y| {
				zp_chunk.get_voxel([x as i32, y as i32, 0])
			})
		}).flatten().collect::<Vec<_>>();

		let neighbour_slices = [xp_slice, yp_slice, zp_slice];
		let chunk_size = self.chunk_size.map(|x| x as usize);
		let blockmap = blocks.clone(); // <- BAD!
		let chunk_contents = main_chunk.contents.clone(); // Also BAD
		// Maybe extract entries for all blocks in chunks?
		let result = move || {
			map_mesh(
				&chunk_contents, 
				chunk_size,
				position,
				&neighbour_slices,
				&blockmap,
				true,
			)
		};

		Ok((result, [main_gen, xp_gen, yp_gen, zp_gen]))
	}

	pub fn insert_chunk(&mut self, chunk_position: [i32; 3], chunk: Chunk, generation: KGeneration) -> Option<(Chunk, KGeneration)> {
		let mc = MapChunk { chunk, generation, };

		self.chunks.insert(chunk_position, MapChunkEntry::Complete(mc))
			.and_then(|e| e.chunk())
			.and_then(|mc| Some(mc.decompose()))
	}

	pub fn generation(&self, chunk_position: [i32; 3]) -> Result<KGeneration, MapError> {
		self.chunks.get(&chunk_position)
			.and_then(|mce| mce.chunk_ref())
			.and_then(|mc| Some(mc.generation))
			.ok_or(MapError::ChunkUnavailable)
	}
	pub fn chunk(&self, chunk_position: [i32; 3]) -> Result<&Chunk, MapError> {
		self.chunks.get(&chunk_position)
			.and_then(|mce| mce.chunk_ref())
			.and_then(|mc| Some(&mc.chunk))
			.ok_or(MapError::ChunkUnavailable)
	}
	pub fn chunk_mut(&mut self, chunk_position: [i32; 3]) -> Result<&mut Chunk, MapError> {
		self.chunks.get_mut(&chunk_position)
			.and_then(|mce| mce.chunk_mut())
			.and_then(|mc| Some(&mut mc.chunk))
			.ok_or(MapError::ChunkUnavailable)
	}
	pub fn chunk_map(&self, chunk_position: [i32; 3]) -> Result<&MapChunk, MapError> {
		self.chunks.get(&chunk_position)
			.and_then(|mce| mce.chunk_ref())
			.ok_or(MapError::ChunkUnavailable)
	}
	pub fn chunk_map_mut(&mut self, chunk_position: [i32; 3]) -> Result<&mut MapChunk, MapError> {
		self.chunks.get_mut(&chunk_position)
			.and_then(|mce| mce.chunk_mut())
			.ok_or(MapError::ChunkUnavailable)
	}

	// All voxels are set through me
	// If a voxel is set on an edge the affected chunk(s) should be marked for remeshing
	// This, however, should be dealt with by the calling party and not within this function
	pub fn set_voxel_world(&mut self, world_coords: [i32; 3], voxel: Voxel) {
		let (cpos, cvpos) = self.world_chunk_voxel(world_coords);
		debug!("world {:?} -> chunk {:?} voxel {:?} to {:?}", &world_coords, &cpos, &cvpos, &voxel);
		if let Ok(mc) = self.chunk_map_mut(cpos) {
			mc.chunk.set_voxel(cvpos, voxel);
			mc.generation.increment();
		} else {
			todo!("Add modification to nonexistent chunk to pending block modifications")
		}
	}

	pub fn get_voxel_world(&self, world_coords: [i32; 3]) -> Result<Voxel, MapError> {
		let (cpos, cvpos) = self.world_chunk_voxel(world_coords);
		// debug!("world {:?} -> chunk {:?} voxel {:?}", &world_coords, &cpos, &cvpos);
		let chunk = self.chunk(cpos)?;
		Ok(chunk.get_voxel(cvpos))
	}

	// Wrappers! (I think they make things easier)
	pub fn world_chunk(&self, world_coords: [i32; 3]) -> [i32; 3] {
		world_chunk(world_coords, self.chunk_size)
	}
	pub fn world_chunk_voxel(&self, world_coords: [i32; 3]) -> ([i32; 3], [i32; 3]) {
		world_chunk_voxel(world_coords, self.chunk_size)
	}
	pub fn chunk_voxel_world(&self, chunk_position: [i32; 3], voxel_position: [i32; 3]) -> [i32; 3] {
		chunk_voxel_world(chunk_position, voxel_position, self.chunk_size)
	}
	pub fn chunk_point(&self, chunk: [i32; 3]) -> Vec3 {
		chunk_point(chunk, self.chunk_size)
	}
	pub fn point_chunk(&self, point: &Vec3) -> [i32; 3] {
		point_chunk(point, self.chunk_size)
	}
	pub fn point_chunk_voxel(&self, point: &Vec3) -> ([i32; 3], [i32; 3]) {
		point_chunk_voxel(point, self.chunk_size)
	}
	pub fn point_world_voxel(&self, point: &Vec3) -> [i32; 3] {
		point_world_voxel(point)
	}
}


// Never generate faces for negativemost blocks, they are covered by their chunks
// If not collect_transparent then don't group faces with a transparent material together, allowing them to be drawn individually (could we use instancing for this?)
// TODO: Group by direction
fn map_mesh(
	chunk_contents: &Vec<Voxel>,
	chunk_size: [usize; 3],
	chunk_position: [i32; 3], // Used only for mesh name
	neighbour_slices: &[Vec<Voxel>; 3], // xp, yp, zp
	blockmap: &BlockManager,
	_collect_transparent: bool,
) -> (
	Vec<(MaterialKey, Mesh)>, 	// Vec<(material idx, mesh data)>
) {
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
		let vertex_position_offset = Vec3::new(px as f32, py as f32, pz as f32);
		quad_positions.iter().for_each(|&p| {
			let vertex_position = vertex_position_offset + p;
			segment.positions.push(vertex_position.into());
		});
	}

	let mut mesh_parts = HashMap::new();

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
							if let Some(material_id) = material_id {
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
							if let Some(material_id) = material_id {
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
	}

	// let face_collections = mesh_parts.drain().map(|(i, segment)| {
	// 	let mesh = Mesh::new(&format!("mesh of chunk {:?} material {:?}", chunk_position, i))
	// 		.with_data("position", segment.positions)
	// 		.with_data("uv", segment.uvs)
	// 		.with_data("normal", segment.normals)
	// 		.with_data("index", segment.indices);
	// 	(i, mesh)
	// }).collect::<Vec<_>>();

	// (face_collections,)
	todo!()
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

const XP_QUAD_VERTICES: [Vec3; 4] = [
	Vec3::new(1.0, 0.0, 0.0),
	Vec3::new(1.0, 0.0, 1.0),
	Vec3::new(1.0, 1.0, 1.0),
	Vec3::new(1.0, 1.0, 0.0),
];
const YP_QUAD_VERTICES: [Vec3; 4] = [
	Vec3::new(1.0, 1.0, 0.0),
	Vec3::new(0.0, 1.0, 0.0),
	Vec3::new(0.0, 1.0, 1.0),
	Vec3::new(1.0, 1.0, 1.0),
];
const ZP_QUAD_VERTICES: [Vec3; 4] = [
	Vec3::new(1.0, 0.0, 1.0),
	Vec3::new(0.0, 0.0, 1.0),
	Vec3::new(0.0, 1.0, 1.0),
	Vec3::new(1.0, 1.0, 1.0),
];
const XN_QUAD_VERTICES: [Vec3; 4] = [
	Vec3::new(0.0, 0.0, 0.0),
	Vec3::new(0.0, 0.0, 1.0),
	Vec3::new(0.0, 1.0, 1.0),
	Vec3::new(0.0, 1.0, 0.0),
];
const YN_QUAD_VERTICES: [Vec3; 4] = [
	Vec3::new(1.0, 0.0, 0.0),
	Vec3::new(0.0, 0.0, 0.0),
	Vec3::new(0.0, 0.0, 1.0),
	Vec3::new(1.0, 0.0, 1.0),
];
const ZN_QUAD_VERTICES: [Vec3; 4] = [
	Vec3::new(1.0, 0.0, 0.0),
	Vec3::new(0.0, 0.0, 0.0),
	Vec3::new(0.0, 1.0, 0.0),
	Vec3::new(1.0, 1.0, 0.0),
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



#[cfg(test)]
mod tests {
	use std::time::Instant;
	use super::*;

	extern crate test;
	use test::Bencher;

	#[bench]
	fn bench_chunk_generation(_b: &mut Bencher) {
		const CHUNK_SIZE: [u32; 3] = [16; 3];
		
		let m = Map::new(CHUNK_SIZE, 0);
		let mut blocks = BlockManager::new();
		crate::voxel::blocks::tests::load_blocks_file_messy("./resources/kblocks.ron", &mut blocks);

		let dims = [5; 3];
		let to_generate = (0..(dims[0]*dims[1]*dims[2])).map(|v| {
			let x = v / (dims[1] * dims[2]) - dims[0] / 2;
			let y = (v % (dims[0] * dims[1])) / dims[1] - dims[1] / 2;
			let z = (v % (dims[0] * dims[1])) % dims[2] - dims[2] / 2;
			[x, y, z]
		}).collect::<Vec<_>>();

		let mut ds = Vec::new();
		for &chunk_position in to_generate.iter() {
			let st = Instant::now();
			let f = m.chunk_generation_function(chunk_position, &blocks).unwrap();
			let _g = f();
			ds.push(st.elapsed());
		}

		println!("Mean: {:.2}ms per chunk", ds.iter().cloned().reduce(|a, v| a + v).unwrap().as_secs_f32() / to_generate.len() as f32 * 1000.0)
	}
}
