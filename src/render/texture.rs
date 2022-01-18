// use anyhow::*;
use image::imageops::FilterType;
use image::{GenericImageView, DynamicImage};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;



#[derive(Debug)]
pub enum TextureType {
	Normal,
	RGBA,
	RGB,
}



#[derive(Debug)]
pub struct BoundTexture {
	pub name: String,
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
}
impl BoundTexture {
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

		let name = format!("Depth texture {}, width {} height {}", &label, width, height);

		Self { name, texture, view }
	}

	pub fn from_path(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: &PathBuf,
		name: &String,
		ttype: TextureType,
	) -> Self {
		let image = image::open(path).expect("Failed to open file");
		BoundTexture::from_image(device, queue, &image, name, ttype)
	}

	pub fn from_image(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		img: &image::DynamicImage,
		name: &String,
		ttype: TextureType,
	) -> Self {
		let name = name.clone();

		let rgba = img.to_rgba8();
		let (width, height) = img.dimensions();
		let size = wgpu::Extent3d {
			width,
			height,
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
				label: Some(name.as_str()),
				size,
				mip_level_count: 1,
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
				bytes_per_row: std::num::NonZeroU32::new(4 * width), // r,g,b,a
				rows_per_image: std::num::NonZeroU32::new(height),
			},
			size,
		);

		Self { name, texture, view }
	}

	pub fn from_images(
		device: &wgpu::Device, 
		queue: &wgpu::Queue, 
		images: &Vec<DynamicImage>,
		name: &String,
		ttype: TextureType,
		width: u32,
		height: u32,
	) -> BoundTexture {
		let name = name.clone();
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
			label: Some(name.as_str()),
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
	
		BoundTexture { name, texture, view }
	}
}



// It is optimal to separate the GPU texture manager and texture data manager
// If we had multiple GPU texture managers we would only want to load the RAM data once
#[derive(Debug, Clone)]
pub struct Texture {
	pub name: String,
	pub path: Option<PathBuf>,
	pub data: DynamicImage,
}
#[derive(Debug)]
pub struct TextureManager {
	textures: Vec<Texture>,
	textures_index_name: HashMap<String, usize>,
	textures_index_path: HashMap<PathBuf, usize>,
}
impl TextureManager {
	pub fn new() -> Self {
		Self {
			textures: Vec::new(),
			textures_index_name: HashMap::new(),
			textures_index_path: HashMap::new(),
		}
	}

	pub fn insert(&mut self, texture: Texture) -> usize {
		info!("New texture {} ({:?})", &texture.name, &texture.path);
		let idx = self.textures.len();
		self.textures_index_name.insert(texture.name.clone(), idx);
		if let Some(path) = texture.path.clone() {
			self.textures_index_path.insert(path, idx);
		}
		self.textures.push(texture);
		idx
	}

	pub fn index(&self, i: usize) -> &Texture {
		&self.textures[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.textures_index_name.contains_key(name) {
			Some(self.textures_index_name[name])
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
}



// In order to unload a texture from the gpu we must be sure that it is not used in any active materials, but I don't know how to do that
pub struct BoundTextureManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	// Textures, array textures, yum yum
	// Texture arrays are not textures, they can go sit in a corner
	// To support unloading this should use a generational arena
	textures: Vec<BoundTexture>, 
	textures_index_name: HashMap<String, usize>,
	textures_index_path: HashMap<PathBuf, usize>,
	data_manager: Arc<RwLock<TextureManager>>,
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
			TextureType::RGBA,
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
