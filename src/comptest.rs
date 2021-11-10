use specs::prelude::*;


struct TransformComponent {
	location: Vector3<f32>,
	rotation: UnitQuaternion<f32>,
	scale: Vector3<f32>,
}
impl Component for TransformComponent {
	// All have isometry
	type Storage = VecStorage<Self>;
}


struct InputComponent {
	// Map of all keys?
}
impl Component for InputComponent {
	// HashMapStorage is better for components that are met rarely
	type Storage = HashMapStorage<Self>;
}



struct InputSystem;
impl<'a> System<'a> for InputSystem {
	type SystemData = (
		WriteStorage<'a, InputComponent>,
	);

	fn run(&mut self, mut data: Self::SystemData) {
		for ic in (&data).join() {
			println!("Help!");
		}
	}
}

fn fun() {
	let mut m = world::new();
	let mut dispatcher_builder = DispatcherBuilder::new()
		.with(InputSystem, "print_bool", &[]);

	dispatcher.setup(&mut world);

	world.create_entity().with(InputComponent{})
}



// Loop of renderable and isometry


