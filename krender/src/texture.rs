use std::{path::{PathBuf, Path}, collections::{HashMap, HashSet}, num::NonZeroU32, sync::atomic::{AtomicBool, Ordering}};
use image::DynamicImage;
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use slotmap::SlotMap;
use crate::{TextureKey, MaterialKey, BindGroupKey, prelude::BindGroupManager, RenderContextKey, util::read_ron};


#[derive(Debug)]
pub enum TextureDataEntry {
	Saved(PathBuf),
	Loaded(Vec<u8>),
}


#[derive(Debug, Serialize, Deserialize)]
pub enum CubemapData {
	// KTX(PathBuf), // Will specify mip levels?
	// DDS(PathBuf),
	Faces([PathBuf; 6]), // Must generate mip levels
}


#[derive(Debug, Serialize, Deserialize)]
pub enum TextureDimensionData {
	D1(PathBuf), // How to store this?
	D2(PathBuf),
	D3(Vec<PathBuf>), // Could make enum if we discover more formats
	D2Array(Vec<PathBuf>),
	Cube(CubemapData),
	// CubeArray(Vec<CubemapData>), 
	// if some have mips and ohers do not, how do you know how to generate mips?
	// We could:
	// - Restrict to all having or not having mipmaps
	// - Just re-mip everything
	// - Somehow pick out the ones without mipmaps? 
	// I don't think I will need this data type anyway
}
impl TextureDimensionData {
	pub fn canonicalize(&mut self, context: impl AsRef<Path>) -> std::io::Result<()> {
		trace!("Canonicalize texture!");
		let context = context.as_ref();
		match self {
			Self::D1(p) => *p = context.join(&p).canonicalize()?,
			Self::D2(p) => *p = context.join(&p).canonicalize()?,
			Self::D3(p) => for p in p.iter_mut() {
				*p = context.join(&p).canonicalize()?;
			},
			Self::D2Array(p) => for p in p.iter_mut() {
				*p = context.join(&p).canonicalize()?;
			},
			Self::Cube(p) => match p {
				CubemapData::Faces(p) => for p in p.iter_mut() {
					*p = context.join(&p).canonicalize()?;
				},
			},
		}
		Ok(())
	}

	pub fn dimension(&self) -> wgpu::TextureDimension {
		match self {
			Self::D1(_) => wgpu::TextureDimension::D1,
			Self::D2(_) => wgpu::TextureDimension::D2,
			Self::D3(_) => wgpu::TextureDimension::D3,
			Self::D2Array(_) => wgpu::TextureDimension::D2,
			Self::Cube(_) => wgpu::TextureDimension::D2,
		}
	}

	pub fn view_dimension(&self) -> wgpu::TextureViewDimension {
		match self {
			Self::D1(_) => wgpu::TextureViewDimension::D1,
			Self::D2(_) => wgpu::TextureViewDimension::D2,
			Self::D3(_) => wgpu::TextureViewDimension::D3,
			Self::D2Array(_) => wgpu::TextureViewDimension::D2Array,
			Self::Cube(_) => wgpu::TextureViewDimension::Cube,
		}
	}

	pub fn load(&self, format: TextureFormat) -> anyhow::Result<(Vec<u8>, wgpu::Extent3d)> {
		trace!("Load texture!");
		match self {
			Self::D1(_) => todo!("Decide what to do when loading D1 textures, or maybe don't allow this at all"),
			Self::D2(path) => {
				let i = image::open(&path)?;
				let size = wgpu::Extent3d {
					width: i.width(),
					height: i.height(),
					depth_or_array_layers: 1,
				};
				Ok((format.image_bytes(&i), size))
			},
			Self::D3(paths) => {
				let mut bytes = Vec::new();
				let mut size = None;
				for path in paths {
					let i = image::open(&path)?;
					let this_s = wgpu::Extent3d {
						width: i.width(),
						height: i.height(),
						depth_or_array_layers: paths.len() as u32,
					};
					let base_s = *size.get_or_insert_with(|| this_s);
					assert_eq!(base_s, this_s);
					bytes.extend_from_slice(format.image_bytes(&i).as_slice());
				}
				
				Ok((bytes, size.unwrap()))
			},
			Self::D2Array(paths) => {
				let mut bytes = Vec::new();
				let mut size = None;
				for path in paths {
					let i = image::open(&path)?;
					let this_s = wgpu::Extent3d {
						width: i.width(),
						height: i.height(),
						depth_or_array_layers: paths.len() as u32,
					};
					let base_s = *size.get_or_insert_with(|| this_s);
					assert_eq!(base_s, this_s);
					bytes.extend_from_slice(format.image_bytes(&i).as_slice());
				}
				
				Ok((bytes, size.unwrap()))
			},
			Self::Cube(cmd) => {
				match cmd {
					CubemapData::Faces(paths) => {
						let mut bytes = Vec::new();
						let mut size = None;
						for path in paths {
							let i = image::open(&path)?;
							let this_s = wgpu::Extent3d {
								width: i.width(),
								height: i.height(),
								depth_or_array_layers: 6,
							};
							let base_s = *size.get_or_insert_with(|| this_s);
							assert_eq!(base_s, this_s);
							bytes.extend_from_slice(format.image_bytes(&i).as_slice());
						}
						
						Ok((bytes, size.unwrap()))
					}
				}
			},
		}
	}
}


