

/*
We would ideally store attributes in bits.
I am too stupid to make that work.
We store attributes in bytes.
*/


pub struct VoxelVolumeHeader {
	pub attributes: Vec<VoxelAttribute>,
	pub bytes_per_voxel: u32,
}
impl VoxelVolumeHeader {
	pub fn new(attributes: Vec<VoxelAttribute>) -> Self {
		let bytes_per_voxel = attributes.iter()
			.fold(0, |ac, at| ac + at.bytes_per_element);
		Self { 
			attributes, 
			bytes_per_voxel,
		}
	}
	pub fn add_attribute(&mut self, attribute: VoxelAttribute) {
		self.bytes_per_voxel += attribute.bytes_per_element;
		self.attributes.push(attribute);
	}
}



pub struct VoxelAttribute {
	pub name: String,
	pub bytes_per_element: u32,
}
impl VoxelAttribute {
	pub fn write_me(&self, value: &[u8], destination: &mut [u8]) {
		for i in 0..self.bytes_per_element as usize {
			destination[i] = value[i];
		}
	}
}



/// Be very carful to only use one T, as consistency is not enforced.
pub struct DynamicArrayVoxelVolume {
	pub size: [u32; 3],
	pub header: VoxelVolumeHeader,
	pub contents: Vec<u8>,
}
impl DynamicArrayVoxelVolume {
	pub fn new(header: VoxelVolumeHeader, size: [u32; 3]) -> Self {
		let bytes_needed = size.iter().fold(1, |a, v| a * v) * header.bytes_per_voxel;

		Self {
			size, 
			header, 
			contents: vec![0; bytes_needed as usize],
		}
	}

	pub fn get<T: bytemuck::Pod + bytemuck::Zeroable>(&self, index: usize) -> &T {
		let bytes = &self.contents[index..(index + self.header.bytes_per_voxel as usize)];
		bytemuck::from_bytes::<T>(bytes)
	}

	pub fn set<T: bytemuck::Pod + bytemuck::Zeroable>(&mut self, index: usize, item: &T) {
		let bytes = bytemuck::bytes_of(item);
		for i in 0..bytes.len() {
			self.contents[index + i] = bytes[i];
		}
	}
}




pub struct TypedArrayVoxelVolume<T: std::fmt::Debug + Clone + PartialEq + Eq + Default> {
	pub size: [u32; 3],
	pub contents: Vec<T>,
}
impl<T: std::fmt::Debug + Clone + PartialEq + Eq + Default> TypedArrayVoxelVolume<T> {
	pub fn new(size: [u32; 3]) -> Self {
		let f = size.iter().fold(1, |a, &v| a * v) as usize;
		Self {
			size,
			contents: vec![T::default(); f],
		}
	}

	pub fn get_index(&self, index: usize) -> &T {
		&self.contents[index]
	}
	pub fn set_index(&mut self, index: usize, item: T) {
		self.contents[index] = item;
	}

	pub fn get(&self, position: [u32; 3]) -> Option<&T> {
		if (0..3).all(|i| self.size[i] < position[i]) {
			let index = (0..3)
				.map(|i| self.size[i] * position[i])
				.reduce(|a, v| a + v).unwrap();
			Some(self.get_index(index as usize))
		} else {
			None
		}
	}
	pub fn set(&mut self, position: [u32; 3], item: T) -> Option<()> {
		if (0..3).all(|i| self.size[i] < position[i]) {
			let index = (0..3)
				.map(|i| self.size[i] * position[i])
				.reduce(|a, v| a + v).unwrap();
			self.set_index(index as usize, item);
			Some(())
		} else {
			None
		}
	}

	// Useful for generating without random access while not having to know about the indexing format
	pub fn iter_mut(&mut self) -> impl Iterator<Item=([u32; 3], &mut T)> {
		let [sx, sy, sz] = self.size;
		let g = (0..sx).flat_map(move |x| {
			(0..sy).flat_map(move |y| {
				(0..sz).map(move |z| {
					[x,y,z]
				})
			})
		});
		g.zip(self.contents.iter_mut())
	}

	pub fn aabb_extent(&self) -> [[f32;3];2] {
		let max = self.size.map(|v| v as f32);
		let min = [0.0; 3];
		[min, max]
	}
}
