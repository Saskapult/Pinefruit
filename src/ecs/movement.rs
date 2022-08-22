use winit::{
	event::*,
};
use crate::{ecs::*, input::KeyKey};
use nalgebra::*;
use shipyard::*;
use crate::input::*;



const KEY_FORWARD: KeyKey = KeyKey::BoardKey(VirtualKeyCode::W);
const KEY_BACKWARD: KeyKey = KeyKey::BoardKey(VirtualKeyCode::S);
const KEY_RIGHT: KeyKey = KeyKey::BoardKey(VirtualKeyCode::D);
const KEY_LEFT: KeyKey = KeyKey::BoardKey(VirtualKeyCode::A);
const KEY_UP: KeyKey = KeyKey::BoardKey(VirtualKeyCode::Space);
const KEY_DOWN: KeyKey = KeyKey::BoardKey(VirtualKeyCode::LShift);

const COMBO_FORWARD: KeyCombo = KeyCombo {
	modifiers: KeyModifiers::EMPTY,
	key: KEY_FORWARD,
};
const COMBO_BACKWARD: KeyCombo = KeyCombo {
	modifiers: KeyModifiers::EMPTY,
	key: KEY_BACKWARD,
};
const COMBO_RIGHT: KeyCombo = KeyCombo {
	modifiers: KeyModifiers::EMPTY,
	key: KEY_RIGHT,
};
const COMBO_LEFT: KeyCombo = KeyCombo {
	modifiers: KeyModifiers::EMPTY,
	key: KEY_LEFT,
};
const COMBO_UP: KeyCombo = KeyCombo {
	modifiers: KeyModifiers::EMPTY,
	key: KEY_UP,
};
const COMBO_DOWN: KeyCombo = KeyCombo {
	modifiers: KeyModifiers::EMPTY,
	key: KEY_DOWN,
};