/// Wraps [wgpu::TextureUsages]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum TextureUsage {
	CopySrc,
	CopyDst,
	TextureBinding,
	StorageBinding,
	RenderAttachment,
}
impl TextureUsage {
	pub fn slice_into(usages: &[Self]) -> wgpu::TextureUsages {
		usages.iter().copied().fold(wgpu::TextureUsages::empty(), |a, v| a | v.into())
	}
}
impl Into<wgpu::TextureUsages> for TextureUsage {
	fn into(self) -> wgpu::TextureUsages {
		match self {
			Self::CopySrc => wgpu::TextureUsages::COPY_SRC,
			Self::CopyDst => wgpu::TextureUsages::COPY_DST,
			Self::TextureBinding => wgpu::TextureUsages::TEXTURE_BINDING,
			Self::StorageBinding => wgpu::TextureUsages::STORAGE_BINDING,
			Self::RenderAttachment => wgpu::TextureUsages::RENDER_ATTACHMENT,
		}
	}
}


// Material can either have texture.png (derived) or texture.ron (specified)
#[derive(Debug, Serialize, Deserialize)]
pub struct TextureSpecification {
	pub label: String,
	pub source: TextureDimensionData,
	pub format: TextureFormat,
	pub mip_count: NonZeroU32,
	pub base_usages: Vec<TextureUsage>,
	pub readable: bool,
}
impl TextureSpecification {
	pub fn from_d2_path(
		name: impl Into<String>, 
		path: impl AsRef<Path>,
		format: TextureFormat,
		readable: bool,
	) -> Self {
		Self {
			label: name.into(),
			source: TextureDimensionData::D2(path.as_ref().into()),
			format,
			mip_count: 1.try_into().unwrap(),
			base_usages: vec![],
			readable,
		}
	}
}


#[derive(Debug)]
pub struct Texture {
	pub label: String,

	pub spec: Option<PathBuf>,
	pub source: Option<TextureDimensionData>,
	pub data: Option<TextureDataEntry>,
	pub format: TextureFormat,
	pub size: wgpu::Extent3d, 
	pub dimension: wgpu::TextureDimension,
	pub view_dimension: wgpu::TextureViewDimension,

	base_usages: wgpu::TextureUsages,
	materials: RwLock<HashMap<(MaterialKey, RenderContextKey), wgpu::TextureUsages>>,
	bind_groups: RwLock<HashSet<BindGroupKey>>,
	
	mip_count: NonZeroU32,
	// Binding and whether it is dirty
	dirty: AtomicBool,
	binding: Option<BoundTexture>,
	staging: Option<Option<wgpu::Buffer>>,

	queued_writes: Vec<(u32, wgpu::Origin3d, Vec<u8>)>,
}
impl Texture {
	pub fn read_specification(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		trace!("Read texture specification!");
		let path = path.as_ref();
		let mut spec = read_ron::<TextureSpecification>(path)?;
		let context = path.parent().unwrap();
		spec.source.canonicalize(context)?;

		let (d, s) = spec.source.load(spec.format)?;

		let dimension = spec.source.dimension();
		let view_dimension = spec.source.view_dimension();

		Ok(Self {
			label: spec.label,
			spec: Some(path.into()),
			source: Some(spec.source),
			data: Some(TextureDataEntry::Loaded(d)),
			format: spec.format,
			size: s,
			dimension,
			view_dimension,

			base_usages: TextureUsage::slice_into(spec.base_usages.as_slice())
				| wgpu::TextureUsages::COPY_DST,
			materials: RwLock::new(HashMap::new()),
			bind_groups: RwLock::new(HashSet::new()),
			mip_count: spec.mip_count,
			dirty: AtomicBool::new(true),
			binding: None,
			staging: spec.readable.then(|| None),
			queued_writes: Vec::new(),
		})
	}

