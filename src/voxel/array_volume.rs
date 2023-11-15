use glam::UVec3;



#[derive(Clone, PartialEq, Eq)]
pub struct ArrayVolume<V> {
	pub size: UVec3,
	contents: Option<Vec<Option<V>>>,
	contents_count: usize,
}
impl<V: PartialEq + Eq + Clone> ArrayVolume<V> {
	pub fn new(size: UVec3) -> Self {
		Self { 
			size, 
			contents: None, 
			contents_count: 0,
		}
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

	// Creates a vec of None of the correct size
	fn make_data(size: UVec3) -> Vec<Option<V>> {
		let len = size.to_array().iter().copied().reduce(|a, v| a * v).unwrap() as usize;
		let mut contents = Vec::with_capacity(len);
		contents.resize_with(len, || None);
		contents
	}
	
	pub fn get(&self, position: UVec3) -> Option<&V> {
		let i = self.index_of(position).unwrap();
		self.contents.as_ref().and_then(|c| c[i].as_ref())
	}
	
	pub fn insert(&mut self, position: UVec3, data: V) {
		let i = self.index_of(position).unwrap();
		let c = self.contents.get_or_insert_with(|| Self::make_data(self.size));
		c[i] = Some(data);
		self.contents_count += 1;
	}

	pub fn remove(&mut self, position: UVec3) {
		let i = self.index_of(position).unwrap();
		if let Some(c) = self.contents.as_mut() {
			if c[i].take().is_some() {
				self.contents_count -= 1;
				// If no contents remain, deallocate
				if self.contents_count == 0 {
					self.contents.take();
				}
			}
		}
		
	}

	pub fn size(&self) -> usize {
		std::mem::size_of::<Self>() + if self.contents.is_some() {
			self.size.to_array().iter().copied().reduce(|a, v| a * v).unwrap() as usize * std::mem::size_of::<V>()
		} else {
			0
		}
	}

	// Tip: you can compress the result with lz4
	pub fn run_length_encode(&self) -> Vec<(Option<V>, u32)> {
		let mut runs = Vec::new();
		if let Some(c) = self.contents.as_ref() {
			let mut last = c[0].clone();
			let mut len = 1;
			for curr in c[1..].iter() {
				if last.eq(curr) {
					len += 1;
				} else {
					runs.push((last, len));
					last = curr.clone();
					len = 1;
				}
			}
			runs.push((last, len));
		} else {
			runs.push((None, self.size.to_array().iter().copied().reduce(|a, v| a * v).unwrap()));
		}
		
		runs
	}
	
	pub fn run_length_decode(size: UVec3, rle: &Vec<(Option<V>, u32)>) -> Self {
		let mut s = Self::new(size);

		let mut i = 0;
		for (id, length) in rle.iter() {
			for _ in 0..*length {
				if id.is_some() {
					let c = s.contents.get_or_insert_with(|| Self::make_data(size));
					c[i] = id.clone();
					s.contents_count += 1;
				}
				i += 1;
			}
		}
		s
	}
}
