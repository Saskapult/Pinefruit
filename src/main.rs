#![feature(int_roundings, variant_count, test, div_duration)]
#![allow(dead_code, soft_unstable)]

mod window;
mod voxel;
mod game;
mod util;
mod ecs;
mod noise;
mod gui;
mod rays;
mod input;
mod rendering_integration;
mod client;
mod server;

use window::*;
use winit::event_loop::EventLoopBuilder;
use profiling::puffin;

#[macro_use]
extern crate log;


fn main() {
	env_logger::init();
	info!("Initialized env_logger");

	profiling::register_thread!("Main Thread");
	puffin::set_scopes_on(true);
	info!("Enabled profiling");

	let event_loop = EventLoopBuilder::<WindowCommand>::with_user_event().build().unwrap();
	let window_manager = WindowManager::new(&event_loop);
	window_manager.run(event_loop);
}
