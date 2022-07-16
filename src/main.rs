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

	// let mut scm = lua::ScriptManager::new();
	// scm.init_bindings().unwrap();
	// scm.repl_loop().unwrap();

	// let img = image::DynamicImage::ImageRgb8(
	// 	image::ImageBuffer::from_fn(100, 100, |_, _| {
	// 		let u = rand::random::<u8>();
	// 		image::Rgb([u; 3])
	// 	})
	// );
	// crate::util::show_image(img).unwrap();
	
	// return;

	let event_loop = new_event_loop();
	let event_loop_proxy = event_loop.create_proxy();

	let event_queue = new_queue();

	let game_thread_event_queue = event_queue.clone();
	let game_thread = thread::Builder::new()
		.name("game thread".into())
		.spawn(move || {
			let mut game = game::Game::new(event_loop_proxy, game_thread_event_queue);
			// game.setup();
			game.new_window();
			loop {
				game.tick();
			}
		})
		.expect("Failed to spawn game thread!");

	run_event_loop(event_loop, event_queue);

	game_thread.join().expect("huh?");
}



