use rand::Rng;
use nalgebra::*;
use wgpu::util::DeviceExt;
use crate::render::boundtexture::TextureFormat;
use super::BoundTexture;




#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SSAOUniform {
	pub radius: f32,
	pub bias: f32,
	pub contrast: f32,
	pub noise_scale: [f32; 2],
	pub kernel: [[f32; 3]; 16],
}
impl SSAOUniform {
	pub fn new(width: u32, height: u32) -> Self {
		let scale_width = width as f32 / 4.0;
		let scale_height = height as f32 / 4.0; 
		Self {
			radius: 1.0,
			bias: 0.01,
			contrast: 1.5,
			noise_scale: [scale_width, scale_height],
			kernel: SSAOUniform::make_hemisphere_kernel(),
		}
	}

	pub fn update(&mut self, width: u32, height: u32) {
		let scale_width = width as f32 / 4.0;
		let scale_height = height as f32 / 4.0; 
		self.noise_scale = [scale_width, scale_height];
	}

	pub fn make_hemisphere_kernel() -> [[f32; 3]; 16] {
		
		let mut kernel = [[0.0; 3]; 16];
		let mut rng = rand::thread_rng();

		for i in 0..16 {
			let mut sample = Vector3::new(
				rng.gen::<f32>() * 2.0 - 1.0, 
				rng.gen::<f32>() * 2.0 - 1.0, 
				rng.gen::<f32>(),
			).normalize();

			//sample *= rng.gen::<f32>();

			let mut scale = (i as f32) / (16 as f32);
			let t = scale * scale;
			//scale = (0.1 * (1.0 - t)) + (1.0 * t);
			scale = 0.1 + t * (1.0 - 0.1);
			sample *= scale;
			
			let s = &mut kernel[i];
			s[0] = sample[0];
			s[1] = sample[1];
			s[2] = sample[2];
		}

		kernel
	}

	pub fn make_noise(amount: u32) -> Vec<[f32; 3]> {
		let mut rng = rand::thread_rng();
		(0..amount).map(|_| {
			[
				rng.gen::<f32>() * 2.0 - 1.0,
				rng.gen::<f32>() * 2.0 - 1.0,
				rng.gen::<f32>(),
			]
		}).collect::<Vec<_>>()
	}

	pub fn make_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("SSAO Uniform Buffer"),
			contents: bytemuck::cast_slice(&[self.clone()]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		})
	}

	pub fn update_buffer(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer) {
		queue.write_buffer(
			buffer, 
			0, 
			bytemuck::cast_slice(&[self.clone()]),
		);
	}

	pub fn make_noise_texture(
		device: &wgpu::Device, 
		queue: &wgpu::Queue, 
		size: [u32; 2],
	) -> BoundTexture {
		let ssao_noise_texture = BoundTexture::new(
			&device, 
			TextureFormat::Rgba8Unorm,
			size[0], size[1],
			1,
			"ssao noise", 
			wgpu::TextureUsages::COPY_DST 
				| wgpu::TextureUsages::TEXTURE_BINDING, 
		);
		let num_pixels = (size[0] * size[1]) as usize;
		let random_stuff = {
			let mut rng = rand::thread_rng();
			let u8max = u8::MAX as f32;
			(0..num_pixels).map(|_| {
				// [ // Random
				// 	(rng.gen::<f32>() * u8max) as u8,
				// 	(rng.gen::<f32>() * u8max) as u8,
				// 	(rng.gen::<f32>() * u8max) as u8,
				// 	(rng.gen::<f32>() * u8max) as u8,
				// ]
				[ // Rotate on z axis in tangent space
					((rng.gen::<f32>() * 2.0 - 1.0) * u8max) as u8,
					((rng.gen::<f32>() * 2.0 - 1.0) * u8max) as u8,
					0,
					0,
				]
			}).collect::<Vec<_>>().concat()
		};
		queue.write_texture(
			wgpu::ImageCopyTexture {
				aspect: wgpu::TextureAspect::All,
				texture: &ssao_noise_texture.texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			random_stuff.as_slice(),
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: std::num::NonZeroU32::new(4 * ssao_noise_texture.size.width),
				rows_per_image: std::num::NonZeroU32::new(ssao_noise_texture.size.height),
			}, 
			ssao_noise_texture.size,
		);

		ssao_noise_texture
	}
}
