#![feature(int_roundings, variant_count, test, div_duration)]
#![allow(dead_code, soft_unstable)]

mod window;
mod util;
mod noise;
mod gui;
mod rays;
mod client;
mod server;

use window::*;
use profiling::puffin;

#[macro_use]
extern crate log;


fn main() {
	env_logger::init();
	info!("Initialized env_logger");

	profiling::register_thread!("Main Thread");
	puffin::set_scopes_on(true);
	info!("Enabled profiling");

	WindowManager::run();
}
