use serde::Serialize;
use std::ops::Range;

// Needs to go somewhere else but I'm lazy


/// Fits things into a buffer.
/// Addresses each byte using u32, so can hold up to 4GB.
/// Hopefully you will never hold that much!
/// If you do need to have more (you shouldn't) you can just make it address using the slab index.
/// Or just plug in a u64, I'm not your boss.
#[derive(Debug)]
pub struct SlabBuffer {
	slab_size: usize,
	slabs: Vec<bool>,
	pub buffer: wgpu::Buffer,
}
impl SlabBuffer {
	const ENFORCE_ALIGN_FOUR: bool = true;

	pub fn new(device: &wgpu::Device, slab_size: usize, slab_count: usize, usages: wgpu::BufferUsages) -> Self {
		info!("Creating slab buffer with {slab_count} slabs of size {slab_size} ({} bytes)", slab_count * slab_size);

		assert!(slab_size * slab_count < u32::MAX as usize, "Buffer is too large to have bytes addressed by u32!");

		if slab_size % 4 != 0 {
			if Self::ENFORCE_ALIGN_FOUR {
				let message = "Slab size is not a multiple of 4, which is not allowed with current settings";
				error!("{message}");
				panic!("{message}");
			} else {
				warn!("buffer start offsets will not be 4 multiples, be sure to account for this in shaders");
			}
		}
		

		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: None,
			size: slab_size as u64 * slab_count as u64,
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE | usages,
			mapped_at_creation: false,
		});
		Self {
			slab_size,
			slabs: vec![false; slab_count],
			buffer,
		}
	}

	/// Storage space in the buffer.
	pub fn size(&self) -> usize {
		self.slab_size * self.slabs.len()
	}

	/// Remaining size in bytes.
	/// Does NOT imply that things that are more than slab size can be fit into this space.
	pub fn remaining_size(&self) -> usize {
		self.slabs.iter()
			.map(|&filled| if filled {0} else {self.slab_size})
			.fold(0, |a, v| a + v)
	}

	/// Fraction of storage space used.
	pub fn capacity_frac(&self) -> f32 {
		1.0 - self.remaining_size() as f32 / self.size() as f32
	}

	/// Writes to buffer memory, does not change slab status.
	/// Returns false if slab unused or out of memory bounds.
	pub fn write(&mut self, queue: &wgpu::Queue, offset: usize, data: &[u8]) -> bool {
		let slab_index = offset / self.slab_size;
		let out_of_bounds = self.slabs.len() >= slab_index;
		if out_of_bounds {
			error!("Tried to write outside of buffer bounds!");
			return false;
		} else {
			queue.write_buffer(&self.buffer, offset as u64, data);
		}
		let slab_filled = self.slabs[slab_index];
		if !slab_filled {
			warn!("Wrote into inactive slab");
		}
		slab_filled
	}

	/// Returns start and end byte offsets.
	/// Item must be repr(C) to not mess stuff up in shader.
	pub fn insert<T: Serialize>(&mut self, queue: &wgpu::Queue, item: &T) -> [usize; 2] {
		let data = bincode::serialize(item).unwrap();
		self.insert_direct(queue, &data[..])
	}

	pub fn insert_direct(&mut self, queue: &wgpu::Queue, data: &[u8]) -> [usize; 2] {
		let length = data.len();
		let slabs_needed = length.div_ceil(self.slab_size);
		let st = self.select_first_fit(slabs_needed)
			.expect("No allocation fit!");
		let en = st + slabs_needed;
		queue.write_buffer(&self.buffer, st as u64 * self.slab_size as u64, data);
		for i in st..en {
			self.slabs[i] = true;
		}
		[st*self.slab_size, st*self.slab_size + length]
	}

	fn select_first_fit(&self, slabs_needed: usize) -> Option<usize> {
		let mut st = 0;
		let mut free_slabs = 0;
		for (i, slab) in self.slabs.iter().enumerate() {
			if !slab { // Slab free
				if free_slabs == slabs_needed {
					return Some(st)
				}
				free_slabs += 1;
			} else {
				st = i+1;
				free_slabs = 0;
			}
		}
		
		None
	}

	// Untested
	fn select_best_fit(&self, slabs_needed: usize) -> Option<usize> {
		let mut free_ranges = Vec::new();

		let mut st = 0;
		let mut free_slabs = 0;
		for (i, slab) in self.slabs.iter().enumerate() {
			if !slab {
				free_slabs += 1;
			} else {
				free_ranges.push((st, free_slabs));
				st = i+1;
				free_slabs = 0;
			}
		}

		let smallest = free_ranges.iter()
			.filter(|&&(_, l)| l >= slabs_needed)
			.reduce(|accum, item| {
				if item.1 <= accum.1 {
					item
				} else {
					accum
				}
			});

		smallest.and_then(|&(st, _)| Some(st))
	}

	pub fn remove(&mut self, range: Range<usize>) {
		let bytes_st = range.start;
		let bytes_en = range.end;
		let slab_st = bytes_st.div_floor(self.slab_size);
		let slab_en = bytes_en.div_ceil(self.slab_size);

		for i in slab_st..=slab_en {
			self.slabs[i] = false;
		}
	}
}
