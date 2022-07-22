use std::time::Instant;

use crate::render::*;
use nalgebra::{Point3, Matrix4};
use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::ecs::*;




#[derive(Debug, Copy, Clone)]
pub enum RenderTarget {
	Window(usize),
	Texture(usize),
}



#[derive(Component, Debug, Clone)]
#[storage(VecStorage)]
pub struct CameraComponent {
	// Old and bad
	pub target: RenderTarget,

	pub fovy: f32,
	pub znear: f32,

	// Rename to render distance or something like that
	// Can be used as ray distance or far plane distance.
	pub zfar: f32,
	
	// Also old, also bad
	// Should be stored in a mesh rendering component based on camera entity
	pub render_data: Vec<ModelInstance>, // All the models visible to this camera
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			target: RenderTarget::Window(0),
			fovy: 45.0,
			znear: 0.1,
			zfar: 100.0,
			render_data: Vec::new(),
		}
	}

	pub fn set_fovy(&mut self, degrees: f32) {
		self.fovy = degrees.to_radians();
		self.znear = 1.0 / (degrees.to_radians() / 2.0).tan();
	}
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct ModelComponent {
	pub mesh_idx: usize,
	pub material_idx: usize,
}
impl ModelComponent {
	pub fn new(
		mesh_idx: usize,
		material_idx: usize,
	) -> Self {
		Self {
			mesh_idx,
			material_idx,
		}
	}
}


#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct SkeletonComponent {
	pub index: usize,
}

#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct VVolumeComponent {
	pub volume: bool,
}

#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct SkeletalVVolumeComponent {
	pub bones: Vec<Matrix4<f32>>,
	pub volumes: (bool, usize),
}

#[derive(Component, Debug)]
#[storage(VecStorage)]
/// A straight line between two points.
/// Usually accompanied by a RenderMarkerComponent.
/// Might be accompanied by a LifetimeComponent.
pub struct SimpleLineComponent {
	pub start: Point3<f32>,
	pub end: Point3<f32>,
}

#[derive(Component, Debug)]
#[storage(VecStorage)]
/// A marker to remove this entity after a point in time.
pub struct LifetimeComponent {
	pub expiry: Instant,
}




/// For each camera gets the stuff that should be rendered
// Todo: Buffer the instances to let renderer render independently
pub struct RenderDataSystem;
impl<'a> System<'a> for RenderDataSystem {
	type SystemData = (
		ReadStorage<'a, ModelComponent>,
		ReadStorage<'a, MapComponent>,
		WriteStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);

	fn run(
		&mut self, 
		(
			models,
			maps,
			mut cameras,
			transforms,
		): Self::SystemData,
	) { 
		for (camera, _camera_transform) in (&mut cameras, &transforms).join() {
			
			let mut render_data = Vec::new();
			// Models
			for (model_c, transform_c) in (&models, &transforms).join() {
				let instance = Instance::new()
					.with_position(transform_c.position)
					.with_rotation(transform_c.rotation);
				let model_instance = ModelInstance {
					material_idx: model_c.material_idx,
					mesh_idx: model_c.mesh_idx,
					instance,
				};
				render_data.push(model_instance);
			}
			// Map chunks
			// Todo: rotation
			for (map_c, transform_c) in (&maps, &transforms).join() {
				// Renders ALL available chunks
				for (cp, entry) in &map_c.chunk_models {
					let mesh_mats = match entry {
						ChunkModelEntry::Complete(mesh_mats) => Some(mesh_mats),
						ChunkModelEntry::ReModeling(mesh_mats, _) => Some(mesh_mats),
						_ => None,
					};
					if let Some(mesh_mats) = mesh_mats {
						let position = transform_c.position + map_c.map.chunk_point(*cp);
						let instance = Instance::new().with_position(position);
						for (mesh_idx, material_idx) in mesh_mats.iter().cloned() {
							let model_instance = ModelInstance {
								material_idx,
								mesh_idx,
								instance,
							};
							render_data.push(model_instance);
						}
					}
				}
			}

			camera.render_data = render_data;
		}
	}
}
