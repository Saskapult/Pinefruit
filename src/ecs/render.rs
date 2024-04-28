use std::time::Instant;
use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Mat4, Vec4, Vec2};
use krender::{prelude::*, MeshKey, MaterialKey, TextureKey, BufferKey};
use ekstensions::prelude::*;
use rand::Rng;
use crate::game::{BufferResource, TextureResource, MaterialResource, OutputResolutionComponent};
use super::TransformComponent;



#[derive(Debug, Component)]
pub struct CameraComponent {
	pub fovy: f32, // In radians, don't forget
	pub near: f32,
	pub far: f32,
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			fovy: 45.0_f32.to_radians(),
			near: 0.1, // Self::near_from_fovy_degrees(45.0),
			far: 500.0,
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

	// let opengl_wgpu_matrix = Mat4 {
	// 	x_axis: Vec4::new(1.0, 0.0, 0.0, 0.0),
	// 	y_axis: Vec4::new(0.0, 1.0, 0.0, 0.0),
	// 	z_axis: Vec4::new(0.0, 0.0, 0.5, 0.5),
	// 	w_axis: Vec4::new(0.0, 0.0, 0.0, 1.0),
	// };

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
				.and_then(|t| Some(t.size))
				.unwrap();
			size.width as f32 / size.height as f32
		};

		// opengl_wgpu_matrix * 
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
			projection,
			projection_i: projection.inverse(),
			view_projection: projection * view,
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


#[derive(Component, Debug)]
pub struct ModelComponent {
	pub material: MaterialKey,
	pub mesh: MeshKey,
}


#[derive(Component, Debug)]
pub struct SkeletalAttachmentComponent {
	pub entity: Entity,
	pub bone: usize,
}


#[derive(Component, Debug)]
/// A straight line between two points.
/// Usually accompanied by a RenderMarkerComponent.
/// Might be accompanied by a LifetimeComponent.
pub struct SimpleLineComponent {
	pub start: Vec3,
	pub end: Vec3,
}


#[derive(Component, Debug)]
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

#[derive(Component, Debug)]
pub struct RenderTargetSizeComponent {
	pub size: [u32; 2],
} 
// And then have a system that resizes the context's result texture?


#[derive(Debug, Component, Default)]
pub struct SSAOComponent {
	// No kernel settings because we can't adjust the sample count
	// Fixed size unifiorm buffer issue!
	pub kernel: Option<BufferKey>,

	pub noise_settings: SSAONoiseSettings,
	old_noise_settings: SSAONoiseSettings,
	pub noise: Option<TextureKey>,

	pub render_settings: SSAORenderSettings,
	old_render_settings: SSAORenderSettings,
	pub render_settings_buffer: Option<BufferKey>,

