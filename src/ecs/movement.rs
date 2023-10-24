use arrayvec::ArrayVec;
use winit::event::VirtualKeyCode;
use crate::{ecs::*, input::KeyKey};
use glam::*;
use eks::prelude::*;



const KEY_FORWARD: KeyKey = KeyKey::BoardKey(VirtualKeyCode::W);
const KEY_BACKWARD: KeyKey = KeyKey::BoardKey(VirtualKeyCode::S);
const KEY_RIGHT: KeyKey = KeyKey::BoardKey(VirtualKeyCode::D);
const KEY_LEFT: KeyKey = KeyKey::BoardKey(VirtualKeyCode::A);
const KEY_UP: KeyKey = KeyKey::BoardKey(VirtualKeyCode::Space);
const KEY_DOWN: KeyKey = KeyKey::BoardKey(VirtualKeyCode::LShift);


#[derive(Debug, ComponentIdent)]
pub struct MovementComponent {
	pub cid_right: ControlKey,
	pub cid_left: ControlKey,
	pub cid_up: ControlKey,
	pub cid_down: ControlKey,
	pub cid_forward: ControlKey,
	pub cid_backward: ControlKey,

	movement_velocity: Vec3,
	pub max_speed: f32,
	pub acceleration: f32,
	pub anti_acceleration: f32,
}
impl MovementComponent {
	pub fn new(control_map: &mut ControlMap) -> Self {
		let cid_right = control_map.new_control(
			"Move Right", 
			"Moves the entity rightward.",
		);
		control_map.add_control_binding(cid_right, KeyCombo {
			modifiers: KeyModifiers::EMPTY,
			keys: ArrayVec::try_from([KEY_RIGHT].as_slice()).unwrap(),
		});
		let cid_left = control_map.new_control(
			"Move Left", 
			"Moves the entity leftward.",
		);
		control_map.add_control_binding(cid_left, KeyCombo {
			modifiers: KeyModifiers::EMPTY,
			keys: ArrayVec::try_from([KEY_LEFT].as_slice()).unwrap(),
		});
		let cid_up = control_map.new_control(
			"Move Up", 
			"Moves the entity upward.",
		);
		control_map.add_control_binding(cid_up, KeyCombo {
			modifiers: KeyModifiers::EMPTY,
			keys: ArrayVec::try_from([KEY_UP].as_slice()).unwrap(),
		});
		let cid_down = control_map.new_control(
			"Move Down", 
			"Moves the entity downward.",
		);
		control_map.add_control_binding(cid_down, KeyCombo {
			modifiers: KeyModifiers::EMPTY,
			keys: ArrayVec::try_from([KEY_DOWN].as_slice()).unwrap(),
		});
		let cid_forward = control_map.new_control(
			"Move Forward", 
			"Moves the entity forward.",
		);
		control_map.add_control_binding(cid_forward, KeyCombo {
			modifiers: KeyModifiers::EMPTY,
			keys: ArrayVec::try_from([KEY_FORWARD].as_slice()).unwrap(),
		});
		let cid_backward = control_map.new_control(
			"Move Back", 
			"Moves the entity backward.",
		);
		control_map.add_control_binding(cid_backward, KeyCombo {
			modifiers: KeyModifiers::EMPTY,
			keys: ArrayVec::try_from([KEY_BACKWARD].as_slice()).unwrap(),
		});

		MovementComponent {
			cid_right, cid_left, cid_up, cid_down, cid_forward, cid_backward, 
			movement_velocity: Vec3::ZERO,
			max_speed: 15.0,
			acceleration: 1.0,
			anti_acceleration: 3.0,
		}
	}

	pub fn with_max_speed(self, max_speed: f32) -> Self {
		Self {
			max_speed,
			..self
		}
	}
	pub fn with_acceleration(self, acceleration: f32) -> Self {
		Self {
			acceleration,
			..self
		}
	}
	pub fn with_anti_acceleration(self, anti_acceleration: f32) -> Self {
		Self {
			anti_acceleration,
			..self
		}
	}
}


// Doesn't do mouse movment anymore
// Please make it do that
pub fn movement_system(
	controls: Comp<ControlComponent>, 
	mut transforms: CompMut<TransformComponent>,
	mut movements: CompMut<MovementComponent>,
) {
	// let apply_duration_secs = (time.this_tick_start - time.last_tick_start).as_secs_f32();
	// println!("ads: {apply_duration_secs}");

	for (control, transform, movement) in (&controls, &mut transforms, &mut movements).iter() {

		let [rx, ry] = control.last_tick_mouse_movement();
		let rx = rx as f32 * 0.001;
		let ry = ry as f32 * 0.001;
		let quat_ry = Quat::from_euler(EulerRot::XYZ, ry, 0.0, 0.0);
		let quat_rx = Quat::from_euler(EulerRot::XYZ, 0.0, rx, 0.0);
		transform.rotation = quat_rx * transform.rotation * quat_ry;

		let mut kpdv = Vec3::ZERO;
		if let Some(kp) = control.last_tick_duration(movement.cid_forward) {
			kpdv.z += kp.as_secs_f32();
		}
		if let Some(kp) = control.last_tick_duration(movement.cid_backward) {
			kpdv.z -= kp.as_secs_f32();
		}
		if let Some(kp) = control.last_tick_duration(movement.cid_right) {
			kpdv.x += kp.as_secs_f32();
		}
		if let Some(kp) = control.last_tick_duration(movement.cid_left) {
			kpdv.x -= kp.as_secs_f32();
		}
		if let Some(kp) = control.last_tick_duration(movement.cid_up) {
			kpdv.y += kp.as_secs_f32();
		}
		if let Some(kp) = control.last_tick_duration(movement.cid_down) {
			kpdv.y -= kp.as_secs_f32();
		}

		transform.translation += transform.rotation * kpdv * movement.max_speed;
	}
}
