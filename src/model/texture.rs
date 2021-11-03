use anyhow::*;
use image::{GenericImageView, DynamicImage};
use std::path::{Path, PathBuf};


pub enum TextureType {
	AmbientTexture,
	DiffuseTexture,
	SpecularTexture,
	NormalTexture,
	ShininessTexture,
	DissolveTexture,
}



// A texture entry
struct TextureEntry {
	name: String,
	texture_type: TextureType,
}

// A texture stored on disk
struct TextureDISK {
	entry: TextureEntry,
	path: Pathbuf,
}

// A texture stored in RAM
struct TextureRAM {
	entry: TextureEntry,
	data: DynamicImage,
}
impl TextureRAM {
	// Load from disk
	pub fn from_disk(disk: TextureDISK) -> Result<Self> {
		let entry = disk.entry;
		let data = image::open(disk.path)?;
		Ok(Self {
			entry,
			data,
		})
	}

	// Load from bytes
	pub fn from_bytes(entry: TextureEntry, bytes: &[u8]) -> Result<Self> {
		let data = image::load_from_memory(bytes)?;
		Ok(Self {
			entry,
			data,
		})
	}
}

// A texture buffer on the GPU
struct TextureGPU {
	entry: TextureEntry,
	texture: Texture
}
impl TextureGPU {
	pub fn from_ram(
		ram: TextureRAM, 
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) -> Result<Self> {
		let entry = ram.entry;
		let texture = Texture::from_image(device, queue, &ram.data, None, entry.texture_type)?;
		Ok(Self{
			entry,
			texture,
		})
	}
}




#[derive(Debug)]
pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}
impl Texture {
	// Depth texture stuff
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;    
	pub fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, label: &str) -> Self {
		let size = wgpu::Extent3d {
			width: config.width,
			height: config.height,
			depth_or_array_layers: 1,
		};
		let desc = wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: Self::DEPTH_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT
				| wgpu::TextureUsages::TEXTURE_BINDING,
		};
		let texture = device.create_texture(&desc);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let sampler = device.create_sampler(
			&wgpu::SamplerDescriptor {
				address_mode_u: wgpu::AddressMode::ClampToEdge,
				address_mode_v: wgpu::AddressMode::ClampToEdge,
				address_mode_w: wgpu::AddressMode::ClampToEdge,
				mag_filter: wgpu::FilterMode::Linear,
				min_filter: wgpu::FilterMode::Linear,
				mipmap_filter: wgpu::FilterMode::Nearest,
				compare: Some(wgpu::CompareFunction::LessEqual),
				lod_min_clamp: -100.0,
				lod_max_clamp: 100.0,
				..Default::default()
			}
		);

		Self { texture, view, sampler }
	}

	// Load a texture
	pub fn from_image(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		img: &image::DynamicImage,
		label: Option<&str>,
		ttype: TextureType,
	) -> Result<Self> {
		let rgba = img.to_rgba8();
		let dimensions = img.dimensions();
		let size = wgpu::Extent3d {
			width: dimensions.0,
			height: dimensions.1,
			depth_or_array_layers: 1,
		};
		let format: wgpu::TextureFormat = match ttype {
			TextureType::DiffuseTexture => wgpu::TextureFormat::Rgba8UnormSrgb,
			TextureType::NormalTexture => wgpu::TextureFormat::Rgba8Unorm,
			_ => wgpu::TextureFormat::Rgba8Unorm,
		};
		// Create a texture on the device
		let texture = device.create_texture(
			&wgpu::TextureDescriptor {
				label,
				size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: format,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			}
		);

		// Write into that texture
		queue.write_texture(
			wgpu::ImageCopyTexture {
				aspect: wgpu::TextureAspect::All,
				texture: &texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			&rgba,
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
				rows_per_image: std::num::NonZeroU32::new(dimensions.1),
			},
			size,
		);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let sampler = device.create_sampler(
			&wgpu::SamplerDescriptor {
				address_mode_u: wgpu::AddressMode::ClampToEdge,
				address_mode_v: wgpu::AddressMode::ClampToEdge,
				address_mode_w: wgpu::AddressMode::ClampToEdge,
				mag_filter: wgpu::FilterMode::Linear,
				min_filter: wgpu::FilterMode::Nearest,
				mipmap_filter: wgpu::FilterMode::Nearest,
				..Default::default()
			}
		);
		
		Ok(Self { texture, view, sampler })
	}
}
