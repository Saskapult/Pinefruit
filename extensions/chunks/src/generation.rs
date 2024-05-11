use std::cmp::Ordering;



#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct KGeneration(u64);
impl KGeneration {
	pub fn new() -> Self {
		Self(0)
	}
	// If generation difference is greater than this we assume a wrap occurred
	// At u64::MAX - 64 we allow 64 generations to be missed before things get buggy
	pub const WRAP_THRESHOLD: u64 = u64::MAX - 64;
	pub fn increment(&mut self) {
		self.0 = self.0.wrapping_add(1);
	}
}
impl Ord for KGeneration {
	fn cmp(&self, other: &Self) -> Ordering {
		let gd = self.0.abs_diff(other.0);
		let o = self.0.cmp(&other.0);
		if gd >= Self::WRAP_THRESHOLD {
			o.reverse()
		} else {
			o
		}
	}
}
impl PartialOrd for KGeneration {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
