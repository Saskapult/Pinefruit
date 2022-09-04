use std::time::Instant;
use nalgebra::*;
use shipyard::*;



#[derive(Unique)]
pub struct TimeResource {
	pub this_tick_start: Instant,
	pub last_tick_start: Instant,
}
impl TimeResource {
	pub fn new() -> Self {
		Self {
			this_tick_start: Instant::now(),
			last_tick_start: Instant::now(),
		}
	}

	pub fn next_tick(&mut self) {
		self.last_tick_start = self.this_tick_start;
		self.this_tick_start = Instant::now();
	}
}


#[derive(Component, Debug, Clone)]
pub struct TransformComponent {
	pub position: Vector3<f32>,
	pub rotation: UnitQuaternion<f32>,
	pub scale: Vector3<f32>,
}
impl TransformComponent {
	pub fn new() -> Self {
		Self {
			position: Vector3::from_element(0.0),
			rotation: UnitQuaternion::identity(),
			scale: Vector3::from_element(1.0),
		}
	}
	pub fn with_position(self, position: Vector3<f32>) -> Self {
		Self {
			position,
			rotation: self.rotation,
			scale: self.scale,
		}
	}
	pub fn with_rotation(self, rotation: UnitQuaternion<f32>) -> Self {
		Self {
			position: self.position,
			rotation,
			scale: self.scale,
		}
	}
	pub fn with_scale(self, scale: Vector3<f32>) -> Self {
		Self {
			position: self.position,
			rotation: self.rotation,
			scale,
		}
	}
	pub fn matrix(&self) -> Matrix4<f32> {
		Matrix4::new_nonuniform_scaling(&self.scale) * self.rotation.to_homogeneous() * Matrix4::new_translation(&self.position)
	}
}
