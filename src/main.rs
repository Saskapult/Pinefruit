mod geometry;
mod render;
mod window;
mod world;
mod texturemanagers;

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

	window::windowmain();

}
