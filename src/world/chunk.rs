use crate::world::*;




#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
}



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
	fn rle(&self) -> Vec<(usize, u32)> {
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
	fn rld(mut self, rle: &Vec<(usize, u32)>) -> Self {
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

	pub fn base(self, self_position: [i32; 3], base_generator: &impl BaseGenerator, bm: &BlockManager) -> Self {
		base_generator.chunk_base(self_position, self, bm)
	}
}



#[cfg(test)]
mod tests {
    use super::*;
	use rand::prelude::*;

	fn randomize_chunk(mut chunk: Chunk) -> Chunk {
		let mut rng = thread_rng();
		for i in 0..chunk.size[0] {
			for j in 0..chunk.size[1] {
				for k in 0..chunk.size[2] {
					let rn = (rng.gen::<f32>() * 8.0) as usize;
					let voxel = match rn == 0 {
						true => Voxel::Empty,
						false => Voxel::Block(rn -1),
					};
					chunk.set_voxel([i as i32, j as i32, k as i32], voxel)
				}
			}
		}
		chunk
	}

    #[test]
    fn test_encode_decode() {
		const CHUNKSIZE: [u32; 3] = [16, 16, 16];

        let chunk1 = randomize_chunk(Chunk::new(CHUNKSIZE));
		let rle = chunk1.rle();
		let chunk2 = Chunk::new(CHUNKSIZE).rld(&rle);

		// println!("{:?}", &chunk1.contents[chunk1.contents.len()-5..]);
		// println!("[(id, len)] = {:?}", &rle[rle.len()-5..]);
		// println!("{:?}",  &chunk2.contents[chunk2.contents.len()-5..]);

        assert_eq!(chunk1, chunk2);
    }
}
