use image::imageops::FilterType;
use image::{GenericImageView, DynamicImage};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::texture::*;




/// Format for loaded textures (not rendering things!)
#[derive(Debug)]
pub enum TextureType {
	RGBA,
	SRGBA,
	DEPTH,
}
impl TextureType {
	pub fn translate(&self) -> wgpu::TextureFormat {
		match self {
			TextureType::SRGBA => wgpu::TextureFormat::Rgba8UnormSrgb,
			TextureType::RGBA => wgpu::TextureFormat::Rgba8Unorm,
			TextureType::DEPTH => wgpu::TextureFormat::Depth32Float,
		}
	}
	// Format and bytes per pixel
	pub fn get_info(&self) -> (wgpu::TextureFormat, u32) {
		match self {
			TextureType::RGBA => (wgpu::TextureFormat::Rgba8Unorm, 4),
			TextureType::SRGBA => (wgpu::TextureFormat::Rgba8UnormSrgb, 4),
			_ => todo!(),
		}
	}
}



/// Wgpu TextureFormat wrapper
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Hash)]
pub enum TextureFormat {
	Rgba8Unorm,
	Rgba8UnormSrgb,
	Depth32Float,
	Bgra8UnormSrgb,
	R8Unorm,
}
impl TextureFormat {
	pub fn translate(&self) -> wgpu::TextureFormat {
		match self {
			TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
			TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
			TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
			TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
			TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
		}
	}
	pub fn is_depth(&self) -> bool {
		match self {
			TextureFormat::Depth32Float => true,
			_ => false,
		}
	}
}



#[derive(Debug)]
pub struct BoundTexture {
	pub name: String,
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub size: wgpu::Extent3d,
	pub mip_count: u32,
}
impl BoundTexture {
	pub const SRGB_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb; 
	pub const OTHER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm; 
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;    

	pub fn new(device: &wgpu::Device, width: u32, height: u32, label: &str) -> Self {
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		let mip_count = 1;
		let desc = wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: mip_count,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: Self::DEPTH_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
		};
		let texture = device.create_texture(&desc);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		let name = format!("Texture {}, width {} height {}", &label, width, height);

		Self { name, texture, view, size, mip_count }
	}
	
	pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> Self {
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		let mip_count = 1;
		let desc = wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: mip_count,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: Self::DEPTH_FORMAT,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
		};
		let texture = device.create_texture(&desc);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		let name = format!("Depth texture {}, width {} height {}", &label, width, height);

		Self { name, texture, view, size, mip_count }
	}

	pub fn from_path(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: &PathBuf,
		name: &String,
		srgb: bool,
	) -> Self {
		let image = image::open(path).expect("Failed to open file");
		BoundTexture::from_image(device, queue, &image, name, srgb)
	}

	pub fn from_image(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		img: &image::DynamicImage,
		name: &String,
		srgb: bool,
	) -> Self {
		let name = name.clone();

		let rgba = img.to_rgba8();
		let (width, height) = img.dimensions();
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		
		let format= if srgb {
			BoundTexture::SRGB_FORMAT
		} else {
			BoundTexture::OTHER_FORMAT
		};

		let mip_count = 1;

		let texture = device.create_texture(
			&wgpu::TextureDescriptor {
				label: Some(name.as_str()),
				size,
				mip_level_count: mip_count,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			}
		);
		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

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
				bytes_per_row: std::num::NonZeroU32::new(4 * width), // r8,g8,b8,a8
				rows_per_image: std::num::NonZeroU32::new(height),
			},
			size,
		);

		Self { name, texture, view, size, mip_count }
	}

	pub fn from_images(
		device: &wgpu::Device, 
		queue: &wgpu::Queue, 
		images: &Vec<DynamicImage>,
		name: &String,
		srgb: bool,
		width: u32,
		height: u32,
	) -> BoundTexture {
		let name = name.clone();
		// Make rgba8 copies with the specified size
		let rgba8 = images.iter().map(|t| {
			if t.dimensions() == (width, height) {
				t.to_rgba8()
			} else {
				t.resize(width, height, FilterType::Triangle).to_rgba8()
			}
		}).collect::<Vec<_>>();
	
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: images.len() as u32,
		};

		let format= if srgb {
			BoundTexture::SRGB_FORMAT
		} else {
			BoundTexture::OTHER_FORMAT
		};

		let mip_count = 1;
	
		// Create the texture stuff
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some(name.as_str()),
			size,
			mip_level_count: mip_count,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
		});
		let view = texture.create_view(&wgpu::TextureViewDescriptor {
			dimension: Some(wgpu::TextureViewDimension::D2Array),
			..Default::default()
		});
	
		// Concatenate raw texture data
		let mut collected_rgba = Vec::new();
		for image_data in rgba8 {
			collected_rgba.append(&mut image_data.into_raw());
		}
	
		// Pass the texture data to the gpu
		queue.write_texture(
			texture.as_image_copy(),
			&collected_rgba[..], // &[u8]
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(NonZeroU32::new(4*width).unwrap()),
				rows_per_image: Some(NonZeroU32::new(height).unwrap()),
			},
			size,
		);
	
		BoundTexture { name, texture, view, size, mip_count }
	}

	/// Bytes will repeat if not big enough
	pub fn from_bytes(
		_device: &wgpu::Device,
		_queue: &wgpu::Queue,
		_name: &String, 
		_width: u32,
		_height: u32,
		_bytes: &[u8], 
		_srgb: bool,
	) -> Self {
		todo!()
	}

	pub fn fill(&self, data: &[u8], queue: &wgpu::Queue) {
		queue.write_texture(
			wgpu::ImageCopyTexture {
				aspect: wgpu::TextureAspect::All,
				texture: &self.texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			data,
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: std::num::NonZeroU32::new(4 * self.size.width),
				rows_per_image: std::num::NonZeroU32::new(self.size.height),
			},
			self.size,
		);
	}

	pub fn new_with_format(
		device: &wgpu::Device,
		name: &String,
		format: wgpu::TextureFormat,
		width: u32,
		height: u32,
	) -> Self {
		let name = name.clone();
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		let mip_count = 1;
		let texture = device.create_texture(
			&wgpu::TextureDescriptor {
				label: Some(name.as_str()),
				size,
				mip_level_count: mip_count,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
			}
		);
		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		Self { name, texture, view, size, mip_count }
	}
}



