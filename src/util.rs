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
			durations_index: 0,
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
		if self.durations.len() == 0 {
			self.durations.push(duration);
			self.durations_index += 1;
		} else {
			self.durations_index = (self.durations_index + 1) % self.num_to_hold;
			if self.durations_index < self.durations.len() {
				self.durations[self.durations_index] = duration;
			} else {
				self.durations.push(duration);
			}
		}
	}

	pub fn average(&self) -> Duration {
		self.durations.iter().sum::<Duration>() / (self.durations.len() as u32)
	}

	pub fn median(&self) -> Duration {
		let mut sorted_durations = self.durations.clone();
		sorted_durations.sort_unstable();

		if sorted_durations.len() % 2 == 0 {
			(sorted_durations[sorted_durations.len()/2] + sorted_durations[sorted_durations.len()/2+1]) / 2
		} else {
			sorted_durations[sorted_durations.len()/2]
		}
	}
}
