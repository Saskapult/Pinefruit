use std::time::{Duration, Instant};


struct TickThing {
	tick: u32,
	offset: u32,
	start: Instant, // Since offset tick
	tick_period: Duration,
}
impl TickThing {
	pub fn tick_to_time(&mut self, now: Instant) {
		let target_tick = self.offset + now.duration_since(self.start).div_duration_f32(self.tick_period).floor() as u32;
		let n = target_tick-self.tick;
		debug!("Need to do {} ticks ({} -> {})", n, self.tick, target_tick);
		for i in 0..n {
			debug!("Tick {}", self.tick + i);
		}
		self.tick += n;
		debug!("Now at tick {}", self.tick);
	}
}
