use std::sync::Arc;
use eks::{query::ResMut, prelude::Resource};
use glam::UVec3;
use parking_lot::RwLock;
use slotmap::SecondaryMap;
use eks::Resource;
use crate::{voxel::{ArrayVolume, BlockKey, CHUNK_SIZE}, util::KGeneration};
use super::{chunks::ChunkKey};



type VolumeType = (u8, BlockKey);

pub struct LiquidChunk {
	// u8 for liquid level
	volume: ArrayVolume<VolumeType>,
	pub generation: KGeneration,
}
impl LiquidChunk {
	pub fn new() -> Self {
		Self {
			volume: ArrayVolume::new(UVec3::splat(CHUNK_SIZE)),
			generation: KGeneration::new(),
		}
	}
}
impl std::fmt::Debug for LiquidChunk {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LiquidChunk")
			.field("generation", &self.generation)
			.finish()
	}
}
impl std::ops::Deref for LiquidChunk {
	type Target = ArrayVolume<VolumeType>;
	fn deref(&self) -> &Self::Target {
		&self.volume
	}
}
impl std::ops::DerefMut for LiquidChunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.volume
	}
}


#[derive(Debug, Default, Resource)]
pub struct LiquidChunksResource {
	pub chunks: Arc<RwLock<SecondaryMap<ChunkKey, LiquidChunk>>>,
}


// pub fn liquids_system(
// 	chunks: ResMut<MapResource>,
// 	liquid_chunks: ResMut<LiquidChunksResource>,
// ) {
	
// 	let mut liquid_chunks = liquid_chunks.chunks.write();
// 	let chunks = chunks.chunks.read();

// 	{ // Insert new
// 		for (_, &key) in chunks.positions.iter() {
// 			if !liquid_chunks.contains_key(key) {
// 				liquid_chunks.insert(key, LiquidChunk::new());
// 			}
// 		}
// 	}
	
// 	{ // Remove old
// 		liquid_chunks.retain(|key, _| chunks.entries.contains_key(key));
// 	}
	
// 	// Tick?
// 	for (_, _chunk) in liquid_chunks.iter_mut() {
		
// 	}

// }

