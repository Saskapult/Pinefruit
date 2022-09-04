

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



#[derive(Debug)]
pub struct TypedArrayVoxelVolume<T: std::fmt::Debug + Clone + PartialEq + Eq + Default> {
	pub size: [u32; 3],
	pub contents: Vec<T>, // Encoding is x,y,z
}
impl<T: std::fmt::Debug + Clone + PartialEq + Eq + Default> TypedArrayVoxelVolume<T> {
	pub fn new(size: [u32; 3]) -> Self {
		let f = size.iter().fold(1, |a, &v| a * v) as usize;
		Self {
			size,
			contents: vec![T::default(); f],
		}
	}

	// Why are these here? It seems like a bad idea
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

	/// [min, max]
	pub fn aabb_extent(&self) -> [[f32;3];2] {
		let max = self.size.map(|v| v as f32);
		let min = [0.0; 3];
		[min, max]
	}

	pub fn run_length_encoding<'a>(&'a self) -> Vec<(&'a T, usize)> {
		let mut runs = Vec::new();
		let mut last = &self.contents[0];
		let mut run_length = 1;
		for item in &self.contents[1..] {
			if item == last {
				run_length += 1;
			} else {
				runs.push((last, run_length));
				last = item;
				run_length = 1;
			}
		}
		runs.push((last, run_length));

		runs
	}
	pub fn run_length_decoding(size: [u32; 3], encoding: &[(impl AsRef<T>, usize)]) -> Self {
		assert_eq!(
			size.iter().fold(1, |a, v| a*v) as usize, 
			encoding.iter().fold(0, |a, &(_, l)| a+l), 
			"Encoding length differs from specified volume size!",
		);

		let mut this = Self::new(size);
		let mut index = 0;
		for (item, length) in encoding {
			for _ in 0..*length {
				this.contents[index] = item.as_ref().clone();
				index += 1;
			}
		}
		this
	}

	/// Generates a mesh of cube-looking things.
	/// Groups these things by a function of their item.
	/// gf(Empty) -> none
	/// gf(Mesh(1)) -> none (do another pass for these)
	/// gf(Block(0)) -> some(0)
	pub fn exposed_faces(
		&self, 
		_grouping_function: impl Fn(&T) -> Option<usize>,
	) -> [Vec<(usize, Vec<[u32; 3]>)>; 6] {
		// Only really needs to record if a voxel is exposed for each direction
		// Can map to vertices after that
		// let mut exposed_faces: [HashMap<_,_>; 6] = (0..6).map(|_| HashMap::new()).collect::<Vec<_>>().try_into().unwrap();

		// let directions = [
		// 	[ 1, 0, 0],
		// 	[-1, 0, 0],
		// 	[ 0, 1, 0],
		// 	[ 0,-1, 0],
		// 	[ 0, 0, 1],
		// 	[ 0, 0,-1],
		// ];
		// let [sx, sy, sz] = self.size;
		// for x in 0..sx {
		// 	for y in 0..sy {
		// 		for z in 0..sz {
		// 			let a = self.get([x,y,z]).unwrap();
		// 			let gfa = grouping_function(a);
		// 			// Check neighbours
		// 			for (i, [dx, dy, dz]) in directions.iter().cloned().enumerate() {
		// 				// If out of bonds then use none as group
		// 				// We still need to test for subtraction overflow though
		// 				let b = self.get([x+dx,y+dy,z+dz]);
		// 				let gfb = b.and_then(|b| Some(grouping_function(b)));
		// 				exposed_faces[i].insert(k, v)
		// 			}
		// 		}
		// 	}
		// }
		

		// exposed_faces.map(|mut hm| hm.drain().collect::<Vec<_>>())
		todo!()
	}
	// Makes (st, end)s for each group for each direction
	pub fn exposed_faces_greedy(
		&self, 
		_grouping_function: impl Fn(&T) -> Option<usize>,
	) -> [Vec<(usize, Vec<[[u32; 3]; 2]>)>; 6] {
		todo!()
	}
}
