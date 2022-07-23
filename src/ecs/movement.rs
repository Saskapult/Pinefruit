use winit::{
	event::*,
};
use crate::ecs::*;
use nalgebra::*;
use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::window::WindowInput;




#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct MovementComponent {
	pub speed: f32,	// Units per second
}
impl MovementComponent {
	pub fn new() -> Self {
		MovementComponent {
			speed: 1.0,
		}
	}

	pub fn with_speed(self, speed: f32) -> Self {
		Self {
			speed,
		}
	}
}



pub struct MovementSystem;
impl<'a> System<'a> for MovementSystem {
	type SystemData = (
		ReadExpect<'a, InputResource>,
		WriteStorage<'a, TransformComponent>,
		WriteStorage<'a, MovementComponent>,
	);

	fn run(
		&mut self, 
		(
			input_resource, 
			mut transform, 
			mut movement
		): Self::SystemData
	) { 
		let apply_duration_secs = (input_resource.last_updated - input_resource.last_read).as_secs_f32();

		let rx = input_resource.mdx as f32 * apply_duration_secs * 0.04;
		let ry = input_resource.mdy as f32 * apply_duration_secs * 0.04;

		let mut displacement = Vector3::from_element(0.0);
		for (key, &kp) in &input_resource.board_keys {
			match key {
				VirtualKeyCode::W => {
					displacement.z += kp.as_secs_f32() / apply_duration_secs;
				},
				VirtualKeyCode::S => {
					displacement.z -= kp.as_secs_f32() / apply_duration_secs;
				},
				VirtualKeyCode::D => {
					displacement.x += kp.as_secs_f32() / apply_duration_secs;
				},
				VirtualKeyCode::A => {
					displacement.x -= kp.as_secs_f32() / apply_duration_secs;
				},
				VirtualKeyCode::Space => {
					displacement.y += kp.as_secs_f32() / apply_duration_secs;
				},
				VirtualKeyCode::LShift => {
					displacement.y -= kp.as_secs_f32() / apply_duration_secs;
				},
				_ => {},
			}
		}

		for (transform_c, movement_c) in (&mut transform, &mut movement).join() {

			movement_c.speed = f32::max(movement_c.speed + input_resource.dscrolly * 4.0, 1.0);

			let quat_ry = UnitQuaternion::from_euler_angles(ry, 0.0, 0.0);
			let quat_rx = UnitQuaternion::from_euler_angles(0.0, rx, 0.0);
			transform_c.rotation = quat_rx * transform_c.rotation * quat_ry;

			transform_c.position += transform_c.rotation * displacement * movement_c.speed * apply_duration_secs;
		}
	}
}



pub fn apply_input(input: &WindowInput, transform_c: &mut TransformComponent, movement_c: &MovementComponent) {
	let rx = input.mdx as f32 * 0.001;
	let ry = input.mdy as f32 * 0.001;

	let mut displacement = Vector3::from_element(0.0);
	for (key, &kp) in &input.board_keys {
		match key {
			VirtualKeyCode::W => {
				displacement.z += kp.as_secs_f32();
			},
			VirtualKeyCode::S => {
				displacement.z -= kp.as_secs_f32();
			},
			VirtualKeyCode::D => {
				displacement.x += kp.as_secs_f32();
			},
			VirtualKeyCode::A => {
				displacement.x -= kp.as_secs_f32();
			},
			VirtualKeyCode::Space => {
				displacement.y += kp.as_secs_f32();
			},
			VirtualKeyCode::LShift => {
				displacement.y -= kp.as_secs_f32();
			},
			_ => {},
		}
	}

	// movement_c.speed = f32::max(movement_c.speed + input.dscrolly * 4.0, 1.0);

	let quat_ry = UnitQuaternion::from_euler_angles(ry, 0.0, 0.0);
	let quat_rx = UnitQuaternion::from_euler_angles(0.0, rx, 0.0);
	transform_c.rotation = quat_rx * transform_c.rotation * quat_ry;

	transform_c.position += transform_c.rotation * displacement * movement_c.speed;
	
}
