#![feature(drain_filter, int_roundings, variant_count, int_log, hash_drain_filter)]
#![allow(dead_code)]

mod render;
mod window;
mod world;
mod game;
mod util;
mod mesh;
mod material;
mod texture;
mod ecs;
mod noise;
mod lua;
mod octree;
mod gui;
mod gpu;
mod rays;
mod input;

use window::*;


#[macro_use]
extern crate log;

#[macro_use]
extern crate derivative;




fn main() {
	println!("Hello, world!");
	env_logger::init();

	trace!("some trace log");		// Low priority
    debug!("some debug log");
    info!("some information log");
    warn!("some warning log");
    error!("some error log");		// High priority


	let window_manager = WindowManager::new();

	// let event_loop_proxy = window_manager.event_loop_proxy.clone();
	// let game_thread = thread::Builder::new()
	// 	.name("game thread".into())
	// 	.spawn(move || {
	// 		let mut game = game::Game::new(
	// 			event_loop_proxy,
	// 		);
	// 		game.setup();
	// 		game.new_window();
	// 		loop {
	// 			game.tick();
	// 		}
	// 	})
	// 	.expect("Failed to spawn game thread!");

	
	window_manager.run();
	// game_thread.join().expect("huh?");
}



