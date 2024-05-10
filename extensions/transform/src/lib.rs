use ekstensions::prelude::*;
use glam::*;
use controls::*;

#[macro_use]
extern crate log;



#[repr(C)]
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[storage_options(render_transform = "TransformComponent::render_transform")]
pub struct TransformComponent {
	pub translation: Vec3,
	pub rotation: Quat,
	pub scale: Vec3,
}
impl TransformComponent {
	pub fn new() -> Self {
		Self {
			translation: Vec3::ZERO,
			rotation: Quat::IDENTITY,
			scale: Vec3::ONE,
		}
	}
	pub fn with_position(self, position: Vec3) -> Self {
		Self {
			translation: position,
			rotation: self.rotation,
			scale: self.scale,
		}
	}
	pub fn with_rotation(self, rotation: Quat) -> Self {
		Self {
			translation: self.translation,
			rotation,
			scale: self.scale,
		}
	}
	pub fn with_scale(self, scale: Vec3) -> Self {
		Self {
			translation: self.translation,
			rotation: self.rotation,
			scale,
		}
	}
	pub fn matrix(&self) -> Mat4 {
		Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
	}
	pub fn render_transform(this: *const u8, buffer: &mut Vec<u8>) -> bincode::Result<()> {
		let this = unsafe { &*(this as *const Self) };
		bincode::serialize_into(buffer, &TransformComponent::matrix(this))
	}
}
impl Default for TransformComponent {
	fn default() -> Self {
		Self {
			translation: Vec3::ZERO,
			rotation: Quat::IDENTITY,
			scale: Vec3::ONE,
		}
	}
}


#[derive(Debug, Component)]
pub struct MovementComponent {
	pub cid_right: ControlKey,
	pub cid_left: ControlKey,
	pub cid_up: ControlKey,
	pub cid_down: ControlKey,
	pub cid_forward: ControlKey,
	pub cid_backward: ControlKey,

	// movement_velocity: Vec3,
	pub max_speed: f32,
	pub acceleration: f32,
	pub anti_acceleration: f32,
}
impl MovementComponent {
	pub fn new(control_map: &mut ControlMap) -> Self {
		let key_forward = KeyKey::BoardKey(KeyCode::KeyW.into());
		let key_backward = KeyKey::BoardKey(KeyCode::KeyS.into());
		let key_right = KeyKey::BoardKey(KeyCode::KeyD.into());
		let key_left = KeyKey::BoardKey(KeyCode::KeyA.into());
		let key_up = KeyKey::BoardKey(KeyCode::Space.into());
		let key_down = KeyKey::BoardKey(KeyCode::ShiftLeft.into());

		let cid_right = control_map.new_control(
			"Move Right", 
			"Moves the entity rightward.",
		);
		control_map.add_control_binding(cid_right, KeyCombo::new(
			[key_right],
			KeyModifiers::EMPTY,
		));
		let cid_left = control_map.new_control(
			"Move Left", 
			"Moves the entity leftward.",
		);
		control_map.add_control_binding(cid_left, KeyCombo::new(
			[key_left],
			KeyModifiers::EMPTY,
		));
		let cid_up = control_map.new_control(
			"Move Up", 
			"Moves the entity upward.",
		);
		control_map.add_control_binding(cid_up, KeyCombo::new(
			[key_up],
			KeyModifiers::EMPTY,
		));
		let cid_down = control_map.new_control(
			"Move Down", 
			"Moves the entity downward.",
		);
		control_map.add_control_binding(cid_down, KeyCombo::new(
			[key_down],
			KeyModifiers::EMPTY,
		));
		let cid_forward = control_map.new_control(
			"Move Forward", 
			"Moves the entity forward.",
		);
		control_map.add_control_binding(cid_forward, KeyCombo::new(
			[key_forward],
			KeyModifiers::EMPTY,
		));
		let cid_backward = control_map.new_control(
			"Move Back", 
			"Moves the entity backward.",
		);
		control_map.add_control_binding(cid_backward, KeyCombo::new(
			[key_backward],
			KeyModifiers::EMPTY,
		));

		MovementComponent {
			cid_right, cid_left, cid_up, cid_down, cid_forward, cid_backward, 
			// movement_velocity: Vec3::ZERO,
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


pub fn movement_system(
	controls: Comp<ControlComponent>, 
	mut transforms: CompMut<TransformComponent>,
	mut movements: CompMut<MovementComponent>,
) {
	info!("{} entities have a movement component", movements.len());

	for (control, transform, movement) in (&controls, &mut transforms, &mut movements).iter() {

		let [rx, ry] = control.last_tick_mouse_movement();

		warn!("{rx} {ry} mouse stuff");

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


#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	vec![]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("client_tick", "movement_system", movement_system)
		.run_after("local_control_system");
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(p: &mut ekstensions::ExtensionStorageLoader) {
	p.component::<TransformComponent>();
	p.component::<MovementComponent>();
}
