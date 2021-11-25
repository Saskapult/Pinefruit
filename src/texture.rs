use anyhow::*;
use image::{GenericImageView, DynamicImage};
use std::path::PathBuf;
use std::num::NonZeroU32;
use crate::resource_manager::IndexMap;



#[derive(Debug)]
pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler,
}
impl Texture {
	// Depth texture stuff
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;    
	pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> Self {
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		let desc = wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: Self::DEPTH_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
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
	) -> Result<Self> {
		let rgba = img.to_rgba8();
		let dimensions = img.dimensions();
		let size = wgpu::Extent3d {
			width: dimensions.0,
			height: dimensions.1,
			depth_or_array_layers: 1,
		};
		// Could use an enum to choose (normal => Rgba8Unorm)
		let format: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

		let texture = device.create_texture(
			&wgpu::TextureDescriptor {
				label,
				size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			}
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

		Ok(Self { texture, view, sampler })
	}
}


pub struct ArrayTextureHelper {
	pub images: IndexMap<DynamicImage>, 
	pub texture: Texture,
}
impl ArrayTextureHelper {
	pub fn new(device: &wgpu::Device, queue: &wgpu::Queue,) -> Self {
		let images = IndexMap::new();
		let texture = make_array_texture(&device, &queue, &images.data);

		Self {
			images,
			texture,
		}
	}

	// Adds a texture to the array texture data
	// update() must be called to change the actual texture
	pub fn add_from_disk(&mut self, name: &String, path: &PathBuf) -> usize {
		let image_data = image::open(path).expect("Failed to read image from disk");
		self.images.insert(name, image_data)
	}

	// Recreates the array texture
	pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue,) {
		self.texture = make_array_texture(&device, &queue, &self.images.data);
	}
	
}


// Creates an array texture
// Only supports rgba images
fn make_array_texture(
	device: &wgpu::Device, 
	queue: &wgpu::Queue, 
	texture_data: &Vec<DynamicImage>,
) -> Texture {
	// Cheat and load test data
	let r = image::open("resources/blockfaces/dirt.png").expect("Fug");
	let g = image::open("resources/blockfaces/sand.png").expect("Fug");
	let TEMPY = [r, g].to_vec();
	let (width, height) = TEMPY[0].to_rgba8().dimensions();

	let texture_size = wgpu::Extent3d {
		width,
		height,
		depth_or_array_layers: TEMPY.len() as u32,
	};

	// Create the texture stuff
	let texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("textures"),
		size: texture_size,
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8UnormSrgb,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
	});
	let view = texture.create_view(&wgpu::TextureViewDescriptor {
		dimension: Some(wgpu::TextureViewDimension::D2Array),
		..Default::default()
	});
	let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

	// Concatenate the texture data
	let mut collected_rgba = Vec::new();
	for tdata in &TEMPY {
		let rgba = tdata.to_rgba8();
		let mut raw = rgba.into_raw();
		collected_rgba.append(&mut raw);
	}
	// Pass the texture data to the gpu
	queue.write_texture(
		texture.as_image_copy(),
		&collected_rgba[..], // &[u8]
		wgpu::ImageDataLayout {
			offset: 0,
			bytes_per_row: Some(NonZeroU32::new(4*width).unwrap()), // r, g, b, a
			rows_per_image: Some(NonZeroU32::new(height).unwrap()),
		},
		texture_size,
	);

	Texture { 
		texture, 
		view, 
		sampler,
	}
}



struct Material {
	pipeline_id: usize,
	bindings: Vec<usize>, // Camera bg, diffuse texture, normal texture
}


