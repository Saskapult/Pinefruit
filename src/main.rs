mod geometry;
mod texture;
mod texturemanagers;
mod render;
mod window;
mod world;
mod game;

use window::*;
use std::thread;
use std::sync::{Arc, Mutex};


#[macro_use]
extern crate log;




fn main() {
	println!("Hello, world!");
	env_logger::init();
	trace!("some trace log");		// Low priority
    debug!("some debug log");
    info!("some information log");
    warn!("some warning log");
    error!("some error log");		// High priority

	let event_loop = new_event_loop();
	let event_loop_proxy = event_loop.create_proxy();

	let event_queue = new_queue();

	let game_thread_event_queue = event_queue.clone();
	let game_thread = thread::spawn(move || {
		let mut game = game::Game::new(event_loop_proxy, game_thread_event_queue);
		game.new_window();
		loop {
			game.simulation_tick();
			//std::thread::sleep(std::time::Duration::from_millis(10));
		}
	});

	run_event_loop(event_loop, event_queue);
}