// In order to unload a texture from the gpu we must be sure that it is not used in any active materials, but I don't know how to do that
#[derive(Debug)]
pub struct BoundTextureManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	// Textures, array textures, yum yum
	// Texture arrays are not textures, they can go sit in a corner
	// To support unloading this should use a generational arena
	textures: Vec<BoundTexture>, 
	textures_index_name: HashMap<String, usize>,
	textures_index_path: HashMap<PathBuf, usize>,
	pub data_manager: Arc<RwLock<TextureManager>>,
}
impl BoundTextureManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		data_manager: &Arc<RwLock<TextureManager>>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(),
			textures: Vec::new(), 
			textures_index_name: HashMap::new(), 
			textures_index_path: HashMap::new(),
			data_manager: data_manager.clone(),
		}
	}

	fn bind(&mut self, texture: &Texture) -> usize {
		info!("Binding texture {}", &texture.name);
		let entry = BoundTexture::from_image(
			&self.device, &self.queue, 
			&texture.data, 
			&format!("{}: {:?}", &texture.name, &texture.path), 
			true, // Todo make this good
		);
		let idx = self.insert(entry);
		if let Some(path) = texture.path.clone() {
			self.textures_index_path.insert(path, idx);
		}
		idx
	}

	pub fn insert(&mut self, texture: BoundTexture) -> usize {
		let idx = self.textures.len();
		self.textures_index_name.insert(texture.name.clone(), idx);
		self.textures.push(texture);
		idx
	}

	pub fn index(&self, i: usize) -> &BoundTexture {
		&self.textures[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.textures_index_name.contains_key(name) {
			Some(self.textures_index_name[name])
		} else {
			None
		}
	}

	// Index by name, bind if not bound
	pub fn index_name_bind(&mut self, name: &String) -> Option<usize> {
		let dm = self.data_manager.read().unwrap();
		if self.textures_index_name.contains_key(name) {
			Some(self.textures_index_name[name])
		} else if let Some(texture_idx) = dm.index_name(name) {
			// Clone is needed because of borrow checker stuff
			let texture = dm.index(texture_idx).clone();
			drop(dm);
			let idx = self.bind(&texture);
			Some(idx)
		} else {
			None
		}
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<usize> {
		if self.textures_index_path.contains_key(path) {
			Some(self.textures_index_path[path])
		} else {
			None
		}
	}

	// Index by path, bind if not bound
	pub fn index_path_bind(&mut self, path: &PathBuf) -> Option<usize> {
		let dm = self.data_manager.read().unwrap();
		if self.textures_index_path.contains_key(path) {
			Some(self.textures_index_path[path])
		} else if let Some(texture_idx) = dm.index_path(path) {
			let texture = dm.index(texture_idx).clone();
			drop(dm);
			let idx = self.bind(&texture);
			Some(idx)
		} else {
			None
		}
	}

}