	pub fn new(
		name: impl Into<String>, 
		format: TextureFormat,
		width: u32,
		height: u32,
		depth: u32,
		readable: bool,
		writable: bool, // not fully implemented! I juist want to render stuff
		// persistent: bool,
	) -> Self {
		Self {
			label: name.into(),
			spec: None,
			source: None,
			data: None,
			format,
			size: wgpu::Extent3d {
				width, height, depth_or_array_layers: depth,
			},
			dimension: wgpu::TextureDimension::D2,
			view_dimension: wgpu::TextureViewDimension::D2,

			base_usages: if writable {
				wgpu::TextureUsages::COPY_DST
			} else {
				wgpu::TextureUsages::empty()
			},
			materials: RwLock::new(HashMap::new()),
			bind_groups: RwLock::new(HashSet::new()),
			mip_count: NonZeroU32::new(1).unwrap(),
			dirty: AtomicBool::new(true),
			binding: None,
			staging: readable.then(|| None),
			queued_writes: Vec::new(),
		}	
	}

	pub fn new_from_path(
		name: impl Into<String>, 
		path: impl AsRef<Path>,
		format: TextureFormat,
		readable: bool,
	) -> Self {
		let path = path.as_ref();
		let mut spec = TextureSpecification::from_d2_path(name, path, format, readable);
		let context = path.parent().unwrap();
		spec.source.canonicalize(context).unwrap();

		let (d, s) = spec.source.load(spec.format).unwrap();

		Self {
			label: spec.label,
			spec: Some(path.into()),
			source: Some(spec.source),
			data: Some(TextureDataEntry::Loaded(d)),
			format: spec.format,
			size: s,
			dimension: wgpu::TextureDimension::D2,
			view_dimension: wgpu::TextureViewDimension::D2,

			base_usages: TextureUsage::slice_into(spec.base_usages.as_slice())
				| wgpu::TextureUsages::COPY_DST,
			materials: RwLock::new(HashMap::new()),
			bind_groups: RwLock::new(HashSet::new()),
			mip_count: spec.mip_count,
			dirty: AtomicBool::new(true),
			binding: None,
			staging: spec.readable.then(|| None),
			queued_writes: Vec::new(),
		}
	}

	pub fn with_usages(mut self, usages: wgpu::TextureUsages) -> Self {
		self.base_usages |= usages;
		self
	}

	pub fn with_mips(mut self, mip_count: u32) -> Self {
		assert_ne!(0, mip_count);
		if mip_count > 1 {
			warn!("entering untested mip code stuff, ye be warned");
			let missing_usages = (self.base_usages & wgpu::TextureUsages::TEXTURE_BINDING) ^ wgpu::TextureUsages::all();

			if missing_usages == wgpu::TextureUsages::empty() {
				warn!("Texture {} is missing usages required for mipmapping, adding usages {missing_usages:?}", self.label);
				self.base_usages |= wgpu::TextureUsages::TEXTURE_BINDING;
			}
		}
		self.mip_count = NonZeroU32::new(mip_count).unwrap();
		self
	}

	pub fn write(
		&mut self, 
		queue: &wgpu::Queue, 
		mip_level: u32, 
		origin: wgpu::Origin3d, 
		data: &[u8],
	) {
		// assert!(self.writable, "Buffer '{}' is not writable!", self.name);
		if let Some(texture) = self.binding.as_ref() {
			let bytes_per_row = std::num::NonZeroU32::new(self.format.bytes_per_element() * texture.size.width).and_then(|u| Some(u.get()));
			let rows_per_image = std::num::NonZeroU32::new(texture.size.height).and_then(|u| Some(u.get()));
			let size = texture.size;
			queue.write_texture(
				wgpu::ImageCopyTexture {
					aspect: wgpu::TextureAspect::All,
					texture: &texture.texture,
					mip_level,
					origin,
				},
				data,
				wgpu::ImageDataLayout {
					offset: 0,
					bytes_per_row,
					rows_per_image,
				},
				size,
			);
		} else {
			warn!("Tried to write to unbound texture '{}', adding to write queue at index {}", self.label, self.queued_writes.len());
			self.queued_writes.push((mip_level, origin, data.to_vec()));
		}
	}

