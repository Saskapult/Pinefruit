#![feature(drain_filter)]
#![allow(dead_code)]

mod render;
mod window;
mod world;
mod game;


use window::*;
use std::thread;


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

	let event_loop = new_event_loop();
	let event_loop_proxy = event_loop.create_proxy();

	let event_queue = new_queue();

	let game_thread_event_queue = event_queue.clone();
	let game_thread = thread::Builder::new()
		.name("game thread".into())
		.spawn(move || {
			let mut game = game::Game::new(event_loop_proxy, game_thread_event_queue);
			game.setup();
			game.new_window();
			loop {
				use std::{time::{Instant, Duration}, thread::sleep};

				let tick_begin = Instant::now();

				game.tick();

				let tick_duration = Instant::now() - tick_begin;
				sleep(Duration::from_secs_f32(1.0/60.0).saturating_sub(tick_duration));

				// info!("tps: {}", 1.0 / (Instant::now() - tick_begin).as_secs_f32());
				
				// std::thread::sleep(std::time::Duration::from_millis(100));
			}
		})
		.expect("fugg in game thread spawn");

	run_event_loop(event_loop, event_queue);

	game_thread.join().expect("huh?");
}



