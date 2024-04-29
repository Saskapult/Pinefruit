use std::{sync::Arc, thread::JoinHandle, time::{Duration, Instant}};
use parking_lot::RwLock;
use crate::client::Client;


struct TickThing {
	tick: u32,
	offset: u32,
	start: Instant, // Since offset tick
	last: Option<Instant>,
	tick_period: Duration,
}
impl TickThing {
	pub fn new(tick_period: Duration) -> Self {
		Self {
			tick: 0,
			offset: 0,
			start: Instant::now(),
			last: None,
			tick_period,
		}
	}

	pub fn set_period(&mut self, tick_period: Duration) {
		self.offset = self.tick;
		self.tick = 0;
		self.tick_period = tick_period;
		if let Some(last) = self.last {
			self.start = last;
		}
	}

	pub fn tick_to_time(&mut self, now: Instant, mut f: impl FnMut()) {
		let target_tick = self.offset + now.duration_since(self.start).div_duration_f32(self.tick_period).floor() as u32;
		let n = target_tick-self.tick;
		debug!("Need to do {} ticks ({} -> {})", n, self.tick, target_tick);
		for i in 0..n {
			debug!("Tick {}", self.tick + i);
			f();
		}
		self.tick += n;
		debug!("Now at tick {}", self.tick);
	}

	pub fn duration_to_next(&self) -> Duration {
		let now = Instant::now();
		self.last
			.and_then(|last| Some(now.duration_since(last)))
			.and_then(|d| Some(self.tick_period.saturating_sub(d)))
			.unwrap_or(Duration::ZERO)
	}
}


/// A message sent from the WindowManager to the Server thread. 
/// 
/// Currently it only provides a way to shut down the server. 
/// There will be other was to shut down the server, like emitting a signal from the ECS world. 
/// So don't worry about that. 
pub enum ServerCommand {
	ShutDown,
}


pub fn run_server(
	server: Arc<RwLock<Client>>, // TODO: replace with server?
) -> (crossbeam_channel::Sender<ServerCommand>, JoinHandle<anyhow::Result<()>>) {
	let (s, r) = crossbeam_channel::unbounded();

	let h = std::thread::spawn(move || {
		profiling::register_thread!("server thread");

		let mut tick_thing = TickThing::new(Duration::from_secs_f32(1.0 / 30.0));
		let mut exit = false;
		while !exit {
			std::thread::sleep(tick_thing.duration_to_next());
			for c in r.try_iter() {
				match c {
					ServerCommand::ShutDown => exit = true,
				}
			}
			{
				let mut server = server.write();
				tick_thing.tick_to_time(Instant::now(), || server.tick());
			}
		}
		Ok(())
	});

	(s, h)
}