	pub fn write_queued(
		&mut self, 
		mip_level: u32, 
		origin: wgpu::Origin3d, 
		data: &[u8],
	) {
		// assert!(self.writable, "Buffer '{}' is not writable!", self.label);
		self.queued_writes.push((mip_level, origin, data.to_vec()));
	}

	// pub fn mean_rgba(&self) -> Result<[f32; 4], TextureError> {
	// 	let mut r = 0.0;
	// 	let mut g = 0.0;
	// 	let mut b = 0.0;
	// 	let mut a = 0.0;
	// 	let image = image::load_from_memory(self.data.data().unwrap()).unwrap();
	// 	let raw = image.to_rgba32f().into_raw();
	// 	raw.chunks_exact(4)
	// 		.for_each(|p| {
	// 			r += p[0];
	// 			g += p[1];
	// 			b += p[2];
	// 			a += p[3];
	// 		});
		
	// 	Ok([r, g, b, a].map(|v| v / (raw.len() / 4) as f32))
	// }

	pub fn total_bytes(&self) -> u32 {
		self.format.bytes_per_element() * self.size.width * self.size.height * self.size.depth_or_array_layers
	}

	pub fn set_mip_count(&mut self, mip_count: u32) {
		assert!(mip_count > 0, "Cannot have zero mips!");
		if mip_count != self.mip_count.get() {
			self.mip_count = NonZeroU32::new(mip_count).unwrap();
			self.dirty.store(true, Ordering::Relaxed);
		}
	}

	// Setting dirty here does bypass the manager, whcih will make rebuild queues not work
	pub fn set_size(&mut self, x: u32, y: u32, z: u32) {
		let new_size = wgpu::Extent3d {
			width: x, height: y, depth_or_array_layers: z,
		};
		if self.size != new_size {
			self.dirty.store(true, Ordering::Relaxed);
			self.size = new_size;
		}
	}

	pub fn get_data_load(&mut self) -> Option<&[u8]> {
		match &self.data {
			None => {
				if let Some(source) = self.source.as_ref() {
					let (d, _) = source.load(self.format).unwrap();
					self.data = Some(TextureDataEntry::Loaded(d));
				}
			},
			Some(TextureDataEntry::Saved(_)) => todo!(),
			Some(TextureDataEntry::Loaded(_)) => {},
		}
		if let Some(TextureDataEntry::Loaded(d)) = self.data.as_ref() {
			Some(d.as_slice())
		} else {
			None
		}
	}

	pub fn rebind(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		bind_groups: &BindGroupManager,
	) {
		debug!("Binding texture '{}'", self.label);

		let size = self.total_bytes() as u64;
		if let Some(staging) = self.staging.as_mut() {
			let _ = staging.insert(device.create_buffer(&wgpu::BufferDescriptor {
				label: Some(&*format!("{} staging buffer", self.label)), 
				size, 
				usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
				mapped_at_creation: true,
			}));
		}

		if self.binding.is_some() {
			trace!("Marking {} dependent bind groups as invalid", self.bind_groups.read().len());
			self.bind_groups.read().iter().for_each(|&key| bind_groups.mark_dirty(key));
		}

		self.dirty.store(false, Ordering::Relaxed);
		self.binding = Some(BoundTexture::from_texture(device, queue, self));
	}

	pub fn binding(&self) -> Option<&BoundTexture> {
		self.binding.as_ref()
	}

	/// Adds usages from material. Returns a bool indicating if the binding was invalidated. 
	/// Turns out I don't use that for anything but I keep it anyway.
	fn add_dependent_material(&self, material: MaterialKey, context: RenderContextKey, usages: wgpu::TextureUsages) -> bool {
		let current_usages = self.usages();
		self.materials.write().insert((material, context), usages);
		if current_usages | usages != current_usages {
			trace!("Texture '{}' is made invalid by an added material", self.label);
			self.dirty.store(true, Ordering::Relaxed);
			true
		} else {
			false
		}
	}

