use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use anyhow::*;
use crate::material::*;
use crate::texture::*;
use crate::world::*;




#[derive(Debug, Serialize, Deserialize)]
pub struct BlockSpecification {
	pub name: String,
	pub faces: HashMap<String, PathBuf>,
	pub sounds: HashMap<String, PathBuf>,
}



pub fn load_blocks_file(
	path: impl AsRef<Path>,
	bm: &mut BlockManager,
	tm: &mut TextureManager, 
	mm: &mut MaterialManager, 
) -> Result<()> {
	let path  = path.as_ref();
	info!("Reading blocks file {:?}", path);

	let canonical_path = path.canonicalize()
		.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	let f = std::fs::File::open(path)
		.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
	let block_specs: Vec<BlockSpecification> = ron::de::from_reader(f)
		.with_context(|| format!("Failed to parse blocks ron file '{:?}'", &canonical_path))?;
	let folder_context = canonical_path.parent().unwrap();

	for spec in block_specs {
		let mut block = Block::new(&spec.name);
		for (face, material_path) in spec.faces {
			
			let full_thing = material_path.into_os_string().into_string().unwrap();
			let (material_spec_path, material_name) = {
				let splits = full_thing.split("::").collect::<Vec<&str>>();
				(splits[0], splits[1])
			};

			// Make sure that file is loaded
			let material_idx = match mm.index_name(&material_name.to_string()) {
				Some(idx) => idx,
				None => {
					warn!("Loading materials file from block loading");
					let canonical_material_path = folder_context.join(material_spec_path).canonicalize()?;
					crate::material::load_materials_file(canonical_material_path, tm, mm)?;
					mm.index_name(&material_name.to_string()).unwrap()
				},
			};

			match &*face {
				"every" => {
					block.xp_material_idx = material_idx;
					block.yp_material_idx = material_idx;
					block.zp_material_idx = material_idx;
					block.xn_material_idx = material_idx;
					block.yn_material_idx = material_idx;
					block.zn_material_idx = material_idx;
					break
				},
				"zp" | "front" 	=> block.zp_material_idx = material_idx,
				"zn" | "back" 	=> block.zn_material_idx = material_idx,
				"xn" | "left" 	=> block.xn_material_idx = material_idx,
				"xp" | "right" 	=> block.xp_material_idx = material_idx,
				"yp" | "up" 	=> block.yp_material_idx = material_idx,
				"yn" | "down" 	=> block.yn_material_idx = material_idx,
				_ => warn!("Weird block face material spec, doing nothing"),
			}
		}
		bm.insert(block);
	}

	Ok(())
}



#[derive(Debug)]
pub struct Block {
	pub name: String,
	pub xp_material_idx: usize,
	pub yp_material_idx: usize,
	pub zp_material_idx: usize,
	pub xn_material_idx: usize,
	pub yn_material_idx: usize,
	pub zn_material_idx: usize,
}
impl Block {
	pub fn new(name: &String) -> Self {
		Self {
			name: name.clone(),
			xp_material_idx: 0,
			yp_material_idx: 0,
			zp_material_idx: 0,
			xn_material_idx: 0,
			yn_material_idx: 0,
			zn_material_idx: 0,
		}
	}
}



// #[derive(Debug)]
// pub struct Block {
// 	pub name: String,
// 	pub material_idx: u32,	// For now block faces share the same material
// }



#[derive(Debug)]
pub struct BlockManager {
	blocks: Vec<Block>,
	index_name: HashMap<String, usize>,
}
impl BlockManager {
	pub fn new() -> Self {
		Self {
			blocks: Vec::new(),
			index_name: HashMap::new(),
		}
	}

	pub fn insert(&mut self, block: Block) -> usize {
		let idx = self.blocks.len();
		self.index_name.insert(block.name.clone(), idx);
		self.blocks.push(block);
		idx
	}

