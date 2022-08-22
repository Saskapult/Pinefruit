use std::time::Instant;
use nalgebra::{Point3, Matrix4};
use shipyard::*;




#[derive(Debug, Copy, Clone)]
pub enum RenderTarget {
	Window(usize),
	Texture(usize),
}



#[derive(Component, Debug, Clone)]
pub struct CameraComponent {
	pub fovy: f32, // In radians, don't forget
	pub near: f32,
	pub far: f32,
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			fovy: 45.0,
			near: Self::near_from_fovy_degrees(45.0),
			far: 100.0,
		}
	}

	fn near_from_fovy_degrees(fovy: f32) -> f32 {
		1.0 / (fovy.to_radians() / 2.0).tan()
	}

	pub fn set_fovy(&mut self, degrees: f32) {
		self.fovy = degrees.to_radians();
		self.near = Self::near_from_fovy_degrees(self.fovy);
	}
}

// // Attached to camera
// #[derive(Component, Debug)]
// struct MeshRenderingComponent {
// 	pub render_data: Vec<ModelInstance>, // All the models visible to this camera
// }
// #[derive(Component, Debug)]
// struct TextureRenderComponent {
// 	pub texture_id: usize,
// }



#[derive(Component, Debug)]
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
pub struct SkeletonComponent {
	pub index: usize,
}

#[derive(Component, Debug)]
pub struct VVolumeComponent {
	pub volume: bool,
}

#[derive(Component, Debug)]
pub struct SkeletalVVolumeComponent {
	pub bones: Vec<Matrix4<f32>>,
	pub volumes: (bool, usize),
}

#[derive(Component, Debug)]
/// A straight line between two points.
/// Usually accompanied by a RenderMarkerComponent.
/// Might be accompanied by a LifetimeComponent.
pub struct SimpleLineComponent {
	pub start: Point3<f32>,
	pub end: Point3<f32>,
}

#[derive(Component, Debug)]
/// A marker to remove this entity after a point in time.
pub struct LifetimeComponent {
	pub expiry: Instant,
}




// /// For each camera gets the stuff that should be rendered
// // Todo: Buffer the instances to let renderer render independently
// pub struct RenderDataSystem;
// impl<'a> System<'a> for RenderDataSystem {
// 	type SystemData = (
// 		ReadStorage<'a, ModelComponent>,
// 		ReadStorage<'a, MapComponent>,
// 		WriteStorage<'a, CameraComponent>,
// 		ReadStorage<'a, TransformComponent>,
// 	);

// 	fn run(
// 		&mut self, 
// 		(
// 			models,
// 			maps,
// 			mut cameras,
// 			transforms,
// 		): Self::SystemData,
// 	) { 
// 		for (camera, _camera_transform) in (&mut cameras, &transforms).join() {
			
// 			let mut render_data = Vec::new();
// 			// Models
// 			for (model_c, transform_c) in (&models, &transforms).join() {
// 				let instance = Instance::new()
// 					.with_position(transform_c.position)
// 					.with_rotation(transform_c.rotation);
// 				let model_instance = ModelInstance {
// 					material_idx: model_c.material_idx,
// 					mesh_idx: model_c.mesh_idx,
// 					instance,
// 				};
// 				render_data.push(model_instance);
// 			}
// 			// Map chunks
// 			// Todo: rotation
// 			for (map_c, transform_c) in (&maps, &transforms).join() {
// 				// Renders ALL available chunks
// 				for (cp, entry) in &map_c.chunk_models {
// 					let mesh_mats = match entry {
// 						ChunkModelEntry::Complete(mesh_mats) => Some(mesh_mats),
// 						ChunkModelEntry::ReModeling(mesh_mats, _) => Some(mesh_mats),
// 						_ => None,
// 					};
// 					if let Some(mesh_mats) = mesh_mats {
// 						let position = transform_c.position + map_c.map.chunk_point(*cp);
// 						let instance = Instance::new().with_position(position);
// 						for (mesh_idx, material_idx) in mesh_mats.iter().cloned() {
// 							let model_instance = ModelInstance {
// 								material_idx,
// 								mesh_idx,
// 								instance,
// 							};
// 							render_data.push(model_instance);
// 						}
// 					}
// 				}
// 			}

// 			camera.render_data = render_data;
// 		}
// 	}
// }
