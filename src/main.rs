mod window;
mod render;
mod texture;
mod model;
mod camera;
extern crate nalgebra_glm as glm;

fn main() {
	println!("Hello, world!");
	env_logger::init();
	log::warn!("[root] warn");
    log::info!("[root] info");
    log::debug!("[root] debug");

	window::windowmain();

}
