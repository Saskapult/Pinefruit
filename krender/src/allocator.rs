//! This doesn't *need* to be in krender, but I think it's more useful here. 
//! Allocators can be used with buffers to do allocation stuff. 
//! I think it's quite neat. 

pub trait BufferAllocator {
	type Key;
	fn alloc(&mut self, size: u64) -> Option<Self::Key>;
	fn free(&mut self, allocation: Self::Key);
	// fn size(&self) -> u64;
	// fn used(&self) -> u64;
	// fn free(&self) -> u64;
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct SlabAllocationKey {
	// These are byte addresses 
	// They will always be a multiple of slab size
	// It makes the struct bigger but my life easier
	pub start: u64, 
	pub end: u64,
}
impl SlabAllocationKey {
	pub fn size(&self) -> u64 {
		self.end - self.start
	}
}


/// A buffer that can reuse data segments!
/// 
/// Addresses each byte using u32, so can hold up to 4GiB.
/// Hopefully you will never hold that much!
/// If you do need to have more (you shouldn't) you can just make it address 
/// using u64 and hold up to 17179869184GiB.
/// If that's not enough for you then I cannot help.
/// 
/// One potential improvement is to shift to world addressing. 
/// This would allow us to store 16GiB of data with u32 addresses.
/// It would also make it harder to write data in smaller sizes.
/// For now, however, I think I do not need to do that.
#[derive(Debug)]
pub struct SlabBufferAllocator {
	slab_size: u32,
	slabs: Vec<bool>, // Could be more clever, but I just don't want to
}
impl SlabBufferAllocator {
	pub fn new(
		slab_size: u32, 
		slab_count: u32,
		tetrabyte_aligned: bool,
	) -> Self {
		let size = slab_size as u64 * slab_count as u64;
		assert!(size < u32::MAX as u64, "Buffer is too large to have bytes addressed by u32!");

		if slab_size % 4 != 0 {
			if tetrabyte_aligned {
				let message = "Slab size is not a multiple of 4, which is not allowed with current settings";
				error!("{message}");
				panic!("{message}");
			} else {
				warn!("buffer start offsets will not be 4 multiples, be sure to account for this in shaders");
			}
		}

		Self {
			slab_size,
			slabs: vec![false; slab_count as usize],
		}
	}

	fn select_first_fit(&self, slabs_needed: u64) -> Option<u64> {
		let mut st = 0;
		let mut free_slabs = 0;
		for (i, slab) in self.slabs.iter().enumerate() {
			if !slab { // Slab free
				free_slabs += 1;
				if free_slabs == slabs_needed {
					return Some(st)
				}
			} else {
				st = i as u64 + 1;
				free_slabs = 0;
			}
		}
		None
	}

	// Untested
	fn _select_best_fit(&self, slabs_needed: usize) -> Option<usize> {
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
}
impl BufferAllocator for SlabBufferAllocator {
	type Key = SlabAllocationKey;
	
	fn alloc(&mut self, size: u64) -> Option<Self::Key> {
		let slabs_needed = size.div_ceil(self.slab_size as u64);
		let start_slab = self.select_first_fit(slabs_needed)?;
		let end_slab = start_slab + slabs_needed;
		if end_slab as usize > self.slabs.len() {
			return None;
		}
		for i in start_slab..end_slab {
			self.slabs[i as usize] = true;
		}
		Some(SlabAllocationKey {
			start: start_slab * self.slab_size as u64, 
			end: end_slab * self.slab_size as u64, 
		})
	}

	fn free(&mut self, allocation: Self::Key) {
		let start_slab = allocation.start / self.slab_size as u64;
		let end_slab = allocation.end / self.slab_size as u64;
		for i in start_slab..end_slab {
			self.slabs[i as usize] = false;
		}
	}
}

