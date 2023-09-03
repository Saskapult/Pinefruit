use std::collections::HashMap;
use std::sync::Arc;
use glam::IVec3;
use eks::prelude::*;
use parking_lot::RwLock;
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


#[derive(Debug, ResourceIdent)]
pub struct MapResource {
	pub chunks: Arc<RwLock<HashMap<IVec3, ChunkEntry>>>,
	pub block_mods: RwLock<Vec<VoxelModification>>, // could group by chunk for embarassing parallelization, also reduce by priority
}
impl MapResource {
	pub fn new() -> Self {
		Self {
			chunks: Arc::new(RwLock::new(HashMap::new())),
			block_mods: RwLock::new(Vec::new()),
		}
	}

	pub fn get_voxel(&self, voxel: IVec3) -> Option<BlockKey> {
		let chunk = chunk_of_voxel(voxel);
		let voxel = voxel_relative_to_chunk(voxel, chunk).as_uvec3();
		match self.chunks.read().get(&chunk) {
			Some(ChunkEntry::Complete(c)) => c.get(voxel).copied(),
			_ => None,
		}
	}

	pub fn modify_voxel(&self, modification: VoxelModification) {
		self.block_mods.write().push(modification);
	}
	pub fn modify_voxels(&self, modifications: &[VoxelModification]) {
		self.block_mods.write().extend_from_slice(modifications);
	}

	/// Gets soem estimation of the map's data usage. 
	/// Assumes that chunks are stored with array volumes with no optimization
	pub fn approximate_size(&self) -> u64 {
		// Currently chunks store Option<BlockKey>, which is 64 bits in size
		(self.chunks.read().len() as u64) * 8 * CHUNK_SIZE as u64
	}
}



