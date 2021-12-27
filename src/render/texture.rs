// use anyhow::*;
use image::imageops::FilterType;
use image::{GenericImageView, DynamicImage};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;



#[derive(Debug)]
pub enum TextureType {
	Normal,
	RGBA,
	RGB,
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

	pub fn from_path(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: &PathBuf,
		label: Option<&String>,
		ttype: TextureType,
	) -> Self {
		let image = image::open(path).expect("Failed to open file");
		Texture::from_image(device, queue, &image, label, ttype)
	}

	pub fn from_image(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		img: &image::DynamicImage,
		label: Option<&String>,
		ttype: TextureType,
	) -> Self {
		let rgba = img.to_rgba8();
		let dimensions = img.dimensions();
		let size = wgpu::Extent3d {
			width: dimensions.0,
			height: dimensions.1,
			depth_or_array_layers: 1,
		};
		// Could use an enum to choose (normal => Rgba8Unorm)
		let format = match ttype {
			TextureType::RGBA => wgpu::TextureFormat::Rgba8UnormSrgb,
			TextureType::Normal => wgpu::TextureFormat::Rgba8Unorm,
			_ => panic!("Weird image detected!"),
		};

		let texture = device.create_texture(
			&wgpu::TextureDescriptor {
				label: {match label { Some(s) => Some(s.as_str()), None => None,}},
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

		Self { texture, view, sampler }
	}

	pub fn from_images(
		device: &wgpu::Device, 
		queue: &wgpu::Queue, 
		images: &Vec<DynamicImage>,
		label: Option<&String>,
		ttype: TextureType,
		width: u32,
		height: u32,
	) -> Texture {
		// Make rgba8 copies with the specified size
		let rgb8 = images.iter().map(|t| {
			if t.dimensions() == (width, height) {
				t.to_rgba8()
			} else {
				t.resize(width, height, FilterType::Triangle).to_rgba8()
			}
		}).collect::<Vec<_>>();
	
		let texture_size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: images.len() as u32,
		};

		let format = match ttype {
			TextureType::RGBA => wgpu::TextureFormat::Rgba8UnormSrgb,
			TextureType::Normal => wgpu::TextureFormat::Rgba8Unorm,
			_ => panic!("Weird image detected!"),
		};
	
		// Create the texture stuff
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: {match label { Some(s) => Some(s.as_str()), None => None,}},
			size: texture_size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
		});
		let view = texture.create_view(&wgpu::TextureViewDescriptor {
			dimension: Some(wgpu::TextureViewDimension::D2Array),
			..Default::default()
		});
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
	
		// Concatenate raw texture data
		let mut collected_rgba = Vec::new();
		for image_data in rgb8 {
			collected_rgba.append(&mut image_data.into_raw());
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
	
		Texture { texture, view, sampler }
	}
}



// It is optimal to separate the GPU texture manager and texture data manager
// If we had multiple GPU texture managers we would only want to load the RAM data once
// Texture data should be identified by its path to avoid loading multiple times
// This does not allow for management of images not stored on disk, but I just don't care anymore
struct TextureDataEntry {
	name: String,
	path: PathBuf,
	data: DynamicImage,
}
struct TextureDataManager {
	entries: Vec<DynamicImage>,
	index_name: HashMap<String, usize>,
	index_path: HashMap<PathBuf, usize>,
}



// In order to unload a texture from the gpu we must be sure that it is not used in any active materials, but I don't know how to do that
pub struct TextureEntry {
	pub name: String,
	pub path: PathBuf,
	pub texture: Texture,
}
pub struct TextureManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	// Textures, array textures, yum yum
	// Texture arrays are not textures, they can go sit in a corner
	// To support unloading this should use a generational arena
	pub textures: Vec<TextureEntry>, 
	pub index_name: HashMap<String, usize>,
	pub index_path: HashMap<PathBuf, usize>,
	// data_manager: Arc<Mutex<TextureDataManager>>,
}
impl TextureManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		// data_manager: &Arc<Mutex<TextureDataManager>>,
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();
		let textures = Vec::new();
		let index_name = HashMap::new();
		let index_path = HashMap::new();
		// let data_manager = data_manager.clone();

		// // Debug texture!
		// // Could we load this from program memory?
		// let debug_name = "debug texture".to_string();
		// let debug_image = image::load_from_memory_with_format(include_bytes!("debug.png"), image::ImageFormat::Png)
		// 	.expect("Failed to load debug texture");
		// let debug_texture = Texture::from_image(&device, &queue, 
		// 	&debug_image, 
		// 	Some(&debug_name),
		// );
		// textures.insert(&debug_name, debug_texture);

		Self {
			device,
			queue,
			textures,
			index_name,
			index_path,
			// data_manager,
		}
	}

	// Enventually we should be rid of this and just see if the data was in the data manager when requested
	pub fn register(
		&mut self,
		data_entry: &TextureDataEntry
	) -> usize {
		let entry = TextureEntry {
			name: data_entry.name.clone(),
			path: data_entry.path.clone(),
			texture: Texture::from_image(&self.device, &self.queue, &data_entry.data, Some(&format!("{}: {:?}", &data_entry.name, &data_entry.path)), TextureType::RGBA),
		};
		let idx = self.textures.len();

		// let views_idx = self.views.len();
		// self.view_index.push(idx, views_idx);
		// self.views.push(&entry.texture.view);

		self.index_name.insert(entry.name.clone(), idx);
		self.index_path.insert(entry.path.clone(), idx);
		self.textures.push(entry);
		idx
	}


}
