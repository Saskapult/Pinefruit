pub mod blocks;
pub use blocks::*;
pub mod array_volume;
pub use array_volume::*;
pub mod terrain;
pub use terrain::*;
pub mod chunk;
pub use chunk::*;


use glam::{Vec3, IVec3};



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


// #[cfg(test)]
// mod tests {
//     use super::*;
// 	use rand::prelude::*;

// 	fn randomize_chunk(mut chunk: ArrayVolume, brange: f32) -> ArrayVolume {
// 		let mut rng = thread_rng();
// 		for i in 0..chunk.size[0] {
// 			for j in 0..chunk.size[1] {
// 				for k in 0..chunk.size[2] {
// 					let rn = (rng.gen::<f32>() * brange) as usize;
// 					let voxel = match rn == 0 {
// 						true => Voxel::Empty,
// 						false => Voxel::Block(rn -1),
// 					};
// 					chunk.set_voxel([i as i32, j as i32, k as i32], voxel)
// 				}
// 			}
// 		}
// 		chunk
// 	}

//     #[test]
//     fn test_chunk_encode_decode() {
// 		const CHUNKSIZE: [u32; 3] = [16, 16, 16];

//         let chunk1 = randomize_chunk(ArrayVolume::new(CHUNKSIZE), 8 as f32);
// 		let rle = chunk1.rle();
// 		let chunk2 = ArrayVolume::new(CHUNKSIZE).rld(&rle);

//         assert_eq!(chunk1, chunk2);
//     }

// 	#[test]
//     fn test_chunk_rle_map() {
// 		const CHUNKSIZE: [u32; 3] = [2, 2, 2];

// 		let mut bm = BlockManager::new();
// 		(0..=8).for_each(|i| {
// 			bm.insert(Block::new(
// 				&format!("block {}", i)
// 			));
// 		});

//         let chunk1 = randomize_chunk(ArrayVolume::new(CHUNKSIZE), 8 as f32);
		
// 		let rle = chunk1.rle();
// 		let (name_idx_map, name_map) = bm.encoding_maps(&rle);
		
// 		let mapped_rle = rle.iter().map(|&(e_id, len)| {
// 			if e_id == 0 {
// 				// If it is empty don't map it
// 				(e_id, len)
// 			} else {
// 				// Otherwise find its index in uniques
// 				println!("map {} ({}) -> {}", e_id, &bm.index(e_id-1).name, name_idx_map[&e_id]);
// 				let name_idx = name_idx_map[&e_id];
// 				// Don't map to zero
// 				(name_idx + 1, len)
// 			}
// 		}).collect::<Vec<_>>();
		
// 		let unmapped_rle = mapped_rle.iter().map(|&(name_idx, len)| {
// 			if name_idx == 0 {
// 				// If idx is 0 then it was always 0 and represents empty
// 				(name_idx, len)
// 			} else {
// 				// Unmap from not mapping to zero
// 				let corrected_name_idx = name_idx - 1;
// 				let name = &name_map[corrected_name_idx];
// 				let e_id = bm.index_name(name).unwrap() + 1;

// 				println!("unmap {} -> {} -> {}", corrected_name_idx, name, e_id);
// 				(e_id, len)
// 			}
// 		}).collect::<Vec<_>>();

//         assert_eq!(rle, unmapped_rle);
//     }

// 	#[test]
//     fn test_chunk_serde() {
// 		const CHUNKSIZE: [u32; 3] = [16, 16, 16];

//         let mut bm = BlockManager::new();
// 		(0..=8).for_each(|i| {
// 			bm.insert(Block::new(
// 				&format!("block {}", i)
// 			));
// 		});

//         let chunk1 = randomize_chunk(ArrayVolume::new(CHUNKSIZE), 8 as f32);
		
// 		let rle = chunk1.rle();
// 		let (name_idx_map, name_map) = bm.encoding_maps(&rle);
		
// 		let mapped_rle = rle.iter().map(|&(e_id, len)| {
// 			if e_id == 0 {
// 				// If it is empty don't map it
// 				(e_id, len)
// 			} else {
// 				// Otherwise find its index in uniques
// 				// println!("map {} ({}) -> {}", e_id, &bm.index(e_id-1).name, name_idx_map[&e_id]);
// 				let name_idx = name_idx_map[&e_id];
// 				// Don't map to zero
// 				(name_idx + 1, len)
// 			}
// 		}).collect::<Vec<_>>();
		
// 		let save_path = "/tmp/chunktest.ron";
// 		{ // Save
			
// 			let f = std::fs::File::create(&save_path).unwrap();
// 			ron::ser::to_writer(f, &(name_map, mapped_rle)).unwrap();
// 		}
// 		// Read
// 		let f = std::fs::File::open(&save_path).unwrap();
// 		let (read_name_map, read_mapped_rle): (Vec<String>, Vec<(usize, u32)>) = ron::de::from_reader(f).unwrap();

// 		let unmapped_rle = read_mapped_rle.iter().map(|&(name_idx, len)| {
// 			if name_idx == 0 {
// 				// If idx is 0 then it was always 0 and represents empty
// 				(name_idx, len)
// 			} else {
// 				// Unmap from not mapping to zero
// 				let corrected_name_idx = name_idx - 1;
// 				let name = &read_name_map[corrected_name_idx];
// 				let e_id = bm.index_name(name).unwrap() + 1;

// 				// println!("unmap {} -> {} -> {}", corrected_name_idx, name, e_id);
// 				(e_id, len)
// 			}
// 		}).collect::<Vec<_>>();
// 		let chunk2 = ArrayVolume::new(CHUNKSIZE).rld(&unmapped_rle);

//         assert_eq!(chunk1, chunk2);
//     }
// }
