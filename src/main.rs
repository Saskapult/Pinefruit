mod model;
mod render;
mod entity;

mod window;


fn main() {
	println!("Hello, world!");
	env_logger::init();
	log::warn!("[root] warn");
    log::info!("[root] info");
    log::debug!("[root] debug");

	window::windowmain();

}
