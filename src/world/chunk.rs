use crate::world::*;




#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Voxel {
	Empty,
	Block(usize),
}
impl Voxel {
	pub fn is_empty(&self) -> bool {
		match self {
			Voxel::Empty => true,
			_ => false,
		}
	}
	pub fn unwrap_id(&self) -> usize {
		match self {
			Voxel::Block(id) => *id,
			_ => panic!("Tried to unwrap an empty voxel!"),
		}
	}
}



// Todo: make this better for vertical iteration
//  (x * xs * zs) + (z * zs) + y
//  Would it be better for x->z->y or z->x->y?
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
	pub size: [u32; 3],
	pub contents: Vec<Voxel>,
	// Won't have block map because same type (memory size) is used for storage anyway
	//fully_empty: bool
}
impl Chunk {
	pub fn new(size: [u32; 3]) -> Self {
		let contents = vec![Voxel::Empty; (size[0] * size[1] * size[2]) as usize];
		Self { size, contents, }
	}

	pub fn new_of(size: [u32; 3], contents: Voxel) -> Self {
		let contents = vec![contents; (size[0] * size[1] * size[2]) as usize];
		Self { size, contents, }
	}
	
	pub fn get_voxel(&self, position: [i32; 3]) -> Voxel {
		let idx = 
			(position[0] * self.size[0] as i32 * self.size[1] as i32 + 
			position[1] * self.size[1] as i32 +
			position[2]) as usize;
		self.contents[idx]
	}
	
	pub fn set_voxel(&mut self, position: [i32; 3], voxel: Voxel) {
		let idx = 
			(position[0] * self.size[0] as i32 * self.size[1] as i32 + 
			position[1] * self.size[1] as i32 +
			position[2]) as usize;
		self.contents[idx] = voxel;
	}

	pub fn is_in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
		(x < self.size[0] as i32 && y < self.size[1] as i32 && z < self.size[2] as i32) && (x >= 0 && y >= 0 && z >= 0)
	}

	// To be used for meshing voxel models
	pub fn mesh() {
		todo!()
	}
	pub fn greedy_mesh() {
		todo!()
	}

	/// Creates a run-length encoding of the chunk.
	/// Does NOT create a mapping for this, uses raw block ids.
	pub fn rle(&self) -> Vec<(usize, u32)> {
		let mut runs = Vec::new(); // (id, length)
		let mut last_voxel = self.contents[0];
		let mut len = 1;
		self.contents[1..].iter().for_each(|&voxel| {
			if voxel == last_voxel {
				len += 1;
			} else {
				let vid = match last_voxel {
					Voxel::Empty => 0,
					Voxel::Block(bid) => bid+1,
				};
				runs.push((vid, len));
				last_voxel = voxel;
				len = 1;
			}
		});
		// Add the last bit
		let vid = match last_voxel {
			Voxel::Empty => 0,
			Voxel::Block(bid) => bid+1,
		};
		runs.push((vid, len));

		runs
	}
	
	/// Decodes self from a run-length encoding.
	/// Like rle, it does NOT use mappings, instead using raw ids.
	pub fn rld(mut self, rle: &Vec<(usize, u32)>) -> Self {
		let mut voxel_position = 0;
		rle.iter().for_each(|&(id, length)| {
			let voxel = match id == 0 {
				true => Voxel::Empty,
				false => Voxel::Block(id - 1),
			};
			(0..length).for_each(|_| {
				self.contents[voxel_position] = voxel;
				voxel_position += 1;
			});
		});
		self
	}

	pub fn carve(self, self_position: [i32; 3], carver: &impl Carver) -> Self {
		carver.carve_chunk(self_position, self)
	}

	pub fn base(self, self_position: [i32; 3], base_generator: &impl SurfaceGenerator, bm: &BlockManager) -> Self {
		base_generator.chunk_surface(self_position, self, bm)
	}
}



pub fn chunk_seed(chunk_position: [i32; 3], world_seed: u64) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};

	let mut s = DefaultHasher::new();
	chunk_position.hash(&mut s);

	s.finish() ^ world_seed
}



#[cfg(test)]
mod tests {
	use super::*;

    #[test]
    fn chunk_seed_test() {
		const G: i32 = 3;

		let world_seed = rand::random::<u64>();
		
		let output = (-G..=G).flat_map(|x| {
			(-G..=G).flat_map(move |y| {
				(-G..=G).map(move |z| {
					chunk_seed([x, y, z], world_seed)
				})
			})
		}).collect::<Vec<_>>();

		println!("{:?}", output);

        assert_eq!(2 + 2, 4);
    }
}
