use image::imageops::FilterType;
use image::{GenericImageView, DynamicImage};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{PathBuf, Path};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use crate::texture::*;
use generational_arena::{Arena, Index};




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
	pub fn translate(self) -> wgpu::TextureFormat {
		self.into()
	}
	pub fn is_depth(&self) -> bool {
		match self {
			TextureFormat::Depth32Float => true,
			_ => false,
		}
	}
	pub fn bytes_per_element(&self) -> u32 {
		match self {
			TextureFormat::Rgba8Unorm => 4,
			TextureFormat::Rgba8UnormSrgb => 4,
			TextureFormat::Bgra8UnormSrgb => 4,
			TextureFormat::Depth32Float => 4,
			TextureFormat::R8Unorm => 1,
		}
	}
	pub fn image_bytes(&self, image: &DynamicImage) -> Vec<u8> {
		match self {
			TextureFormat::Rgba8Unorm => image.to_rgba8(),
			TextureFormat::Rgba8UnormSrgb => image.to_rgba8(),
			_ => todo!("Figure out how to make slices of non-rgb(a) data"),
		}.into_raw()
	}
}
impl Into<wgpu::TextureFormat> for TextureFormat {
	fn into(self) -> wgpu::TextureFormat {
		match self {
			TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
			TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
			TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
			TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
			TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
		}
	}
}
impl From<wgpu::TextureFormat> for TextureFormat {
	fn from(fmt: wgpu::TextureFormat) -> TextureFormat {
		match fmt {
			wgpu::TextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
			wgpu::TextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
			wgpu::TextureFormat::Bgra8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
			wgpu::TextureFormat::Depth32Float => TextureFormat::Depth32Float,
			wgpu::TextureFormat::R8Unorm => TextureFormat::R8Unorm,
			_ => panic!("No conversion!"),
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
	pub mipped_yet: bool, // Have mipmaps been generated yet
}
impl BoundTexture {
	const DEFAULT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm; 
	const DEFAULT_FORMAT_SRGB: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb; 
	pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;    
	// Usages needed to create a non-blank texture
	const NONBLANK_USAGES: wgpu::TextureUsages = wgpu::TextureUsages::COPY_DST;
	// Texture usages needed for mipmapping
	const MIP_USAGES: wgpu::TextureUsages = wgpu::TextureUsages::TEXTURE_BINDING;

	pub fn new(
		device: &wgpu::Device, 
		format: TextureFormat, 
		width: u32, 
		height: u32, 
		mip_count: u32, // Should not be zero
		label: impl AsRef<str>,
		mut usages: wgpu::TextureUsages,
	) -> Self {
		let label = label.as_ref();

		let mipped = mip_count > 1;
		if mipped {
			warn!("entering untested mip code stuff, ye be warned");
			let missing_usages = (usages & Self::MIP_USAGES) ^ wgpu::TextureUsages::all();

			if missing_usages == wgpu::TextureUsages::empty() {
				warn!("Texture {label} is missing usages required for mipmapping, adding usages {missing_usages:?}");
				usages = usages | Self::MIP_USAGES
			}
		}
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		let desc = wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: mip_count,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: format.into(),
			usage: usages,
		};
		let texture = device.create_texture(&desc);

		let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

		Self { 
			name: label.into(), 
			texture, 
			view, 
			size, 
			mip_count,
			mipped_yet: !mipped,
		}
	}
	
	pub fn create_depth_texture(
		device: &wgpu::Device, 
		width: u32, 
		height: u32, 
		label: impl AsRef<str>,
		usages: wgpu::TextureUsages, // wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
	) -> Self {
		Self::new(
			device,
			Self::DEPTH_FORMAT,
			width, 
			height,
			1,
			label,
			usages,
		)
	}

	pub fn from_path(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		path: impl AsRef<Path>,
		mip_count: u32, 
		label: impl AsRef<str>,
		format: TextureFormat,
		usages: wgpu::TextureUsages
	) -> Self {
		let image = image::open(path).expect("Failed to open file");
		BoundTexture::from_image(device, queue, &image, mip_count, label, format, usages)
	}

	pub fn from_image(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		image: &image::DynamicImage,
		mip_count: u32, 
		label: impl AsRef<str>,
		format: TextureFormat,
		mut usages: wgpu::TextureUsages,
	) -> Self {
		let label = label.as_ref();

		let data = format.image_bytes(&image);
		let (width, height) = image.dimensions();

		usages = usages | Self::NONBLANK_USAGES;

		let texture = Self::new(
			device,
			format,
			width,
			height,
			mip_count,
			label,
			usages,
		);

		queue.write_texture(
			wgpu::ImageCopyTexture {
				aspect: wgpu::TextureAspect::All,
				texture: &texture.texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			&data[..],
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: std::num::NonZeroU32::new(format.bytes_per_element() * width),
				rows_per_image: std::num::NonZeroU32::new(height),
			},
			texture.size,
		);

		texture
	}

	pub fn from_images(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		images: &Vec<DynamicImage>,
		width: u32, 
		height: u32, 
		mip_count: u32, 
		label: impl AsRef<str>,
		format: TextureFormat,
		usages: wgpu::TextureUsages,
	) -> BoundTexture {
		let label = label.as_ref();

		let data = images.iter().flat_map(|i| {
			if i.dimensions() == (width, height) {
				format.image_bytes(i)
			} else {
				let g = i.resize(width, height, FilterType::Triangle);
				format.image_bytes(&g)
			}.to_vec()
		}).collect::<Vec<_>>();
	
		let size = wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: images.len() as u32,
		};
	
		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some(label),
			size,
			mip_level_count: mip_count,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: format.into(),
			usage: usages,
		});
		let _view = texture.create_view(&wgpu::TextureViewDescriptor {
			dimension: Some(wgpu::TextureViewDimension::D2Array),
			..Default::default()
		});

		queue.write_texture(
			texture.as_image_copy(),
			&data[..],
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: NonZeroU32::new(format.bytes_per_element() * width),
				rows_per_image: NonZeroU32::new(height),
			},
			size,
		);
	
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
}



