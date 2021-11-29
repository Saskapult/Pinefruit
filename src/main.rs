#![feature(drain_filter)]

mod geometry;
mod texture;
mod resource_manager;
mod render;
mod window;
mod world;
mod game;

use window::*;
use std::thread;


#[macro_use]
extern crate log;

// extern crate nalgebra_glm as glm;




fn main() {
	println!("Hello, world!");
	env_logger::init();
	trace!("some trace log");		// Low priority
    debug!("some debug log");
    info!("some information log");
    warn!("some warning log");
    error!("some error log");		// High priority

	// let aspect = 1.0;
	// let fovy = 45.0;
	// let near = 0.1;
	// let far = 100.0;

	// let lh = glm::perspective_lh(aspect, fovy, near, far);
	// let rh = glm::perspective_rh(aspect, fovy, near, far);
	// let dh = nalgebra::Matrix4::new_perspective(aspect, fovy, near, far);
	
	// println!("lh: {}", &lh);
	// println!("rh: {}", &rh);
	// println!("dh: {}", &dh);

	// return;

	let event_loop = new_event_loop();
	let event_loop_proxy = event_loop.create_proxy();

	let event_queue = new_queue();

	let game_thread_event_queue = event_queue.clone();
	let game_thread = thread::Builder::new()
		.name("game thread".into())
		.spawn(move || {
			let mut game = game::Game::new(event_loop_proxy, game_thread_event_queue);
			game.new_window();
			loop {
				game.tick();
				// Sleep half second
				std::thread::sleep(std::time::Duration::from_millis(500));
			}
		})
		.expect("fugg in game thread spawn");

	run_event_loop(event_loop, event_queue);

	game_thread.join().expect("huh?");
}



