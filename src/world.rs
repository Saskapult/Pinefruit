use nalgebra::*;
use std::collections::HashMap;



const CHUNKSIZE: usize = 32;
const CHUNKSIZE_SQUARED: usize = CHUNKSIZE * CHUNKSIZE;
const CHUNKSIZE_CUBED: usize = CHUNKSIZE * CHUNKSIZE * CHUNKSIZE;
const CHUNKSIZE_PLUS_ONE: usize = CHUNKSIZE + 1;
const CHUNKSIZE_PLUS_ONE_SQUARED: usize = CHUNKSIZE_PLUS_ONE * CHUNKSIZE_PLUS_ONE;



pub struct Map {
	pub chunks: Vec<Chunk>,
}
impl Map {
	pub fn new() -> Self {

		let chunks = Vec::new();

		Self {
			chunks,
		}
	}

	// Takes a world position, finds its chunk, and finds the block in that chunk
	pub fn block_at_pos(position: Vector3<f32>) {
		let chunk_pos = [
			(position[0] / (CHUNKSIZE as f32)).floor(),
			(position[1] / (CHUNKSIZE as f32)).floor(),
			(position[2] / (CHUNKSIZE as f32)).floor(),
		];
		let relative_block_pos = [
			(position[0] % (CHUNKSIZE as f32)),
			(position[1] % (CHUNKSIZE as f32)),
			(position[2] % (CHUNKSIZE as f32)),
		];
		// Check if chunk exists
	}
}


// Should be hashed in a z-order curve
pub struct Chunk {
	location: [i32; 3],			// Chunk location in chunk coordinates
	blocks: Vec<i32>,			// Len is CHUNKSIZE^3
	// Blocks to tick
	vertex_buf: wgpu::Buffer,
	index_buf: wgpu::Buffer,
	index_count: usize,
}
impl Chunk {
	
	fn meshme(&self) {
		let worldposition = [
			(self.location[0] * (CHUNKSIZE as i32)) as f32,
			(self.location[1] * (CHUNKSIZE as i32)) as f32,
			(self.location[2] * (CHUNKSIZE as i32)) as f32,
		];

		// let mesh_vertices = Vec::new();
		// let mesh_indices = Vec::new();
		// let mesh_texture_coordinates = Vec::new();

		for x in 0..CHUNKSIZE {
			for y in 0..CHUNKSIZE {
				for z in 0..CHUNKSIZE {
					
					let block_position = [
						worldposition[0] + (x as f32),
						worldposition[1] + (y as f32),
						worldposition[2] + (z as f32),
					];

					// Add block to mesh
				}
			}
		}

	}
}


struct Block {
	name: String,
	texture_id: u32,
}


struct BlockMap {
	blocks_id: Vec<Block>,				// Get block by id
	blocks_name: HashMap<String, u32>	// Get block id by name
}
impl BlockMap {
	// Blocks could be inserted more than once
	pub fn insert(&mut self, block: Block) -> u32 {
		let index = self.blocks_id.len() as u32;
		self.blocks_name.insert(block.name.clone(), index);
		self.blocks_id.push(block);
		index
	}

	pub fn get_block(&self, id: u32) -> &Block {
		&self.blocks_id[id as usize]
	}
	
	pub fn get_id(&self, name: &String) -> u32 {
		self.blocks_name[name]
	}
}


struct BlockRun {
	block_type: String,
	length: u32,
}


struct ChunkMesher {
	runs: Vec<BlockRun>,
	sides: [[bool; CHUNKSIZE_SQUARED]; 6],
}