// Maybe hold arena of samplers identified by a sampler descriptor?
#[derive(Debug)]
pub struct BoundTextureManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	// Textures, array textures, yum yum
	// Texture arrays are not textures, they can go sit in a corner
	textures: Arena<BoundTexture>, 
	textures_index_name: HashMap<String, Index>,
	textures_index_path: HashMap<PathBuf, Index>,
	textures_unmipped: Vec<Index>,
}
impl BoundTextureManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(),
			textures: Arena::new(), 
			textures_index_name: HashMap::new(), 
			textures_index_path: HashMap::new(),
			textures_unmipped: Vec::new(),
		}
	}

	fn bind(&mut self, texture: &Texture) -> Index {
		info!("Binding texture {}", &texture.name);
		let entry = BoundTexture::from_image(
			&self.device, &self.queue, 
			&texture.data, 
			1,
			&*format!("{}: {:?}", &texture.name, &texture.path), 
			TextureFormat::Rgba8UnormSrgb,
			wgpu::TextureUsages::RENDER_ATTACHMENT
				| wgpu::TextureUsages::COPY_DST
				| wgpu::TextureUsages::TEXTURE_BINDING,
		);
		let idx = self.insert(entry);
		if let Some(path) = texture.path.clone() {
			self.textures_index_path.insert(path, idx);
		}
		idx
	}

	pub fn insert(&mut self, texture: BoundTexture) -> Index {
		let name = texture.name.clone();
		let mipped_yet = texture.mipped_yet;
		let idx = self.textures.insert(texture);
		self.textures_index_name.insert(name, idx);
		if !mipped_yet {
			self.textures_unmipped.push(idx);
		}
		idx
	}

	pub fn remove_clean(&mut self, index: Index) -> Option<BoundTexture> {
		self.textures_unmipped.retain(|i| *i != index);
		self.textures_index_name.retain(|_, i| *i != index);
		self.textures_index_path.retain(|_, i| *i != index);
		self.textures.remove(index)
	}

	pub fn remove_dirty(&mut self, index: Index) -> Option<BoundTexture> {
		self.textures.remove(index)
	}

	pub fn index(&self, i: Index) -> Option<&BoundTexture> {
		self.textures.get(i)
	}

	pub fn index_name(&self, name: &String) -> Option<Index> {
		self.textures_index_name.get(name).and_then(|&i| Some(i))
	}

	// Index by name, bind if not bound
	pub fn index_name_bind(&mut self, _name: &String) -> Option<Index> {
		todo!("See index_path_bind implementation")
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<Index> {
		self.textures_index_path.get(path).and_then(|&i| Some(i))
	}

	// Index by path, bind if not bound
	pub fn index_path_bind(&mut self, path: &PathBuf, textures: &TextureManager) -> Option<Index> {
		if let Some(i) = self.index_path(path) {
			Some(i)
		} else if let Some(_g) = textures.index_path(path) {
			todo!()
		} else {
			None
		}
	}

	// This function should take all unmipped textures and generate mips for them.
	// Maybe it should take an encoder? Yes, it probably should.
	// Perhaps boundtexture should have a function to decide how it is mipped based on its dimension?
	pub fn mip_unmipped(&mut self, _encoder: &mut wgpu::CommandEncoder) {

		let _textures = self.textures_unmipped.drain(..).filter_map(|i| self.textures.get(i));

		todo!()
	}
}