#[derive(Debug, Component)]
pub struct MovementComponent {
	pub cid_right: ControlId,
	pub cid_left: ControlId,
	pub cid_up: ControlId,
	pub cid_down: ControlId,
	pub cid_forward: ControlId,
	pub cid_backward: ControlId,
	movement_velocity: Vector3<f32>,
	pub max_speed: f32,
	pub acceleration: f32,
	pub anti_acceleration: f32,
}
impl MovementComponent {
	pub fn new(control_map: &mut ControlMap) -> Self {
		let cid_right = control_map.new_cid(
			"Move Right", 
			"Moves the entity rightward.",
		);
		control_map.add_cid_key(cid_right, COMBO_RIGHT);
		let cid_left = control_map.new_cid(
			"Move Left", 
			"Moves the entity leftward.",
		);
		control_map.add_cid_key(cid_left, COMBO_LEFT);
		let cid_up = control_map.new_cid(
			"Move Up", 
			"Moves the entity upward.",
		);
		control_map.add_cid_key(cid_up, COMBO_UP);
		let cid_down = control_map.new_cid(
			"Move Down", 
			"Moves the entity downward.",
		);
		control_map.add_cid_key(cid_down, COMBO_DOWN);
		let cid_forward = control_map.new_cid(
			"Move Forward", 
			"Moves the entity forward.",
		);
		control_map.add_cid_key(cid_forward, COMBO_FORWARD);
		let cid_backward = control_map.new_cid(
			"Move Back", 
			"Moves the entity backward.",
		);
		control_map.add_cid_key(cid_backward, COMBO_BACKWARD);

		MovementComponent {
			cid_right, cid_left, cid_up, cid_down, cid_forward, cid_backward, 
			movement_velocity: Vector3::new(0.0, 0.0, 0.0),
			max_speed: 10.0,
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



pub fn movement_system(
	time: UniqueView<TimeResource>,
	mice: View<MouseComponent>, 
	keys: View<KeysComponent>, 
	mut transforms: ViewMut<TransformComponent>,
	mut movements: ViewMut<MovementComponent>,
) {
	let apply_duration_secs = (time.this_tick_start - time.last_tick_start).as_secs_f32();
	// println!("ads: {apply_duration_secs}");

	for (mouse, key, transform, movement) in (&mice, &keys, &mut transforms, &mut movements).iter() {

		let rx = mouse.data.total_movement[0] as f32 * 0.001;
		let ry = mouse.data.total_movement[1] as f32 * 0.001;
		let quat_ry = UnitQuaternion::from_euler_angles(ry, 0.0, 0.0);
		let quat_rx = UnitQuaternion::from_euler_angles(0.0, rx, 0.0);
		transform.rotation = quat_rx * transform.rotation * quat_ry;

		let mut kpdv = Vector3::from_element(0.0);
		let mut n_keys = 0.0;
		if let Some(kp) = key.data.key_duration(KEY_FORWARD) {
			kpdv.z += kp.as_secs_f32();
			n_keys += 1.0;
		}
		if let Some(kp) = key.data.key_duration(KEY_BACKWARD) {
			kpdv.z -= kp.as_secs_f32();
			n_keys += 1.0;
		}
		if let Some(kp) = key.data.key_duration(KEY_RIGHT) {
			kpdv.x += kp.as_secs_f32();
			n_keys += 1.0;
		}
		if let Some(kp) = key.data.key_duration(KEY_LEFT) {
			kpdv.x -= kp.as_secs_f32();
			n_keys += 1.0;
		}
		if let Some(kp) = key.data.key_duration(KEY_UP) {
			kpdv.y += kp.as_secs_f32();
			n_keys += 1.0;
		}
		if let Some(kp) = key.data.key_duration(KEY_DOWN) {
			kpdv.y -= kp.as_secs_f32();
			n_keys += 1.0;
		}
		if n_keys > 0.01 {
			kpdv /= n_keys;
		}
		// println!("kpdv = {kpdv:?}");
		let added_velocity = transform.rotation * kpdv * movement.acceleration;
		// println!("added_velocity = {added_velocity:?}");

		// Apply damping opposite to old movement
		let signs_before = (0..3).map(|i| movement.movement_velocity[i] >= 0.0).collect::<Vec<_>>();
		println!("nmv = {}", movement.movement_velocity.normalize());
		let anti_velocity = movement.movement_velocity.normalize() * movement.anti_acceleration * apply_duration_secs;
		println!("anti_velocity = {anti_velocity}");
		let mut damped_velocity = movement.movement_velocity - anti_velocity;
		let signs_after = (0..3).map(|i| damped_velocity[i] >= 0.0).collect::<Vec<_>>();
		// If any elements changed signs due to damping set them to zero
		signs_before.iter().zip(signs_after.iter()).enumerate().for_each(|(i, (b, a))| {
			if b != a {
				damped_velocity[i] = 0.0;
			}
		});
		
		// Cap speed
		let final_velocity = (damped_velocity + added_velocity).cap_magnitude(movement.max_speed);
		println!("final velocity = {final_velocity}");

		movement.movement_velocity = final_velocity;
		transform.position += final_velocity;
	}
}



pub fn control_movement_system(
	// time: UniqueView<TimeResource>,
	mice: View<MouseComponent>, 
	controls: View<ControlComponent>, 
	mut transforms: ViewMut<TransformComponent>,
	mut movements: ViewMut<MovementComponent>,
) {
	// let apply_duration_secs = (time.this_tick_start - time.last_tick_start).as_secs_f32();
	// println!("ads: {apply_duration_secs}");

	for (mouse, control, transform, movement) in (&mice, &controls, &mut transforms, &mut movements).iter() {

		let rx = mouse.data.total_movement[0] as f32 * 0.001;
		let ry = mouse.data.total_movement[1] as f32 * 0.001;
		let quat_ry = UnitQuaternion::from_euler_angles(ry, 0.0, 0.0);
		let quat_rx = UnitQuaternion::from_euler_angles(0.0, rx, 0.0);
		transform.rotation = quat_rx * transform.rotation * quat_ry;

		let mut kpdv = Vector3::from_element(0.0);
		if let Some(kp) = control.data.control_duration(movement.cid_forward) {
			kpdv.z += kp.as_secs_f32();
		}
		if let Some(kp) = control.data.control_duration(movement.cid_backward) {
			kpdv.z -= kp.as_secs_f32();
		}
		if let Some(kp) = control.data.control_duration(movement.cid_right) {
			kpdv.x += kp.as_secs_f32();
		}
		if let Some(kp) = control.data.control_duration(movement.cid_left) {
			kpdv.x -= kp.as_secs_f32();
		}
		if let Some(kp) = control.data.control_duration(movement.cid_up) {
			kpdv.y += kp.as_secs_f32();
		}
		if let Some(kp) = control.data.control_duration(movement.cid_down) {
			kpdv.y -= kp.as_secs_f32();
		}

		transform.position += transform.rotation * kpdv * movement.max_speed;
	}
}
