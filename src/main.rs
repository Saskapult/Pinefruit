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

#[macro_use]
extern crate log;


fn main() {
	env_logger::init();
	profiling::register_thread!("Main Thread");
	
	WindowManager::run();
}
