pub mod blocks;
pub use blocks::*;
pub mod array_volume;
pub use array_volume::*;
pub mod terrain;
pub use terrain::*;
pub mod chunk;
pub use chunk::*;


use glam::{Vec3, IVec3, IVec2, UVec3};



/// Chunk side length. 
/// Determines the chunk extent for the whole project! 
/// It's here so I can replace it with 32 to test things. 
/// 
/// Hopefully I used this instead of just plugging in numbers...
pub const CHUNK_SIZE: u32 = 16;


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


pub fn cube_iterator_xyz_uvec(size: UVec3) -> impl Iterator<Item = UVec3> {
	let [x, y, z] = size.to_array();
	(0..x).flat_map(move |x| {
		(0..y).flat_map(move |y| {
			(0..z).map(move |z| {
				UVec3::new(x, y, z)
			})
		})
	})
} 


#[derive(Debug)]
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


#[derive(Debug)]
pub struct VoxelCube {
	pub centre: IVec3,
	pub half_edge_length: UVec3,
}
impl VoxelCube {
	pub fn new(centre: IVec3, half_edge_length: UVec3) -> Self {
		Self { centre, half_edge_length, }
	}

	/// Expands the half edge length by a value
	pub fn expand(mut self, by: UVec3) -> Self {
		self.half_edge_length += by;
		self
	}

	pub fn edge_length(&self) -> UVec3 {
		self.half_edge_length * 2 + UVec3::ONE
	}

	pub fn min(&self) -> IVec3 {
		self.centre - self.half_edge_length.as_ivec3()
	}

	pub fn max(&self) -> IVec3 {
		self.centre + self.half_edge_length.as_ivec3()
	}

	pub fn contains(&self, postion: IVec3) -> bool {
		(postion - self.centre).abs().as_uvec3().cmplt(self.half_edge_length + 1).all()
	}

	pub fn iter(&self) -> impl IntoIterator<Item = IVec3> + '_ {
		let base_pos = self.min();
		cube_iterator_xyz_uvec(self.edge_length())
			.map(move |p| base_pos + p.as_ivec3() )
	}
}


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
pub struct VoxelModification {
	pub position: IVec3, // Usually world-relative, but it's left unclear so we don't have to write as much code
	pub set_to: Option<BlockKey>,
	pub priority: u32,
}
impl VoxelModification {
	// This should return another type of struct but I'm lazy
	pub fn as_chunk_relative(mut self) -> (IVec3, Self) {
		let c = chunk_of_voxel(self.position);
		self.position -= c * (CHUNK_SIZE as i32);
		(c, self)
	}
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


#[test]
fn test_cube() {
	let v = VoxelCube::new(IVec3::ZERO, UVec3::splat(16));

	println!("{}, {}", v.min(), v.max());
	println!("{:?}, {:?}", v.iter().into_iter().next(), v.iter().into_iter().last());
	for p in v.iter() {
		assert!(v.contains(p));
	}
}