#![feature(drain_filter)]
#![allow(dead_code)]
#![feature(int_log)]

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


use window::*;
use std::thread;
use std::sync::mpsc::channel;
use winit::{
	event::*,
	event_loop::*,
	window::*,
};


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

	let (game_sender, game_receiver) = channel();

	let event_loop = EventLoop::<EventLoopEvent>::with_user_event();
	let event_loop_proxy = event_loop.create_proxy();

	let game_thread = thread::Builder::new()
		.name("game thread".into())
		.spawn(move || {
			let mut game = game::Game::new(event_loop_proxy, game_receiver);
			game.setup();
			game.new_window();
			loop {
				game.tick();
			}
		})
		.expect("Failed to spawn game thread!");

	run_event_loop(event_loop, game_sender);

	game_thread.join().expect("huh?");
}



