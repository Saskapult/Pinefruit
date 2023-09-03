use glam::UVec3;

use crate::util::KGeneration;
use super::{ArrayVolume, BlockKey};


/// Determines the chunk extent for the whole project! 
/// It's here so I can replace it with 32 to test things. 
/// 
/// Hopefully I used this instead of just plugging in numbers...
pub const CHUNK_SIZE: u32 = 16;



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
