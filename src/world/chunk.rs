


#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Voxel {
	Empty,
	Block(u32),
}



#[derive(Debug, Clone)]
pub struct Chunk {
	pub size: [u32; 3],
	pub contents: Vec<Voxel>,
	// Won't have block map because same type (memory size) is used for storage anyway
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

	fn rle(&self) -> String {
		let mut runs = Vec::new(); // (id, length)
		let mut vox = self.contents[0];
		let mut len = 1;
		for voxel in &self.contents[1..] {
			if *voxel != vox {
				let vid = match *voxel {
					Voxel::Empty => 0,
					Voxel::Block(bid) => bid+1,
				};
				runs.push((vid, len));
				vox = *voxel;
				len = 1;
			} else {
				len += 1;
			}
		}
		todo!()
	}
	
	fn rld(&mut self, _rle: String) {
		todo!()
	}
}