	fn remove_dependent_material(&self, material: MaterialKey, context: RenderContextKey) -> bool {
		let old_usages = self.usages();
		self.materials.write().remove(&(material, context));
		let new_usages = self.usages();
		if old_usages != new_usages {
			trace!("Buffer '{}' is made invalid by a removed material", self.label);
			self.dirty.store(true, Ordering::Relaxed);
			true
		} else {
			false
		}
	}

	fn add_dependent_bind_group(&self, bind_group: BindGroupKey) {
		self.bind_groups.write().insert(bind_group);
	}

	fn remove_dependent_bind_group(&self, bind_group: BindGroupKey) {
		self.bind_groups.write().remove(&bind_group);
	}

	pub fn usages(&self) -> wgpu::TextureUsages {
		self.materials.read().values()
			.copied()
			.fold(self.base_usages, |a, u| a | u)
	}
}


/// Wgpu TextureFormat wrapper
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Hash)]
pub enum TextureFormat {
	Rgba8Unorm,
	Rgba8UnormSrgb,
	Depth32Float,
	Bgra8Unorm,
	Bgra8UnormSrgb,
	R8Unorm,
	Rgba32Float,
}
impl TextureFormat {
	pub fn bytes_per_element(&self) -> u32 {
		match self {
			TextureFormat::Rgba8Unorm => 4,
			TextureFormat::Rgba8UnormSrgb => 4,
			TextureFormat::Bgra8Unorm => 4,
			TextureFormat::Bgra8UnormSrgb => 4,
			TextureFormat::Depth32Float => 4,
			TextureFormat::R8Unorm => 1,
			TextureFormat::Rgba32Float => 16,
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
			TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
			TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
			TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
			TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
			TextureFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
		}
	}
}
impl From<wgpu::TextureFormat> for TextureFormat {
	fn from(fmt: wgpu::TextureFormat) -> TextureFormat {
		match fmt {
			wgpu::TextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
			wgpu::TextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
			wgpu::TextureFormat::Bgra8Unorm => TextureFormat::Bgra8Unorm,
			wgpu::TextureFormat::Bgra8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
			wgpu::TextureFormat::Depth32Float => TextureFormat::Depth32Float,
			wgpu::TextureFormat::R8Unorm => TextureFormat::R8Unorm,
			wgpu::TextureFormat::Rgba32Float => TextureFormat::Rgba32Float,
			_ => unimplemented!("No conversion!"),
		}
	}
}


#[derive(Debug)]
pub struct BoundTexture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub size: wgpu::Extent3d,
	pub mip_count: NonZeroU32,
	pub mipped_yet: bool, // Have mipmaps been generated yet
	pub usages: wgpu::TextureUsages,
}
impl BoundTexture {
	pub fn from_texture(
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		texture: &mut Texture,
	) -> Self {

		let desc = wgpu::TextureDescriptor {
			label: Some(texture.label.as_str()),
			size: texture.size,
			mip_level_count: texture.mip_count.into(),
			sample_count: 1,
			dimension: texture.dimension,
			format: texture.format.into(),
			usage: texture.usages(),
			view_formats: &[texture.format.into()],
		};
		let w_texture = device.create_texture(&desc);

		let view = w_texture.create_view(&wgpu::TextureViewDescriptor {
			format: Some(texture.format.into()),
			dimension: Some(texture.view_dimension),
			..Default::default()
		});

		let s = Self { 
			texture: w_texture, 
			view, 
			size: texture.size, 
			mip_count: texture.mip_count,
			mipped_yet: texture.mip_count.get() == 1,
			usages: texture.usages(),
		};

		let bytes_per_row = std::num::NonZeroU32::new(texture.format.bytes_per_element() * texture.size.width).and_then(|u| Some(u.get()));
		let rows_per_image = std::num::NonZeroU32::new(texture.size.height).and_then(|u| Some(u.get()));
		let size = texture.size;

		if let Some(data) = texture.get_data_load() {
			queue.write_texture(
				wgpu::ImageCopyTexture {
					aspect: wgpu::TextureAspect::All,
					texture: &s.texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
				},
				data,
				wgpu::ImageDataLayout {
					offset: 0,
					bytes_per_row,
					rows_per_image,
				},
				size,
			);
		}

		s
	}
}


#[derive(Debug, Default)]
pub struct TextureManager {
	textures: SlotMap<TextureKey, Texture>,
	textures_by_name: HashMap<String, TextureKey>,
	textures_by_path: HashMap<PathBuf, TextureKey>,
}
impl TextureManager {
	pub fn new() -> Self {
		Self {
			textures: SlotMap::with_key(),
			textures_by_name: HashMap::new(),
			textures_by_path: HashMap::new(),
		}
	}

