pub mod blocks;
pub use blocks::*;
pub mod array_volume;
pub use array_volume::*;
pub mod terrain;
pub use terrain::*;
pub mod chunk;
pub use chunk::*;









// #[derive(Debug, Clone, Copy)]
// pub struct WorldVoxel(IVec3);
// impl WorldVoxel {
// 	pub fn as_chunk_relative(self) -> (IVec3, ChunkVoxel) {
// 		let c = chunk_of_voxel(self.0);
// 		let r = self.0 - c * (CHUNK_SIZE as i32);
// 		(c, ChunkVoxel(r))
// 	}
// }


// #[derive(Debug, Clone, Copy)]
// pub struct ChunkVoxel(IVec3);
// impl ChunkVoxel {
// 	pub fn as_world_voxel(self, chunk: IVec3) -> WorldVoxel {
// 		WorldVoxel(self.0 + chunk * (CHUNK_SIZE as i32))
// 	}
// }
// impl std::ops::Deref for ChunkVoxel {
// 	type Target = IVec3;
// 	fn deref(&self) -> &Self::Target {
// 		&self.0
// 	}
// }





#[derive(Debug, Clone, Copy)]
pub struct SpiralIterator2D {
	axis: usize,
    direction: i32,
    position: IVec2,
    i: u32,
    i_max: u32,
    cur_iter: u32,
    max_iter: u32,
}
impl SpiralIterator2D {
	pub fn new(extent: u32, start: IVec2) -> Self {
		let i_max = extent.pow(2);
		Self {
			axis: 0,
			direction: 1,
			position: start,
			i: 0,
			i_max,
			cur_iter: 0,
			max_iter: 1,
		}
	}
}
impl Iterator for SpiralIterator2D {
	type Item = IVec2;
	fn next(&mut self) -> Option<Self::Item> {
		self.i += 1;
        if self.i > self.i_max {
            return None;
        }
        let p = self.position;

        if self.cur_iter == self.max_iter {
            self.cur_iter = 0;
            self.axis = (self.axis + 1) % 2;

            if self.axis == 0 {
                self.max_iter += 1;
                self.direction *= -1;
            }
        }
        self.position[self.axis] += self.direction;
        self.cur_iter += 1;
        Some(p)
	}
}


#[test]
fn test_cube() {
	let v = VoxelCube::new(IVec3::ZERO, UVec3::splat(16));

	println!("{}, {}", v.min(), v.max());
	println!("{:?}, {:?}", v.iter().into_iter().next(), v.iter().into_iter().last());
	for p in v.iter() {
		assert!(v.contains(p));
	}
}