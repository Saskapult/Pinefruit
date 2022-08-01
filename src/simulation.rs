use std::time::Instant;

use specs::prelude::*;
use crate::ecs::*;


struct ReplicationComponent {
	id: u32,
}


pub struct Simulation {
	world: World,
	last_tick: Instant,
}
impl Simulation {


	pub fn render(&mut self, when: Instant) {

		let dt = when - self.last_tick; // 0 if after, maybe not ideal

	}
}

