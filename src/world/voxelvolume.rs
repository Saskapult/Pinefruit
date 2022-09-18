

/*
We would ideally store attributes in bits.
I am too stupid to make that work.
We store attributes in bytes.
*/

use std::collections::HashMap;


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

		// let [xs, ys, zs] = self.size;

		// fn adda3<T: std::ops::Add<Output=T> + Copy>(a: [T; 3], b: [T; 3]) -> [T; 3] {
		// 	[a[0]+b[0], a[1]+b[1], a[2]+b[2]]
		// }

		// let directions = [
		// 	[ 1, 0, 0 ],
		// 	[ 0, 1, 0 ],
		// 	[ 0, 0, 1 ],
		// 	[-1, 0, 0 ],
		// 	[ 0,-1, 0 ],
		// 	[ 0, 0,-1 ],
		// ];

		
		// for direction in 0..3 {
		// 	let mut this_direction: HashMap<usize, Vec<[[u32; 3]; 2]>> = HashMap::new();
		// 	let kd = directions[(direction+0) % 3];
		// 	let id = directions[(direction+1) % 3];
		// 	let jd = directions[(direction+2) % 3];
		// 	let is = self.size[(direction+1) % 3];
		// 	let js = self.size[(direction+2) % 3];
		// 	let mut used = vec![false; (is * js) as usize];

		// 	for x in 0..xs as i32 {
		// 		for y in 0..ys as i32 {
		// 			for z in 0..zs as i32 {
		// 				let v = self.get([x, y, z].map(|v| v as u32)).unwrap();
		// 				// If has some group
		// 				if let Some(group) = grouping_function(v) {
		// 					// Check if not blocked by other
		// 					if let Some(v2) = self.get(adda3([x,y,z], kd).map(|v| v as u32)) {
		// 						if grouping_function(v2).is_some() {
		// 							continue
		// 						}
		// 					}
		// 					let st = [x, y, z];
		// 					let mut end = [x, y, z];
		// 					loop {

		// 					}

		// 				}
		// 			}
		// 		}

		// 		// Reset used
		// 		used.iter_mut().for_each(|v| *v = false);
		// 	}
		// }

		todo!()
	}
}



fn indexof(position: [u32; 2], scales: [u32; 2]) -> Option<usize> {
	if position[0] >= scales[0] || position[1] >= scales[1] {
		None
	} else {
		Some((position[0] * scales[1] + position[1]) as usize)
	}
}



#[test]
fn test_greedy2d() {
	let extent = [5, 5];
	let slice = vec![
		0, 0, 0, 0, 0,
		1, 1, 0, 0, 0,
		1, 1, 0, 2, 0,
		0, 0, 0, 2, 0,
		1, 1, 0, 2, 0,
	];

	for row in slice.chunks(extent[0]).map(|r| r.iter().map(|&v| format!("{v}")).collect::<Vec<_>>().join(", ")) {
		println!("{row}");
	}

	// Starting positions
	let [xs, ys] = extent.map(|v| v as u32);
	let mut used = vec![false; (xs * ys) as usize];
	let mut results: HashMap<usize, Vec<[[u32; 2]; 2]>> = HashMap::new();
	for x in 0..xs {
		for y in 0..ys {
			// Used things can't be start position
			if used[indexof([x, y], [xs, ys]).unwrap()] {
				continue
			}
			let group = slice[indexof([x, y], [xs, ys]).unwrap()];
			let st = [x, y];
			println!("st = {st:?}");
			let mut end_x = x;
			loop {
				// Used or out of bounds
				if indexof([end_x+1, y], [xs, ys]).and_then(|i| Some(used[i])).unwrap_or(true) {
					println!("used or out of bounds");
					break
				}
				// Different group or out of bounds
				if indexof([end_x+1, y], [xs, ys]).and_then(|i| Some(slice[i] != group)).unwrap_or(true) {
					let group = indexof([end_x+1, y], [xs, ys]).and_then(|i| Some(slice[i])).unwrap();
					println!("different group ({group})");
					break
				}
				// Increment
				end_x += 1;
				used[indexof([end_x, y], [xs, ys]).unwrap()] = true;
				println!("ex++ ({end_x})");
			}
			let mut end_y = y;
			loop {
				let mut should_break = false;
				for inner_x in x..=end_x {
					// Used or out of bounds
					if indexof([inner_x, end_y+1], [xs, ys]).and_then(|i| Some(used[i])).unwrap_or(true) {
						should_break = true;
						println!("used or out of bounds");
						break
					}
					// Different group or out of bounds
					if indexof([inner_x, end_y+1], [xs, ys]).and_then(|i| Some(slice[i] != group)).unwrap_or(true) {
						should_break = true;
						println!("different group");
						break
					}
				}
				if should_break { break }
				end_y += 1;
				for inner_x in x..=end_x {
					used[indexof([inner_x, end_y], [xs, ys]).unwrap()] = true;
				}
				println!("ey++ ({end_y})");
				// std::thread::sleep(std::time::Duration::from_secs_f32(0.5));
			}
			let en = [end_x, end_y];

			// Append segment
			if let Some(v) = results.get_mut(&group) {
				v.push([st, en]);
			} else {
				results.insert(group, vec![[st, en]]);
			}
		}
	}
	println!("{results:#?}");

	let mut reconstructed = vec![None; extent.iter().fold(1, |a, &v| a * v as usize)];

	for (&k, v) in results.iter() {
		for &[[sx, sy], [ex, ey]] in v {
			for x in sx..=ex {
				for y in sy..=ey {
					let g = &mut reconstructed[indexof([x, y], [xs, ys]).unwrap()];
					if g.is_some() {
						panic!("{:?} is already {g:?}!", [x, y]);
					}
					*g = Some(k);
				}
			}
		}
	}

	for row in reconstructed.chunks(extent[0]).map(|r| r.iter().map(|&v| v.and_then(|v| Some(format!("{v}"))).unwrap_or("#".to_string())).collect::<Vec<_>>().join(", ")) {
		println!("{row}");
	}

	let reconstructed = reconstructed.iter().map(|v| v.unwrap()).collect::<Vec<_>>();

	assert_eq!(slice, reconstructed);

}

