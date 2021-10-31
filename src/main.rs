mod window;
mod render;
mod texture;
mod model;
mod camera;

fn main() {
	println!("Hello, world!");
	env_logger::init();

	window::windowmain();

}
