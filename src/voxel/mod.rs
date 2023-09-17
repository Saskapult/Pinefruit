pub mod blocks;
pub use blocks::*;
pub mod array_volume;
pub use array_volume::*;
pub mod terrain;
pub use terrain::*;
pub mod chunk;
pub use chunk::*;


use glam::{Vec3, IVec3, IVec2};



/// Chunk side length
const CHUNK_SIZE: u32 = 16;


pub fn voxel_of_point(point: Vec3) -> IVec3 {
	point.floor().as_ivec3()
}


pub fn voxel_relative_to_chunk(voxel: IVec3, chunk: IVec3) -> IVec3 {
	let cv = chunk * IVec3::new(CHUNK_SIZE as i32, CHUNK_SIZE as i32, CHUNK_SIZE as i32);
	voxel - cv
}


pub fn chunk_of_voxel(voxel: IVec3) -> IVec3 {
	IVec3::new(
		voxel.x.div_euclid(CHUNK_SIZE as i32),
		voxel.y.div_euclid(CHUNK_SIZE as i32),
		voxel.z.div_euclid(CHUNK_SIZE as i32),
	)
}


pub fn chunk_of_point(point: Vec3) -> IVec3 {
	chunk_of_voxel(voxel_of_point(point))
}


pub struct VoxelSphere {
	pub centre: IVec3,
	pub radius: i32,
}
impl VoxelSphere {
	pub fn new(centre: IVec3, radius: i32) -> Self {
		Self { centre, radius, }
	}

	pub fn is_within(&self, position: IVec3) -> bool {
		let x = position.x - self.centre.x;
		let y = position.y - self.centre.y;
		let z = position.z - self.centre.z;
		(x.pow(2) + y.pow(2) + z.pow(2)) <= self.radius.pow(2)
	}

	pub fn iter(&self) -> impl IntoIterator<Item = IVec3> + '_ {
		((self.centre.x-self.radius)..=(self.centre.x+self.radius)).flat_map(move |x|
			((self.centre.y-self.radius)..=(self.centre.y+self.radius)).flat_map(move |y|
				((self.centre.z-self.radius)..=(self.centre.z+self.radius)).map(move |z| IVec3::new(x, y, z))))
					.filter(|&position| self.is_within(position))
	}
}


#[derive(Debug, Clone, Copy)]
pub struct VoxelModification {
	pub position: IVec3, // Usually world-relative, but it's left unclear so we don't have to write as much code
	pub set_to: Option<BlockKey>,
	pub priority: u32,
}


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
