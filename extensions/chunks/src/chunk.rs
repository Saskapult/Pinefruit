use glam::UVec3;
use crate::generation::KGeneration;
use super::{array_volume::ArrayVolume, CHUNK_SIZE};



/// A generic container for a generational volume. 
#[derive(Clone)]
pub struct Chunk<T: std::fmt::Debug + Clone> {
	pub contents: ArrayVolume<T>, // array or octree
	pub generation: KGeneration,
	// tickable blocks? 
	// other info?
}
impl<T: std::fmt::Debug + Clone> Chunk<T> {
	pub fn new() -> Self {
		Self {
			contents: ArrayVolume::new(UVec3::new(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE)),
			generation: KGeneration::new(),
		}
	}

	pub fn new_with_contents(contents: ArrayVolume<T>) -> Self {
		Self {
			contents,
			generation: KGeneration::new(),
		}
	}
}
impl<T: std::fmt::Debug + Clone> std::fmt::Debug for Chunk<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Chunk")
			.field("generation", &self.generation)
			.finish()
	}
}
impl<T: std::fmt::Debug + Clone> std::ops::Deref for Chunk<T> {
	type Target = ArrayVolume<T>;
	fn deref(&self) -> &Self::Target {
		&self.contents
	}
}
impl<T: std::fmt::Debug + Clone> std::ops::DerefMut for Chunk<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.contents
	}
}