	pub fn insert(&mut self, texture: Texture) -> TextureKey {
		let name = texture.label.clone();
		let p = texture.spec.as_ref().and_then(|p| Some(p.clone()));
		let idx = self.textures.insert(texture);
		self.textures_by_name.insert(name, idx);
		if let Some(p) = p {
			self.textures_by_path.insert(p, idx);
		}
		idx
	}

	pub fn remove(&mut self, key: TextureKey) {
		if let Some(texture) = self.textures.remove(key) {
			self.textures_by_name.remove(&texture.label);
		}
	}

	pub fn get(&self, key: TextureKey) -> Option<&Texture> {
		self.textures.get(key)
	}
	pub fn get_mut(&mut self, key: TextureKey) -> Option<&mut Texture> {
		self.textures.get_mut(key)
	}

	pub fn key_by_name(&self, name: &String) -> Option<TextureKey> {
		self.textures_by_name.get(name).copied()
	}

	pub fn key_by_path(&self, path: &PathBuf) -> Option<TextureKey> {
		self.textures_by_path.get(path).copied()
	}

	pub fn add_dependent_material(&self, texture: TextureKey, material: MaterialKey, context: RenderContextKey, usages: wgpu::TextureUsages) {
		if let Some(t) = self.textures.get(texture) {
			t.add_dependent_material(material, context, usages);
		} else {
			warn!("Tried to add dependent material to nonexistent texture");
		}
	}

	pub fn remove_dependent_material(&self, texture: TextureKey, material: MaterialKey, context: RenderContextKey) {
		if let Some(t) = self.textures.get(texture) {
			t.remove_dependent_material(material, context);
		} else {
			warn!("Tried to remove dependent material from nonexistent texture");
		}
	}

	pub fn add_dependent_bind_group(&self, texture: TextureKey, bind_group: BindGroupKey) {
		if let Some(t) = self.textures.get(texture) {
			t.add_dependent_bind_group(bind_group);
		} else {
			warn!("Tried to add dependent bind group to nonexistent texture");
		}
	}

	pub fn remove_dependent_bind_group(&self, texture: TextureKey, bind_group: BindGroupKey) {
		if let Some(t) = self.textures.get(texture) {
			t.remove_dependent_bind_group(bind_group);
		} else {
			warn!("Tried to remove dependent bind group from nonexistent texture");
		}
	}

	/// Bind or rebind textures
	pub fn update_bindings(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bind_groups: &BindGroupManager) {
		for (_, texture) in self.textures.iter_mut() {
			if texture.dirty.load(Ordering::Relaxed) {
				texture.rebind(device, queue, bind_groups);
			}
		}
	}

	pub fn do_queued_writes(&mut self, queue: &wgpu::Queue) {
		for texture in self.textures.values_mut() {
			if texture.queued_writes.len() > 0 {
				trace!("Texture {} does {} queued writes", texture.label, texture.queued_writes.len());
			}

			for (mip_level, origin, data) in texture.queued_writes.iter() {
				let bytes_per_row = std::num::NonZeroU32::new(texture.format.bytes_per_element() * texture.size.width).and_then(|u| Some(u.get()));
				let rows_per_image = std::num::NonZeroU32::new(texture.size.height).and_then(|u| Some(u.get()));
				let size = texture.size;
				queue.write_texture(
					wgpu::ImageCopyTexture {
						aspect: wgpu::TextureAspect::All,
						texture: &texture.binding.as_ref().unwrap().texture,
						mip_level: *mip_level,
						origin: *origin,
					},
					data,
					wgpu::ImageDataLayout {
						offset: 0,
						bytes_per_row,
						rows_per_image,
					},
					size,
				);
			}
			texture.queued_writes.clear();
		}
	}

	// This function should take all unmipped textures and generate mips for them.
	// Maybe it should take an encoder? Yes, it probably should.
	// Perhaps boundtexture should have a function to decide how it is mipped based on its dimension?
	/// Must be called after bind_unbound()
	pub fn mip_unmipped(&mut self, _encoder: &mut wgpu::CommandEncoder) {
		// Iterate through all entries, if loaded check if mipped yet
		todo!()
	}
}