	pub output_settings: SSAOOutputTextureSettings,
	pub output: Option<TextureKey>,
	pub generate_mtl: Option<MaterialKey>,
	pub apply_mtl: Option<MaterialKey>,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SSAONoiseSettings {
	pub width: u32,
	pub height: u32,
}
impl Default for SSAONoiseSettings {
	fn default() -> Self {
		Self {
			width: 4,
			height: 4,
		}
	}
}


// Used to tell the shader what do do with the information it is given
#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, PartialEq)]
pub struct SSAORenderSettings {
	pub tile_scale: f32, // Should depend on output resolution
	pub contrast: f32,
	pub bias: f32,
	pub radius: f32,
	// Unless we were to store ssao kernel in a storage buffer, it is stored in a uniform buffer
	// The shader therefore has a fixed kernel size and we can't adjust it without reloading it
	// pub kenerl_size: u32,
}
impl Default for SSAORenderSettings {
	fn default() -> Self {
		Self {
			tile_scale: 0.0,
			contrast: 0.5,
			bias: 0.0,
			radius: 1.0,
		}
	}
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SSAOOutputTextureSettings {
	pub scale: f32
}
impl Default for SSAOOutputTextureSettings {
	fn default() -> Self {
		Self {
			scale: 1.0,
		}
	}
}


// Todo: write new data to settings instread of making new buffer
pub fn ssao_system(
	(
		context,
		input,
	): (
		&mut RenderContext<Entity>,
		&mut RenderInput<Entity>,
	),
	mut buffers: ResMut<BufferResource>,
	mut textures: ResMut<TextureResource>,
	mut ssaos: CompMut<SSAOComponent>,
	mut materials: ResMut<MaterialResource>,
) {
	info!("SSAO system");
	if let Some(ssao) = context.entity.and_then(|entity| ssaos.get_mut(entity)) {
		let kernel_dirty = ssao.kernel.is_none();

		if kernel_dirty {
			warn!("Rebuilding SSAO kernel");

			let data = make_ssao_kernel().iter().copied()
				.map(|v| [v.x, v.y, v.z, 0.0])
				.collect::<Vec<_>>();

			let key = *ssao.kernel.get_or_insert_with(|| {
				trace!("Initialize SSAO kernel");
				let key = buffers.insert(Buffer::new(
					"ssao kernel", 
					data.len() as u64 * 4 * 4, 
					false, 
					true, 
					true,
				));
				context.insert_buffer("ssao kernel", key);
				key
			});
			let b = buffers.get_mut(key).unwrap();
			
			b.write_queued(0, bytemuck::cast_slice(data.as_slice()));
		}

		let noise_dirty = ssao.noise.is_none() || ssao.noise_settings != ssao.old_noise_settings;
		
		if noise_dirty {
			warn!("Rebuilding SSAO noise");
			ssao.old_noise_settings = ssao.noise_settings;

			let key = *ssao.noise.get_or_insert_with(|| {
				trace!("Initialize SSAO noise");
				let key = textures.insert(Texture::new(
					"ssao noise", 
					TextureFormat::Rgba32Float, 
					ssao.noise_settings.width, 
					ssao.noise_settings.height, 
					1, 
					false,
					true,
				));
				context.insert_texture("ssao noise", key);
				key
			});
			let t = textures.get_mut(key).unwrap();

			let data = make_ssao_noise(ssao.noise_settings).iter()
				.copied()
				.map(|v| [v.x, v.y, 0.0, 0.0])
				.flatten()
				.collect::<Vec<_>>();
			t.set_size(ssao.noise_settings.width, ssao.noise_settings.height, 1);
			t.write_queued(0, wgpu::Origin3d::ZERO, bytemuck::cast_slice(data.as_slice()));
		}

		let render_dirty = ssao.render_settings_buffer.is_none() || ssao.render_settings != ssao.old_render_settings;

		if render_dirty {
			warn!("Rebuilding SSAO settings");
			ssao.old_render_settings = ssao.render_settings;

			let key = *ssao.render_settings_buffer.get_or_insert_with(|| {
				trace!("Initialize SSAO settings");
				let key = buffers.insert(Buffer::new(
					"ssao settings", 
					std::mem::size_of::<SSAORenderSettings>() as u64, 
					false, 
					true, 
					true,
				));
				context.insert_buffer("ssao settings", key);
				key
			});
			let b = buffers.get_mut(key).unwrap();
			b.write_queued(0, bytemuck::bytes_of(&ssao.render_settings));
		}

		let output_dirty = ssao.output
			.and_then(|k| textures.get(k))
			.and_then(|t| {
				let output_size = context.textures.get("output_texture").copied()
					.and_then(|k| textures.get(k))
					.unwrap().size;
				let width = (ssao.output_settings.scale * output_size.width as f32).round() as u32;
				let height = (ssao.output_settings.scale * output_size.height as f32).round() as u32;
				Some(t.size.width != width || t.size.height != height)
			})
			.unwrap_or(true);

		if output_dirty {
			warn!("Rebuilding SSAO output");
			let output_size = context.textures.get("output_texture").copied()
					.and_then(|k| textures.get(k))
					.unwrap().size;
			let width = (ssao.output_settings.scale * output_size.width as f32).round() as u32;
			let height = (ssao.output_settings.scale * output_size.height as f32).round() as u32;

			let key = *ssao.output.get_or_insert_with(|| {
				trace!("Initialize SSAO output");
				let key = textures.insert(Texture::new(
					"ssao output", 
					TextureFormat::Rgba8Unorm, 
					width, 
					height, 
					1, 
					false,
					false,
				).with_usages(wgpu::TextureUsages::RENDER_ATTACHMENT));
				context.insert_texture("ssao output", key);
				key
			});
			let t = textures.get_mut(key).unwrap();
			t.set_size(width, height, 1);
		}
		
		let ssao_generate_mtl = *ssao.generate_mtl.get_or_insert_with(|| {
			info!("Insert ssao generate material");
			materials.read("resources/materials/ssao_generate.ron")
		});
		input.stage("ssao generate")
			.target(AbstractRenderTarget::new().with_colour(RRID::context("ssao output"), None))
			.push((ssao_generate_mtl, None, Entity::default()));

		let ssao_apply_mtl = *ssao.apply_mtl.get_or_insert_with(|| {
			info!("Insert ssao apply material");
			materials.read("resources/materials/ssao_apply.ron")
		});
		input.stage("ssao apply")
			.target(AbstractRenderTarget::new().with_colour(RRID::context("output_texture"), None))
			.push((ssao_apply_mtl, None, Entity::default()));
	}
}


// A hemispherical kernel of radius 1.0 facing +z
fn make_ssao_kernel() -> Vec<Vec3> {
	const KERNEL_SIZE: u32 = 64;

	#[inline(always)]
	fn lerp(a: f32, b: f32, f: f32) -> f32 {
		a + f * (b - a)
	}

	let mut rng = rand::thread_rng();
	(0..KERNEL_SIZE).map(|i| {
		// Hemisphere
		let v = Vec3::new(
			rng.gen::<f32>() * 2.0 - 1.0, 
			rng.gen::<f32>() * 2.0 - 1.0, 
			rng.gen::<f32>(),
		).normalize() * rng.gen::<f32>();

		// More samples closer to centre
		let scale = (i as f32) / ((KERNEL_SIZE - 1) as f32);
		let scale = lerp(0.1, 1.0, scale.powi(2));
		let v = v * scale;

		v
	}).collect()
}


// Random normal-tangent-space vectors
// These are only positive values, so do `* 2.0 - 1.0` in the shader
fn make_ssao_noise(settings: SSAONoiseSettings) -> Vec<Vec2> {
	let mut rng = rand::thread_rng();
	(0..(settings.width*settings.height)).map(|_| {
		Vec2::new(
			rng.gen::<f32>() * 2.0 - 1.0, 
			rng.gen::<f32>() * 2.0 - 1.0, 
		)
	}).collect()
}


#[derive(Debug, Component)]
pub struct AlbedoOutputComponent {
	pub width: u32,
	pub height: u32,
	texture: Option<TextureKey>,
}


// The albedo output should have a resolution equal to the output texture. 
pub fn context_albedo_system(
	(context,): (&mut RenderContext<Entity>,), 
	mut textures: ResMut<TextureResource>,
	mut albedos: CompMut<AlbedoOutputComponent>,
	ouput_textures: Comp<OutputResolutionComponent>,
) {
	if let Some(entity) = context.entity {
		if let Some(output_texture) = ouput_textures.get(entity) {
			// Should probably do this elsewhere
			if !albedos.contains(entity) {
				albedos.insert(entity, AlbedoOutputComponent {
					width: output_texture.width,
					height: output_texture.height,
					texture: None,
				});
			}

			if let Some(albedo) = albedos.get_mut(entity) {
				let albedo_dirty = albedo.texture.is_none()
					|| albedo.width != output_texture.width
					|| albedo.height != output_texture.height;
				if albedo_dirty {
					albedo.width = output_texture.width;
					albedo.height = output_texture.height;

					let key = *albedo.texture.get_or_insert_with(|| {
						trace!("Initialize albedo texture");
						let key = textures.insert(Texture::new(
							"ssao output", 
							TextureFormat::Rgba8Unorm, 
							albedo.width, 
							albedo.height, 
							1, 
							false,
							false,
						).with_usages(wgpu::TextureUsages::RENDER_ATTACHMENT));
						context.insert_texture("albedo", key);
						key
					});
					let t = textures.get_mut(key).unwrap();
					debug!("Resize albedo texture to {}x{}", albedo.width, albedo.height);
					t.set_size(albedo.width, albedo.height, 1);
				}
			}
		}
	}
}
