use glam::UVec3;

use crate::util::KGeneration;
use super::{ArrayVolume, BlockKey, CHUNK_SIZE};



#[derive(Clone)]
pub struct Chunk {
	pub storage: ArrayVolume<BlockKey>, // array or octree
	pub generation: KGeneration,
	// tickable blocks? 
	// other info?
}
impl Chunk {
	pub fn new() -> Self {
		Self {
			storage: ArrayVolume::new(UVec3::new(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE)),
			generation: KGeneration::new(),
		}
	}
}
impl std::fmt::Debug for Chunk {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Chunk")
			.field("generation", &self.generation)
			.finish()
	}
}
impl std::ops::Deref for Chunk {
	type Target = ArrayVolume<BlockKey>;
	fn deref(&self) -> &Self::Target {
		&self.storage
	}
}
impl std::ops::DerefMut for Chunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.storage
	}
}
