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
	// pub script: PathBuf,
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
				_ => warn!("Weird block face material spec (what is '{face}'?), doing nothing"),
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

	// The following are here to remind you that they should exist
	// It would be best to load these from a lua function
	// block.lua
	//  - on_create
	//  - on_interact
	//  - on_destroy
	//  - load data (vec<u8>) (specific to things with data)
	//  - Other stuff not called from outside
	// Blocks could store the names of functions to use or check if it has an "on _" function
	pub fn on_create(&self, _instance: BlockInstance) -> Result<()> {
		// The on_create method for a chest should either:
		//  - get an index to a generic inventory container
		//  - create a block instance for the chunk
		// (allows for shared contents, stored in world?)
		Ok(())
	}
	// Takes interaction item?
	// Hand interaction, stepped on, what others might there be?
	pub fn on_interact(&self, _instance: BlockInstance) -> Result<()> {
		Ok(())
	}
	// Called when interacted, item inserted, bleh
	pub fn on_updata(&self, _instance: BlockInstance) -> Result<()> {
		// Furnace should step across to current time, checking if there is enough fuel
		Ok(())
	}
	pub fn on_destroy(&self, _instance: BlockInstance) -> Result<()> {
		Ok(())
	}
	// How to do event hooks?
	// Need to expose:
	//  - change face material
	//  - play sound
}



// Data for the block, to be used by the block's script
#[derive(Debug)]
pub struct BlockInstance {
	data: Vec<u8>,
}



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



#[derive(Debug, Copy, Clone)]
pub enum BlockModReason {
	WorldGenSet(Voxel),
	Explosion(f32),
}



#[derive(Debug, Copy, Clone)]
pub enum VoxelPosition {
	WorldRelative([i32; 3]),
	ChunkRelative {
		chunk_position: [i32; 3], 
		voxel_position: [i32; 3],
	},
}
impl VoxelPosition {
	pub fn from_chunk_voxel(chunk_position: [i32; 3], voxel_position: [i32; 3]) -> Self {
		Self::ChunkRelative {
			chunk_position, 
			voxel_position,
		}
	}

	pub fn from_world(world_position: [i32; 3]) -> Self {
		Self::WorldRelative(world_position)
	}

	pub fn chunk_voxel_position(&self, chunk_size: [u32; 3]) -> ([i32; 3], [i32; 3]) {
		match *self {
			VoxelPosition::WorldRelative(world_position) => {
				crate::world::map::world_chunk_voxel(world_position, chunk_size)
			}
			VoxelPosition::ChunkRelative { chunk_position, voxel_position } => {
				(chunk_position, voxel_position)
			}
		}
	}
	
	pub fn world_position(&self, chunk_size: [u32; 3]) -> [i32; 3] {
		match *self {
			VoxelPosition::WorldRelative(world_position) => {
				world_position
			}
			VoxelPosition::ChunkRelative { chunk_position, voxel_position } => {
				crate::world::map::chunk_voxel_world(chunk_position, voxel_position, chunk_size)
			}
		}
	}
}



#[derive(Debug, Clone)]
pub struct BlockMod {
	pub position: VoxelPosition,
	pub reason: BlockModReason,
}



/// Block mods arranged by chunk
#[derive(Debug, Clone)]
pub struct ChunkBlockMods {
	chunk_size: [u32; 3],
	contents: HashMap<[i32; 3], Vec<BlockMod>>,
}
impl ChunkBlockMods {
	pub fn new(chunk_size: [u32; 3]) -> Self {
		Self {
			chunk_size,
			contents: HashMap::new(),
		}
	}

	pub fn contains_key(&self, key: &[i32; 3]) -> bool {
		self.contents.contains_key(key)
	}
}
impl std::ops::Add<ChunkBlockMods> for ChunkBlockMods {
	type Output = Self;

	fn add(mut self, _rhs: Self) -> Self {
		if self.chunk_size != _rhs.chunk_size {
			panic!()
		}

		for (chunk_position, mut queue) in _rhs.contents {
			if self.contents.contains_key(&chunk_position) {
				let self_queue = self.contents.get_mut(&chunk_position).unwrap();
				self_queue.append(&mut queue);
			} else {
				self.contents.insert(chunk_position, queue);
			}
		}

		self
	}
}
impl std::ops::AddAssign<ChunkBlockMods> for ChunkBlockMods {
	fn add_assign(&mut self, other: ChunkBlockMods) {
		for (p, mut q) in other {
			if self.contents.contains_key(&p) {
				let a_q = self.contents.get_mut(&p).unwrap();
				a_q.append(&mut q);
			} else {
				self.contents.insert(p, q);
			}
		}
	}
}
impl std::ops::Add<BlockMod> for ChunkBlockMods {
	type Output = Self;

	fn add(mut self, _rhs: BlockMod) -> Self {
		let (chunk_position, voxel_position) = _rhs.position.chunk_voxel_position(self.chunk_size);
		let bm = BlockMod {
			position: VoxelPosition::from_chunk_voxel(chunk_position, voxel_position),
			.._rhs
		};
		if self.contents.contains_key(&chunk_position) {
			let queue = self.contents.get_mut(&chunk_position).unwrap();
			queue.push(_rhs);
		} else {
			self.contents.insert(chunk_position, vec![bm]);
		}
		self
	}
}
impl std::ops::AddAssign<BlockMod> for ChunkBlockMods {
	fn add_assign(&mut self, other: BlockMod) {
		let (chunk_position, voxel_position) = other.position.chunk_voxel_position(self.chunk_size);
		let bm = BlockMod {
			position: VoxelPosition::from_chunk_voxel(chunk_position, voxel_position),
			..other
		};
		if self.contents.contains_key(&chunk_position) {
			let queue = self.contents.get_mut(&chunk_position).unwrap();
			queue.push(bm);
		} else {
			self.contents.insert(chunk_position, vec![bm]);
		}
	}
}
impl std::ops::Index<[i32; 3]> for ChunkBlockMods {
	type Output = Vec<BlockMod>;

	fn index(&self, idx: [i32; 3]) -> &Self::Output {
		self.contents.get(&idx).unwrap()
	}
}
impl std::ops::IndexMut<[i32; 3]> for ChunkBlockMods {
	fn index_mut(&mut self, idx: [i32; 3]) -> &mut Self::Output {
		self.contents.get_mut(&idx).unwrap()
	}
}
impl std::iter::IntoIterator for ChunkBlockMods {
	type Item = ([i32; 3], Vec<BlockMod>);
	type IntoIter = std::collections::hash_map::IntoIter<[i32; 3], Vec<BlockMod>>;

	fn into_iter(self) -> Self::IntoIter {
		self.contents.into_iter()
	}
}
