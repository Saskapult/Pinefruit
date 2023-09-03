use glam::UVec3;



#[derive(Clone, PartialEq, Eq)]
pub struct ArrayVolume<V> {
	pub size: UVec3,
	pub contents: Vec<Option<V>>,
}
impl<V: PartialEq + Eq + Clone> ArrayVolume<V> {
	pub fn new(size: UVec3) -> Self {
		let len = (size.x * size.y * size.z) as usize;
		let mut contents = Vec::with_capacity(len);
		contents.resize_with(len, || None);
		Self { size, contents, }
	}

	pub fn in_bounds(&self, position: UVec3) -> bool {
		position.cmplt(self.size).all()
	}

	fn index_of(&self, position: UVec3) -> Option<usize> {
		self.in_bounds(position).then(|| {
			let [px, py, pz] = position.to_array();
			let [sx, sy, _] = self.size.to_array();
			let i = px * sx * sy
				+ py * sy
				+ pz;
			i as usize
		})
	}
	
	pub fn get(&self, position: UVec3) -> Option<&V> {
		let i = self.index_of(position).unwrap();
		self.contents[i].as_ref()
	}
	
	pub fn insert(&mut self, position: UVec3, data: V) {
		let i = self.index_of(position).unwrap();
		self.contents[i] = Some(data);
	}

	pub fn remove(&mut self, position: UVec3) {
		let i = self.index_of(position).unwrap();
		self.contents[i] = None;
	}

	pub fn size(&self) -> usize {
		self.contents.len() * std::mem::size_of::<V>()
	}

	// Tip: you can compress the result with lz4
	pub fn run_length_encode(&self) -> Vec<(Option<V>, u32)> {
		let mut runs = Vec::new();
		let mut last = self.contents[0].clone();
		let mut len = 1;
		for curr in self.contents[1..].iter() {
			if last.eq(curr) {
				len += 1;
			} else {
				runs.push((last, len));
				last = curr.clone();
				len = 1;
			}
		}
		runs.push((last, len));

		runs
	}
	
	pub fn run_length_decode(mut self, rle: &Vec<(Option<V>, u32)>) -> Self {
		let mut i = 0;
		for (id, length) in rle.iter() {
			for _ in 0..*length {
				self.contents[i] = id.clone();
				i += 1;
			}
		}
		self
	}
}
