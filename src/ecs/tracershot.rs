use winit::{
	event::*,
};
use crate::ecs::*;
use specs::prelude::*;



pub struct TraceShotSystem;
impl<'a> System<'a> for TraceShotSystem {
	type SystemData = (
		ReadExpect<'a, InputResource>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
		ReadStorage<'a, MapComponent>,
	);

	fn run(
		&mut self, 
		(
			input_resource, 
			cameras,
			transforms, 
			maps
		): Self::SystemData
	) { 
		if input_resource.board_keys.contains_key(&VirtualKeyCode::P) {
			println!("PP!");
			for (_camera, transform) in (&cameras, &transforms).join() {
				for map in (&maps).join() {
					let i = crate::render::rays::map_trace(
						&map.map, 
						transform.position, 
						transform.rotation, 
						800, 600, 90.0,
					);
					crate::util::show_image(i).unwrap();
				}
			}
		}
		
	}
}
