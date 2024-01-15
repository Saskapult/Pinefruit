use std::sync::Arc;
use eks::prelude::*;
use parking_lot::RwLock;
use crate::voxel::*;

use self::chunks::ChunkKey;

pub mod modification;
pub mod octree;
pub mod model; 
pub mod looking;
pub mod liquids;
pub mod light;
pub mod chunks;
pub mod terrain;



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