	pub fn index(&self, i: usize) -> &Block {
		&self.blocks[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.index_name.contains_key(name) {
			Some(self.index_name[name])
		} else {
			None
		}
	}

	/// Creates an encoding map for a run-length encoding.
	/// 
	/// encoding id -> unique index (for encoding).
	/// Empty is not included
	/// 
	/// unique index -> block name (for decoding)
	pub fn encoding_maps(&self, rle: &Vec<(usize, u32)>) -> (HashMap<usize, usize>, Vec<String>) {
		
		// Find unique encoding ids which are not zero
		let mut uniques = rle.iter().filter_map(|&(e_id, _)| {
			if e_id > 0 {
				Some(e_id)
			} else {
				None
			}
		}).collect::<Vec<_>>();
		uniques.sort();
		uniques.dedup();

		// Create a mapping to find their index in this sorted list
		// encoding id -> unique index
		let uidx_map = uniques.iter().enumerate().map(|(uidx, &e_id)| {
			(e_id, uidx)
		}).collect::<HashMap<_,_>>();

		// Map each unique non-zero encoding id to its block name
		// unique index -> block name
		let name_map = uniques.iter().map(|&e_id| {
			self.blocks[e_id-1].name.clone()
		}).collect::<Vec<_>>();

		(uidx_map, name_map)
	}
}



#[derive(Debug)]
pub enum BlockModReason {
	WorldGenSet(Voxel),
	Explosion(f32),
}



#[derive(Debug, Copy, Clone)]
pub enum VoxelPosition {
	WorldPosition([i32; 3]),
	ChunkPosition {
		chunk_position: [i32; 3], 
		voxel_position: [i32; 3],
	},
}
impl VoxelPosition {
	pub fn from_chunk_voxel(chunk_position: [i32; 3], voxel_position: [i32; 3]) -> Self {
		Self::ChunkPosition {
			chunk_position, 
			voxel_position,
		}
	}

	pub fn from_world(world_position: [i32; 3]) -> Self {
		Self::WorldPosition(world_position)
	}

	pub fn chunk_voxel_position(&self, chunk_size: [u32; 3]) -> ([i32; 3], [i32; 3]) {
		match *self {
			VoxelPosition::WorldPosition(world_position) => {
				let chunk_position = [
					world_position[0].div_euclid(chunk_size[0] as i32),
					world_position[1].div_euclid(chunk_size[1] as i32),
					world_position[2].div_euclid(chunk_size[2] as i32),
				];
				let mut voxel_position = [
					world_position[0] % (chunk_size[0] as i32),
					world_position[1] % (chunk_size[1] as i32),
					world_position[2] % (chunk_size[2] as i32),
				];
				voxel_position.iter_mut().zip(chunk_size.iter()).for_each(|(v, &cs)| {
					if *v < 0 {
						*v = cs as i32 + *v;
					}
				});
				(chunk_position, voxel_position)
			}
			VoxelPosition::ChunkPosition { chunk_position, voxel_position } => {
				(chunk_position, voxel_position)
			}
		}
	}
	
	pub fn world_position(&self, chunk_size: [u32; 3]) -> [i32; 3] {
		match *self {
			VoxelPosition::WorldPosition(world_position) => {
				world_position
			}
			VoxelPosition::ChunkPosition { chunk_position, voxel_position } => {
				let world_position = [
					chunk_position[0] * chunk_size[0] as i32 + voxel_position[0],
					chunk_position[1] * chunk_size[1] as i32 + voxel_position[1],
					chunk_position[2] * chunk_size[2] as i32 + voxel_position[2],
				];
				world_position
			}
		}
	}
}



#[derive(Debug)]
pub struct BlockMod {
	pub position: VoxelPosition,
	pub reason: BlockModReason,
}
pub type ChunkBlockMods = HashMap<[i32; 3], Vec<BlockMod>>;



/// Appends a to b, leaving b empty
pub fn append_chunkblockmods(
	a: &mut ChunkBlockMods, 
	b: ChunkBlockMods,
) {
	for (p, mut q) in b {
		if a.contains_key(&p) {
			let a_q = a.get_mut(&p).unwrap();
			a_q.append(&mut q);
		} else {
			a.insert(p, q);
		}
	}
}



pub fn insert_chunkblockmods(
	a: &mut ChunkBlockMods, 
	bm: BlockMod,
	chunk_size: [u32; 3],
) {
	let (chunk_position, voxel_position) = bm.position.chunk_voxel_position(chunk_size);
	let bm = BlockMod {
		position: VoxelPosition::from_chunk_voxel(chunk_position, voxel_position),
		..bm
	};
	if a.contains_key(&chunk_position) {
		let q = a.get_mut(&chunk_position).unwrap();
		q.push(bm);
	} else {
		a.insert(chunk_position, vec![bm]);
	}
}
