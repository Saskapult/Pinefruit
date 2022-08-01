use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU32;
use std::sync::{RwLock, Arc};
use std::time::{Duration, Instant};
use winit::event::*;



// Maps can be replaced with arrays of all possible values?
#[derive(Debug)]
pub struct Input {
	pub board_key_durations: HashMap<VirtualKeyCode, Duration>, // Duration pressed since last read
	pub board_key_presses: HashMap<VirtualKeyCode, u8>, // How many times pressed since last read
	pub board_pressed: HashMap<VirtualKeyCode, Instant>, // What is pressed and since when
	pub mouse_key_durations: HashMap<MouseButton, Duration>,
	pub mouse_key_presses: HashMap<MouseButton, u8>,
	pub mouse_pressed: HashMap<MouseButton, Instant>,
	pub mx: f64,
	pub my: f64,
	pub mdx: f64,
	pub mdy: f64,
	pub dscrollx: f32,
	pub dscrolly: f32,
	pub last_read: Instant, // When was this data last used
	pub last_feed: Instant,	// When was new data last fed in
}
impl Input {
	pub fn new() -> Self {
		Self {
			board_key_durations: HashMap::new(),
			board_key_presses: HashMap::new(),
			board_pressed: HashMap::new(),
			mouse_key_durations: HashMap::new(),
			mouse_key_presses: HashMap::new(),
			mouse_pressed: HashMap::new(),
			mx: 0.0,
			my: 0.0,
			mdx: 0.0, 
			mdy: 0.0,
			dscrollx: 0.0,
			dscrolly: 0.0,
			last_read: Instant::now(),
			last_feed: Instant::now(),
		}
	}

	/// Collects all key durations without unpressing keys
	pub fn end(&mut self) -> Duration {
		let now = self.last_feed;

		self.board_pressed.iter_mut().for_each(|(key, pressed)| {
			let kp = now - *pressed;
			*pressed = now;
			if let Some(dur) = self.board_key_durations.get_mut(key) {
				*dur += kp;
			} else {
				self.board_key_durations.insert(*key, kp);
			}
		});

		self.mouse_pressed.iter_mut().for_each(|(key, pressed)| {
			let kp = now - *pressed;
			*pressed = now;
			if let Some(dur) = self.mouse_key_durations.get_mut(key) {
				*dur += kp;
			} else {
				self.mouse_key_durations.insert(*key, kp);
			}
		});

		let dur = self.last_feed - self.last_read;
		self.last_read = Instant::now();

		dur
	}

	/// Clears duration (and other) data but not pressed keys
	pub fn reset(&mut self) {
		self.board_key_durations.clear();
		self.mouse_key_durations.clear();

		self.mdx = 0.0;
		self.mdy = 0.0;

		self.dscrollx = 0.0;
		self.dscrolly = 0.0;
	}
}


#[derive(Debug)]
pub struct UiInput {
	pub board_keys: HashMap<VirtualKeyCode, u8>, // How many times pressed since last read
	pub board_pressed: HashSet<VirtualKeyCode>, // What is pressed and since when
	pub mouse_keys: HashMap<MouseButton, u8>,
	pub mouse_pressed: HashSet<MouseButton>,
	pub mx: f64,
	pub my: f64,
	pub mdx: f64,
	pub mdy: f64,
	pub dscrollx: f32,
	pub dscrolly: f32,
	pub last_read: Instant, // When was this data last used
	pub last_feed: Instant,	// When was new data last fed in
}


use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};

fn mainy() {

	// spawn game thread
	let (input_sender, input_receiver) = channel();

	thread::Builder::new()
		.name("game thread".into())
		.spawn(move || {
			let mut game = Game::new();
			loop {
				game.tick();
			}
		}).unwrap();

	loop {
		// Send input
		input_sender.send(0_u32).unwrap()
	}

}


struct WindowManager {
	
	world_gen: u32,
	world_update: Arc<WorldUpdateThing>,
}
impl WindowManager {
	pub fn new(
		world_update: Arc<WorldUpdateThing>,
	) -> Self {
		Self {
			world_gen: 0,
			world_update,
		}
	}

	pub fn tick(&mut self) {
		// collect input

		if self.world_gen > self.world_update.gen.load()

		// For each window:
		// update ui
	}
}


struct WorldUpdateThing {
	pub gen: AtomicU32,
	pub update: RwLock<u32>,
}
impl WorldUpdateThing {
	pub fn new() -> Self {
		todo!()
	}
}



struct Game {
	input_receiver: Receiver<u32>,
	world_update: Arc<WorldUpdateThing>,
}
impl Game {
	pub fn new(
		// input receiver
	) -> Self {

		let (input_sender, input_receiver) = channel();
		let world_update = Arc::new(WorldUpdateThing::new());

		let wuc = world_update.clone();
		thread::Builder::new()
			.name("window thread".into())
			.spawn(move || {
				let mut window_manager = WindowManager::new(
					wuc,
				);
				loop {
					window_manager.tick();
				}
			}).unwrap();

		Game {
			input_receiver,
			world_update,
		}
	}

	pub fn tick(&mut self) {


		// Get window input
		// Run systems

		
	}
}