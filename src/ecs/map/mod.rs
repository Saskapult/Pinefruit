use std::collections::HashMap;
use std::sync::Arc;
use glam::IVec3;
use eks::prelude::*;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use slotmap::{new_key_type, SlotMap};
use crate::voxel::*;
use crate::voxel::chunk::CHUNK_SIZE;

pub mod loading;
pub mod modification;
pub mod octree;
pub mod model; 
pub mod looking;



// This should not be here I think
#[derive(Debug, ResourceIdent, Default)]
pub struct BlockResource {
	pub blocks: Arc<RwLock<BlockManager>>,
}
impl std::ops::Deref for BlockResource {
	type Target = Arc<RwLock<BlockManager>>;
	fn deref(&self) -> &Self::Target {
		&self.blocks
	}
}
impl std::ops::DerefMut for BlockResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.blocks
	}
}


// An entry in the mesh storage for a map component
#[derive(Debug, Clone)]
pub enum ChunkEntry {
	UnLoaded,	// Used if chunk does not exist yet
	Loading,	// Waiting for disk data done
	Generating,	// Waiting for generation done
	Complete(Arc<Chunk>),
}
impl ChunkEntry {
	pub fn complete(&self) -> Option<&Arc<Chunk>> {
		match self {
			ChunkEntry::Complete(c) => Some(c),
			_ => None,
		}
	}
	pub fn unwrap_complete(&self) -> &Arc<Chunk> {
		match self {
			ChunkEntry::Complete(c) => c,
			_ => panic!(),
		}
	}
}


new_key_type! {
	pub struct ChunkKey;
}


#[derive(Debug, Default)]
pub struct ChunkMap {
	pub positions: FxHashMap<IVec3, ChunkKey>,
	// Redundant information stored becasue we can't zip these to iterate
	pub entries: SlotMap<ChunkKey, (IVec3, ChunkEntry)>,
}
impl ChunkMap {
	pub fn get(&self, key: ChunkKey) -> Option<&ChunkEntry> {
		self.entries.get(key).and_then(|(_, e)| Some(e))
	}

	pub fn get_mut(&mut self, key: ChunkKey) -> Option<&mut ChunkEntry> {
		self.entries.get_mut(key).and_then(|(_, e)| Some(e))
	}

	pub fn key(&self, pos: &IVec3) -> Option<ChunkKey> {
		self.positions.get(pos).cloned()
	}

	pub fn insert(&mut self, pos: IVec3, chunk: ChunkEntry) -> ChunkKey {
		let k = self.entries.insert((pos, chunk));
		self.positions.insert(pos, k);
		k
	}

	pub fn remove(&mut self, pos: &IVec3) -> Option<(ChunkKey, ChunkEntry)> {
		self.positions.remove(pos).and_then(|k|
			self.entries.remove(k).and_then(|(_, e)| Some((k, e)))
		)
	}

	// No guarantee that order will be the same!
	// Can't do this
	// pub fn iter(&self) -> impl IntoIterator<Item = ((&IVec3, &ChunkKey), &ChunkEntry)> + '_ {
	// 	self.positions.iter().zip(self.entries.values())
	// }
	// pub fn iter_mut(&mut self) -> impl IntoIterator<Item = ((&IVec3, &ChunkKey), &mut ChunkEntry)> + '_ {
	// 	self.positions.iter().zip(self.entries.values_mut())
	// }

	pub fn len(&self) -> usize {
		self.entries.len()
	}
}


#[derive(Debug, ResourceIdent)]
pub struct MapResource {
	pub chunks: Arc<RwLock<ChunkMap>>,
	// Should these be in their own resource?
	// Multiplayer says yes
	// Todo: Have yet another hashmap for chunk-relative position
	// Would allow for prioritizing
	// Idea: Don't need the chunk hashing in that case, might cut down on 
	// overhead if we only stored world-relative
	pub block_mods: RwLock<HashMap<IVec3, Vec<VoxelModification>>>,
}
impl MapResource {
	pub fn new() -> Self {
		Self {
			chunks: Arc::new(RwLock::new(ChunkMap::default())),
			block_mods: RwLock::new(HashMap::new()),
		}
	}

	pub fn get_voxel(&self, voxel: IVec3) -> Option<BlockKey> {
		let chunk = chunk_of_voxel(voxel);
		let voxel = voxel_relative_to_chunk(voxel, chunk).as_uvec3();
		let chunks = self.chunks.read();
		match chunks.key(&chunk).and_then(|k| chunks.get(k)) {
			Some(ChunkEntry::Complete(c)) => c.get(voxel).copied(),
			_ => None,
		}
	}

	pub fn modify_voxel(&self, modification: VoxelModification) {
		let (c, r) = modification.as_chunk_relative();
		self.block_mods.write().entry(c).or_default().push(r);
	}

	pub fn modify_voxels(&self, modifications: &[VoxelModification]) {
		let mut block_mods = self.block_mods.write();
		
		for modification in modifications {
			let (c, r) = modification.as_chunk_relative();
			block_mods.entry(c).or_default().push(r);
		}
	}

	/// Gets some estimate of the map's data usage. 
	/// Assumes that chunks are stored with array volumes with no optimization
	pub fn approximate_size(&self) -> u64 {
		// Currently chunks store Option<BlockKey>, which is 64 bits in size
		(self.chunks.read().len() as u64) * 8 * CHUNK_SIZE as u64
	}
}



