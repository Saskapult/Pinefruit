use std::time::{Instant, Duration};
use nalgebra::*;
// use specs::prelude::*;
use specs::{Component, VecStorage};

use crate::util::DurationHolder;





// Holds timestep data
pub struct StepResource {
	pub last_step: Instant, // Time of last step
	pub this_step: Instant, // Time of current step
	pub step_diff: Duration, // this-last
	pub step_durations: DurationHolder,
}
impl StepResource {
	pub fn new() -> Self {
		let heh = Instant::now();
		Self {
			last_step: heh,
			this_step: heh, 
			step_diff: heh - heh,
			step_durations: DurationHolder::new(5),
		}
	}
}



#[derive(Component, Debug, Clone)]
#[storage(VecStorage)]
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


