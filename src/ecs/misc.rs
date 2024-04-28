use std::time::{Duration, Instant};
use glam::*;
use ekstensions::prelude::*;



#[derive(Resource, Debug)]
pub struct TimeResource {
	pub this: Instant, // time of this tick
	pub last: Instant, // time of last tick
	pub diff: Duration, // |this - last|
}
impl TimeResource {
	pub fn next(&mut self) {
		self.last = self.this;
		self.this = Instant::now();
		self.diff = self.this - self.last;
	}
}


// Todo: Rename to WorldTransform
#[repr(C)]
#[derive(Component, Debug, Clone, Copy)]
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
