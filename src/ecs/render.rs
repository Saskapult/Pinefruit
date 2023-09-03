use std::time::Instant;
use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Mat4, Vec4};
use krender::{prelude::*, MeshKey, MaterialKey};
use eks::prelude::*;
use crate::game::{BufferResource, TextureResource};
use super::TransformComponent;



#[derive(Debug, ComponentIdent)]
pub struct CameraComponent {
	pub fovy: f32, // In radians, don't forget
	pub near: f32,
	pub far: f32,
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			fovy: 45.0_f32.to_radians(),
			near: 0.01, // Self::near_from_fovy_degrees(45.0),
			far: 100.0,
		}
	}

	pub fn with_fovy_degrees(self, degrees: f32) -> Self {
		Self {
			fovy: degrees.to_radians(),
			near: Self::near_from_fovy_degrees(degrees),
			..self
		}
	}

	pub fn with_far(self, far: f32) -> Self {
		Self {
			far,
			..self
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


pub fn context_camera_system(
	(context,): (&mut RenderContext<Entity>,), 
	transforms: Comp<TransformComponent>,
	mut cameras: CompMut<CameraComponent>,
	mut buffers: ResMut<BufferResource>,
	textures: Res<TextureResource>,
) {
	#[repr(C)]
	#[derive(Debug, Pod, Zeroable, Clone, Copy)]
	struct CameraUniformData {
		near: f32,
		far: f32,
		fovy: f32,
		aspect: f32,
		position: Vec4,
		rotation: Mat4,
		view: Mat4,
		view_i: Mat4,
		projection: Mat4,
		projection_i: Mat4,
		view_projection: Mat4,
	}

	let opengl_wgpu_matrix = Mat4 {
		x_axis: Vec4::new(1.0, 0.0, 0.0, 0.0),
		y_axis: Vec4::new(0.0, 1.0, 0.0, 0.0),
		z_axis: Vec4::new(0.0, 0.0, 0.5, 0.5),
		w_axis: Vec4::new(0.0, 0.0, 0.0, 1.0),
	};

	if let Some(entity) = context.entity {
		if !cameras.contains(entity) {
			warn!("Insert camera component");
			cameras.insert(entity, CameraComponent::new());
		}
		let c = cameras.get(entity).unwrap();
		let t = transforms.get(entity).cloned().unwrap_or_default();
		
		let aspect_ratio = {
			let size = context.textures.get("output_texture")
				.cloned()
				.and_then(|k| textures.textures.get(k))
				.and_then(|t| Some(t.data.size))
				.unwrap();
			size.width as f32 / size.height as f32
		};

		let projection = Mat4::perspective_lh(c.fovy, aspect_ratio, c.near, c.far);
		let view = Mat4::from_rotation_translation(t.rotation, t.translation).inverse();
		let uniform = CameraUniformData {
			near: c.near,
			far: c.far,
			fovy: c.fovy,
			aspect: aspect_ratio,
			position: Vec4::new(t.translation.x, t.translation.y, t.translation.z, 1.0),
			rotation: Mat4::from_quat(t.rotation),
			view: Mat4::from_rotation_translation(t.rotation, t.translation).inverse(),
			view_i: Mat4::from_rotation_translation(t.rotation, t.translation),
			projection: Mat4::perspective_lh(c.fovy, aspect_ratio, c.near, c.far),
			projection_i: Mat4::perspective_lh(c.fovy, aspect_ratio, c.near, c.far).inverse(),
			view_projection: opengl_wgpu_matrix * projection * view,
			// inv_projection: projection.inverse(),
		};
		let data = bytemuck::bytes_of(&uniform);

		if let Some(&key) = context.buffers.get(&"camera".to_string()) {
			// Write to buffer
			let buffer = buffers.buffers.get_mut(key).unwrap();
			buffer.write_queued(0, data);
		} else {
			let name = format!("RenderContext '{}' camera buffer", context.name);
			info!("Initialize {name}");
			// Create buffer init
			let buffer = Buffer::new_init(
				name, 
				data, 
				false,
				true,
				false,
			);
			let key = buffers.buffers.insert(buffer);
			context.buffers.insert("camera".to_string(), key);
		}
	}
}


#[derive(ComponentIdent, Debug)]
pub struct ModelComponent {
	pub material: MaterialKey,
	pub mesh: MeshKey,
}


#[derive(ComponentIdent, Debug)]
pub struct SkeletalAttachmentComponent {
	pub entity: Entity,
	pub bone: usize,
}


#[derive(ComponentIdent, Debug)]
/// A straight line between two points.
/// Usually accompanied by a RenderMarkerComponent.
/// Might be accompanied by a LifetimeComponent.
pub struct SimpleLineComponent {
	pub start: Vec3,
	pub end: Vec3,
}


#[derive(ComponentIdent, Debug)]
/// A marker to remove this entity after a point in time.
pub struct LifetimeComponent {
	pub expiry: Instant,
}


// #[derive(Unique, Debug)]
// pub struct RenderContextResource {
// 	pub contexts: RenderContextManager<EntityId>,
// 	// can't run systems with data, bah
// 	// We will just set this or smnk
// 	// Could be its own unique/resource
// 	pub active_context: Option<RenderContextKey>,
// }

/// In the actual thing this should be a dynamic collection of things. 
/// In this version it's just a static set of these functions. 
/// Can't be a workload either 

#[derive(ComponentIdent, Debug)]
pub struct RenderTargetSizeComponent {
	pub size: [u32; 2],
} 
// And then have a system that resizes the context's result texture?

