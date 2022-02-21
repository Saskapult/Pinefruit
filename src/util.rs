use std::time::Duration;



/// Holds durations, can find average and median
#[derive(Debug)]
pub struct DurationHolder {
	num_to_hold: usize,
	durations: Vec<Duration>,
	durations_index: usize,
}
impl DurationHolder {
	pub fn new(num_to_hold: usize) -> Self {
		Self {
			num_to_hold,
			durations: Vec::with_capacity(num_to_hold),
			durations_index: num_to_hold-1,
		}
	}

	pub fn reset(&mut self) {
		self.durations = Vec::with_capacity(self.num_to_hold);
		self.durations_index = 0;
	}

	pub fn resize(&mut self, new_size: usize) {
		if new_size - self.num_to_hold > 0 {
			self.durations.reserve(new_size - self.num_to_hold);
		}
		if self.durations_index >= new_size {
			self.durations_index = 0;
		}
		self.num_to_hold = new_size;
	}

	pub fn record(&mut self, duration: Duration) {
		self.durations_index = (self.durations_index + 1) % self.num_to_hold;
		if self.durations_index < self.durations.len() {
			self.durations[self.durations_index] = duration;
		} else {
			self.durations.push(duration);
		}
	}

	pub fn latest(&self) -> Option<Duration> {
		if self.durations.len() == 0 {
			None
		} else {
			Some(self.durations[self.durations_index])
		}
	}

	pub fn average(&self) -> Option<Duration> {
		if self.durations.len() == 0 {
			None
		} else {
			Some(self.durations.iter().sum::<Duration>() / (self.durations.len() as u32))
		}
	}

	pub fn median(&self) -> Option<Duration> {
		if self.durations.len() == 0 {
			None
		} else {
			let mut sorted_durations = self.durations.clone();
			sorted_durations.sort_unstable();

			if sorted_durations.len() % 2 == 0 {
				Some((sorted_durations[sorted_durations.len()/2] + sorted_durations[sorted_durations.len()/2+1]) / 2)
			} else {
				Some(sorted_durations[sorted_durations.len()/2])
			}
		}
		
	}
}


// pub trait RapierConvertable<T> {
// 	fn to(input: T) -> T;
// 	fn from(input: T) -> T;
// }

// impl<V> RapierConvertable<V> for [V; 3] {
// 	fn to(mut input: [V; 3]) -> [V; 3] {
// 		input
// 	}
// 	fn from(mut input: [V; 3]) -> [V; 3] {
// 		input
// 	}
// }

// Rapier uses a y-up coordinate system
/// Switches y for z
pub fn k_tofrom_rapier(mut input: nalgebra::Vector3<f32>) -> nalgebra::Vector3<f32> {
	let y = input[1];
	input[1] = input[2];
	input[2] = y;
	input
}

