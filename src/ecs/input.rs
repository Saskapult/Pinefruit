use std::{collections::HashMap, time::{Instant, Duration}};
use winit::{
	event::*,
};
use crate::ecs::*;
use nalgebra::*;
use specs::prelude::*;
// use specs::{Component, VecStorage};





// Holds input data
pub struct InputResource {
	// The press percentages for all keys pressed during a timestep
	// It is possible for a percentage to be greater than 100%
	// This happends if startt is after the earliest queue value
	pub board_keys: HashMap<VirtualKeyCode, Duration>,
	pub board_presscache: Vec<VirtualKeyCode>,
	pub mouse_keys: HashMap<MouseButton, Duration>,
	pub mouse_presscache: Vec<MouseButton>,
	pub mx: f64,
	pub my: f64,
	pub mdx: f64,
	pub mdy: f64,
	pub dscrollx: f32,
	pub dscrolly: f32,
	pub last_read: Instant,
	pub last_updated: Instant,
	// controlmap: HashMap<VirtualKeyCode, (some kind of enum option?)>
}
impl InputResource {
	pub fn new() -> Self {
		Self {
			board_keys: HashMap::new(),
			board_presscache: Vec::new(),
			mouse_keys: HashMap::new(),
			mouse_presscache: Vec::new(),
			mx: 0.0,
			my: 0.0,
			mdx: 0.0, 
			mdy: 0.0,
			dscrollx: 0.0,
			dscrolly: 0.0,
			last_read: Instant::now(),
			last_updated: Instant::now(),
		}
	}
}



/// Reads input resource queue and decides what to do with it
pub struct InputSystem;
impl<'a> System<'a> for InputSystem {
	type SystemData = (
		WriteExpect<'a, InputResource>,
		WriteStorage<'a, TransformComponent>,
		WriteStorage<'a, MovementComponent>,
	);

	fn run(
		&mut self, 
		(
			mut input_resource, 
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

		input_resource.last_read = Instant::now();
	}
}
