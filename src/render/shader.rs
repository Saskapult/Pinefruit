use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::path::{PathBuf, Path};
use std::time::SystemTime;
use std::{sync::Arc, num::NonZeroU32};
use std::collections::{HashMap, BTreeMap};
use wgpu::{self, VertexAttribute};
use crate::render::*;
use generational_arena::{Arena, Index};



/*
Todo: Add push constant ranges specification
*/



/// Serializable shader information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSpecification {
	pub name: String,
	pub base: ShaderBaseDescriptor,
	pub bind_groups: BTreeMap<u32, BTreeMap<u32, (ShaderDataSource, SpecificationBindingTypeUsages)>>
}
impl ShaderSpecification {
	pub fn from_path(
		path: impl AsRef<std::path::Path>,
	) -> anyhow::Result<Self> {
		let f = std::fs::File::open(path.as_ref())?;
		let info = ron::de::from_reader::<std::fs::File, ShaderSpecification>(f)?;
		Ok(info)
	}

	pub fn canonicalize_modules(mut self, base_path: &Path) -> std::io::Result<Self> {
		self.base.canonicalize_modules(base_path)?;
		Ok(self)
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ShaderLanguage {
	Spirv,
	Glsl,
	Wgsl,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ShaderDataSource {
	Texture(String),
	Buffer(String),
	MaterialTexture(String),
	MaterialBuffer(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ShaderBaseDescriptor {
	Compute(ShaderModuleDescriptor),
	Polygon{
		vertex: ShaderModuleDescriptor, 
		fragment: Option<ShaderModuleDescriptor>,
		polygon_behaviour: Option<PolygonBehaviour>,
		attachments: Vec<ShaderAttatchmentSpecification>,
		multisample_count: u32,
	},
}
impl ShaderBaseDescriptor {
	pub fn is_polygon(&self) -> bool {
		match self {
			Self::Compute(..) => false,
			Self::Polygon{..} => true,
		}
	}
	pub fn canonicalize_modules(&mut self, base_path: &Path) -> std::io::Result<()> {
		match self {
			Self::Compute(cmd) => {
				cmd.path = base_path.join(&cmd.path).canonicalize()?;
			},
			Self::Polygon{ vertex: vmd, fragment: fmd, .. } => {
				vmd.path = base_path.join(&vmd.path).canonicalize()?;
				if let Some(fmd) = fmd {
					fmd.path = base_path.join(&fmd.path).canonicalize()?;
				}
			},
		}
		Ok(())
	}
	pub fn modules<'a>(&'a self) -> Vec<&'a ShaderModuleDescriptor> {
		match self {
			Self::Compute(smd) => vec![smd],
			Self::Polygon { vertex, fragment, .. } => {
				if let Some(smd) = fragment.as_ref() {
					vec![vertex, smd]
				} else {
					vec![vertex]
				}
			}
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShaderModuleDescriptor {
	pub language: ShaderLanguage,
	pub path: PathBuf,
	pub entry: String,
}


/// Properties, depth, and mode
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolygonBehaviour {
	pub vertex_properties: Vec<VertexProperty>,
	pub instance_properties: Vec<InstanceProperty>,
	pub depth_write: Option<bool>,
	pub mode: PolygonMode,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PolygonMode {
	Fill, Line, Point,
}
impl PolygonMode {
	pub fn translate(&self) -> wgpu::PolygonMode {
		match self {
			PolygonMode::Fill => wgpu::PolygonMode::Fill,
			PolygonMode::Line => wgpu::PolygonMode::Line,
			PolygonMode::Point => wgpu::PolygonMode::Point,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct SpecificationTextureDescriptor {
	pub format: TextureFormat,
	pub dimension: SpecificationViewDimension,
	pub count: u32,
	pub sample_type: SpecificationSampleType,
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SpecificationSampleType {
	Float,
	Depth,
	Sint,
	Uint,
}
impl Into<wgpu::TextureSampleType> for SpecificationSampleType {
	fn into(self) -> wgpu::TextureSampleType {
		match self {
			Self::Float => wgpu::TextureSampleType::Float { filterable: true },
			Self::Depth => wgpu::TextureSampleType::Depth,
			Self::Sint => wgpu::TextureSampleType::Sint,
			Self::Uint => wgpu::TextureSampleType::Uint,
		}
	}
}


// The following is just here to let me serialize usage information
#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone)]
pub enum SpecificationBindingTypeUsages {
	UniformBuffer(ShaderStagesWrapper),
	Texture(ShaderStagesWrapper, SpecificationTextureDescriptor, bool, SpecificationSampleType), // Multisampled
	Sampler(ShaderStagesWrapper),
	SamplerArray(ShaderStagesWrapper),
	StorageTexture(ShaderStagesWrapper, SpecificationTextureDescriptor, SpecificationAccessType),
	StorageBuffer(ShaderStagesWrapper, bool), // read only
}
impl SpecificationBindingTypeUsages {
	/// Gets layout entry and the corresponding resource descriptor.
	/// Resource descriptor is merged and used to create final resource!
	pub fn extract(self, layout_index: u32) -> (wgpu::BindGroupLayoutEntry, ResourceDescriptor) {
		match self {
			Self::UniformBuffer(ssw) => {
				let bgle = wgpu::BindGroupLayoutEntry {
					binding: layout_index,
					visibility: ssw.into(),
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				};
				let rd = ResourceDescriptor::Buffer(BufferDescriptor { 
					usages: wgpu::BufferUsages::UNIFORM,
					storage_read_only: true,
				});
				(bgle, rd)
			},
			
			Self::Texture(ssw, std, multisampled, sample_type) => {
				let bgle = wgpu::BindGroupLayoutEntry {
					binding: layout_index,
					visibility: ssw.into(),
					ty: wgpu::BindingType::Texture {
						sample_type: sample_type.into(),
						view_dimension: std.dimension.into(),
						multisampled,
					},
					count: NonZeroU32::new(std.count),
				};
				let rd = ResourceDescriptor::Texture(TextureDescriptor {
					format: std.format,
					dimension: std.dimension.into(),
					usages: wgpu::TextureUsages::TEXTURE_BINDING,
					storage_access_type: None,
				});
				(bgle, rd)
			},
			Self::Sampler(ssw) => {
				let bgle = wgpu::BindGroupLayoutEntry {
					binding: layout_index,
					visibility: ssw.into(),
					ty: wgpu::BindingType::Sampler (
						wgpu::SamplerBindingType::Filtering,
					),
					count: None,
				};
				let rd = ResourceDescriptor::Sampler(None);
				(bgle, rd)
			},
			Self::SamplerArray(ssw) => {
				let bgle = wgpu::BindGroupLayoutEntry {
					binding: layout_index,
					visibility: ssw.into(),
					ty: wgpu::BindingType::Sampler (
						wgpu::SamplerBindingType::Filtering,
					),
					count: NonZeroU32::new(1024),
				};
				let rd = ResourceDescriptor::Sampler(Some(1024));
				(bgle, rd)
			},
			Self::StorageTexture(ssw, std, staw) => {
				let bgle = wgpu::BindGroupLayoutEntry {
					binding: layout_index,
					visibility: ssw.into(),
					ty: wgpu::BindingType::StorageTexture {
						access: staw.into(),
						format: std.format.into(),
						view_dimension: wgpu::TextureViewDimension::D2,
					},
					count: None,
				};
				let rd = ResourceDescriptor::Texture(TextureDescriptor {
					format: std.format,
					dimension: std.dimension.into(),
					usages: wgpu::TextureUsages::STORAGE_BINDING,
					storage_access_type: Some(staw.into()),
				});
				(bgle, rd)
			},
			Self::StorageBuffer(ssw, read_only) => {
				let bgle = wgpu::BindGroupLayoutEntry {
					binding: layout_index,
					visibility: ssw.into(),
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Storage { read_only },
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				};
				let rd = ResourceDescriptor::Buffer(BufferDescriptor { 
					usages: wgpu::BufferUsages::STORAGE,
					storage_read_only: read_only,
				});
				(bgle, rd)
			},
		}
	}
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum SpecificationViewDimension {
	D1,
	D2,
	D3,
	D2Array,
	Cube,
	CubeArray,
}
impl Into<wgpu::TextureViewDimension> for SpecificationViewDimension {
	fn into(self) -> wgpu::TextureViewDimension {
		match self {
			Self::D1 => wgpu::TextureViewDimension::D1,
			Self::D2 => wgpu::TextureViewDimension::D2,
			Self::D3 => wgpu::TextureViewDimension::D3,
			Self::D2Array => wgpu::TextureViewDimension::D2Array,
			Self::Cube => wgpu::TextureViewDimension::Cube,
			Self::CubeArray => wgpu::TextureViewDimension::CubeArray,
		}
	}
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum SpecificationShaderStage {
	Vertex,
	Fragment,
	Compute,
}
impl Into<wgpu::ShaderStages> for SpecificationShaderStage {
	fn into(self) -> wgpu::ShaderStages {
		match self {
			Self::Vertex => wgpu::ShaderStages::VERTEX,
			Self::Fragment => wgpu::ShaderStages::FRAGMENT,
			Self::Compute => wgpu::ShaderStages::COMPUTE,
		}
	}
}
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct ShaderStagesWrapper(Vec<SpecificationShaderStage>);
impl Into<wgpu::ShaderStages> for ShaderStagesWrapper {
	fn into(self) -> wgpu::ShaderStages {
		let mut g = wgpu::ShaderStages::NONE;
		for &f in self.0.iter() {
			let d = f.into();
			g = g | d;
		}
		g
	}
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum SpecificationAccessType {
    WriteOnly,
    ReadOnly,
    ReadWrite,
}
impl Into<wgpu::StorageTextureAccess> for SpecificationAccessType {
	fn into(self) -> wgpu::StorageTextureAccess {
		match self {
			Self::WriteOnly => wgpu::StorageTextureAccess::WriteOnly,
			Self::ReadOnly => wgpu::StorageTextureAccess::ReadOnly,
			Self::ReadWrite => wgpu::StorageTextureAccess::ReadWrite,
		}
	}
}


#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub enum ResourceDescriptor {
	Buffer(BufferDescriptor),
	Texture(TextureDescriptor),
	Sampler(Option<u32>),
}
impl ResourceDescriptor {
	pub fn try_combine(&mut self, other: &Self) -> bool {
		match self {
			Self::Buffer(bd) => if let Self::Buffer(obd) = other {
				bd.try_combine(obd)
			} else {
				false
			},
			Self::Texture(td) => if let Self::Texture(otd) = other {
				td.try_combine(otd)
			} else {
				false
			},
			Self::Sampler(sc) => if let Self::Sampler(osc) = other {
				if let Some(count) = sc {
					if let Some(o_count) = osc {
						warn!("Combining sampler array counts to make biggest one of that");
						*count = u32::max(*count, *o_count);
						return true;
					}
				}
				false
			} else {
				false
			},
		}
	}
}
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub struct BufferDescriptor {
	pub usages: wgpu::BufferUsages,
	pub storage_read_only: bool, // default true, only read if storage is in usages
}
impl BufferDescriptor {
	pub fn try_combine(&mut self, other: &Self) -> bool {
		self.usages = self.usages | other.usages;
		self.storage_read_only &= other.storage_read_only;
		true
	}
}
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub struct TextureDescriptor {
	pub format: TextureFormat,
	pub dimension: wgpu::TextureViewDimension,
	pub usages: wgpu::TextureUsages,
	pub storage_access_type: Option<wgpu::StorageTextureAccess>,
}
impl TextureDescriptor {
	pub fn try_combine(&mut self, other: &Self) -> bool {
		if self.format == other.format && self.dimension == other.dimension {
			self.usages = self.usages | other.usages;
			if let Some(o_sta) = other.storage_access_type {
				if let Some(sta) = self.storage_access_type {
					let combined_mask = Self::sta_to_mask(sta) | Self::sta_to_mask(o_sta);
					self.storage_access_type = Some(Self::sta_from_mask(combined_mask));
				} else {
					self.storage_access_type = Some(o_sta);
				}
			}
			true
		} else {
			false
		}
	}

	fn sta_to_mask(sta: wgpu::StorageTextureAccess) -> u8 {
		match sta {
			wgpu::StorageTextureAccess::ReadOnly => 0b01,
			wgpu::StorageTextureAccess::WriteOnly => 0b10,
			wgpu::StorageTextureAccess::ReadWrite => 0b11,
		}
	}

	fn sta_from_mask(mask: u8) -> wgpu::StorageTextureAccess {
		match mask {
			0b01 => wgpu::StorageTextureAccess::ReadOnly,
			0b10 => wgpu::StorageTextureAccess::WriteOnly,
			0b11 => wgpu::StorageTextureAccess::ReadWrite,
			_ => panic!("Invalid sta mask"),
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShaderAttatchmentSpecification {
	pub usage: String,
	pub format: TextureFormat,
	pub blend_colour: ShaderAttatchmentBlendComponentSpecification,
	pub blend_alpha: ShaderAttatchmentBlendComponentSpecification,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ShaderAttatchmentBlendComponentSpecification {
	Specific {
		src_factor: ShaderAttatchmentBlendFactorSpecification,
		dst_factor: ShaderAttatchmentBlendFactorSpecification,
		operation: ShaderAttatchmentBlendOperationSpecification,
	},
	Replace,	// wgpu::BlendComponent::REPLACE
	Over,		// wgpu::BlendComponent::OVER
}
impl ShaderAttatchmentBlendComponentSpecification {
	pub fn translate(&self) -> wgpu::BlendComponent {
		match self {
			ShaderAttatchmentBlendComponentSpecification::Specific{
				src_factor, dst_factor, operation
			} => {
				wgpu::BlendComponent {
					src_factor: src_factor.translate(),
					dst_factor: dst_factor.translate(),
					operation: operation.translate(),
				}
			},
			ShaderAttatchmentBlendComponentSpecification::Over => wgpu::BlendComponent::OVER,
			ShaderAttatchmentBlendComponentSpecification::Replace => wgpu::BlendComponent::REPLACE,
		}
	}
}



#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ShaderAttatchmentBlendFactorSpecification {
	Zero,
	One,
	OneMinusSrcAlpha,
}
impl ShaderAttatchmentBlendFactorSpecification {
	pub fn translate(&self) -> wgpu::BlendFactor {
		match self {
			ShaderAttatchmentBlendFactorSpecification::Zero => wgpu::BlendFactor::Zero,
			ShaderAttatchmentBlendFactorSpecification::One => wgpu::BlendFactor::One,
			ShaderAttatchmentBlendFactorSpecification::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ShaderAttatchmentBlendOperationSpecification {
	Add,
	Subtract,
	ReverseSubtract,
	Min,
	Max,
}
impl ShaderAttatchmentBlendOperationSpecification {
	pub fn translate(&self) -> wgpu::BlendOperation {
		match self {
			ShaderAttatchmentBlendOperationSpecification::Add => wgpu::BlendOperation::Add,
			ShaderAttatchmentBlendOperationSpecification::Subtract => wgpu::BlendOperation::Subtract,
			ShaderAttatchmentBlendOperationSpecification::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
			ShaderAttatchmentBlendOperationSpecification::Min => wgpu::BlendOperation::Min,
			ShaderAttatchmentBlendOperationSpecification::Max => wgpu::BlendOperation::Max,
		}
	}
}



/// The proto-shader is used to create a shader.
/// (well not yet in the code but it should be)
/// It is created by taking a specification and graph to find visibilities and usages.
#[derive(Debug)]
pub struct ShaderPrototype {
	pub specification_path: PathBuf,
	pub name: String,
	pub base: ShaderBaseDescriptor,
	pub bind_groups: BTreeMap<u32, BTreeMap<u32, (ShaderDataSource, ResourceDescriptor, wgpu::BindGroupLayoutEntry)>>,
	pub push_constant_ranges: Vec<wgpu::PushConstantRange>,
}
impl ShaderPrototype {
	pub fn from_specification_path(
		path: impl AsRef<Path>,
	) -> Self {
		let specification_path = PathBuf::from(path.as_ref()).canonicalize().unwrap();
		let specification_parent = specification_path.parent().unwrap();
		let specification = ShaderSpecification::from_path(&specification_path).unwrap().canonicalize_modules(specification_parent).unwrap();

		let bind_groups = specification.bind_groups.iter()
			.map(|(&i, bg)| {
				(i, bg.iter().map(|(&i, (name, bt))| {
					let (bgle, rd) = bt.clone().extract(i);
					(i, (name.clone(), rd, bgle))
				})
				.collect::<BTreeMap<_,_>>())
			})
			.collect::<BTreeMap<_,_>>();

		Self {
			specification_path,
			name: specification.name,
			base: specification.base,
			bind_groups,
			push_constant_ranges: vec![],
		}
	}

	// Can't output wgpu::BindGroupLayoutDescriptor<'a> because bgles do not necessarily live in contiguous memory
	pub fn bind_group_entries(
		&self, 
		index: u32,
	) -> Option<Vec<wgpu::BindGroupLayoutEntry>> {
		self.bind_groups.get(&index).and_then(|bg| {
			Some(bg.iter().map(|(_, &(_, _, bgle))| {
				bgle
			}).collect::<Vec<_>>())
		})
	}

	pub fn get_resources(&self) -> Vec<(ShaderDataSource, ResourceDescriptor)> {
		let mut resources = Vec::new();

		// Get binding resources
		for (_, bg) in self.bind_groups.iter() {
			for (_, (n, rd, _)) in bg.iter() {
				resources.push((n.clone(), rd.clone()));
			}
		}

		// Get render attachments
		if let ShaderBaseDescriptor::Polygon { attachments, .. } = &self.base {
			for attachment in attachments {
				let rd = ResourceDescriptor::Texture(TextureDescriptor {
					format: attachment.format,
					dimension: wgpu::TextureViewDimension::D2,
					usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
					storage_access_type: None,
				});
				resources.push((ShaderDataSource::Texture(attachment.usage.clone()), rd));
			}
		}

		resources
	}

	// pub fn merge_resources(
	// 	&self, 
	// 	aliases: &HashMap<String, String>, 
	// 	destination: &mut HashMap<String, Vec<ResourceDescriptor>>,
	// ) {
	// 	// Insert into big thing
	// 	for (name, descriptor) in self.get_resources() {
	// 		let name = aliases.get(name).unwrap_or(name);
	// 		if let Some(btus) = destination.get_mut(name) {
	// 			// Try combine with each existing resource descriptor
	// 			let mut found_place = false;
	// 			for o_btu in btus.iter_mut() {
	
	// 				let combine = o_btu.try_combine(&descriptor);

	// 				if combine {
	// 					found_place = true;
	// 					break
	// 				}
	// 			}
	// 			if !found_place {
	// 				btus.push(descriptor);
	// 			}
	// 		} else {
	// 			destination.insert(name.clone(), vec![descriptor]);
	// 		}
	// 	}
	// }
}




// Store alongside prototype
#[derive(Debug)]
pub struct CompiledShader {
	pub pipeline_layout: wgpu::PipelineLayout, // Immutable
	pub pipeline: ShaderPipeline, // Mutable with v/i stuff
	pub bind_group_layout_indices: BTreeMap<u32, Index>,
}



#[derive(Debug, Error)]
pub enum ShaderError {
	#[error("glslc error: {0}")]
	CompilationGlslcError(String),
	#[error("code format error: {0}")]
	CompilationFormatError(String),
	#[error("failed to locate file: {0}")]
	FileMissingError(PathBuf),
	#[error("module '{0}' not found in loaded modules")]
	MissingModuleError(PathBuf),
}



#[derive(Debug)]
pub enum ShaderPipeline {
	Polygon(wgpu::RenderPipeline),
	Compute(wgpu::ComputePipeline),
}
impl ShaderPipeline {
	pub fn compute(&self) -> Option<&wgpu::ComputePipeline> {
		if let Self::Compute(p) = self {
			Some(p)
		} else {
			None
		}
	}
}



// Remember to only change index if shader is changed, not updated
#[derive(Debug)]
pub struct ShaderManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,

	// Prototype, last prototype load, layout, ?pipeline
	shaders: Arena<(ShaderPrototype, SystemTime, wgpu::PipelineLayout, Option<ShaderPipeline>)>,
	shaders_index_by_name: HashMap<String, Index>,
	shaders_index_by_path: HashMap<PathBuf, Index>,

	// Path -> Language, ?(Module, LoadedAt)
	shader_modules: HashMap<PathBuf, (ShaderLanguage, Option<(wgpu::ShaderModule, SystemTime)>)>,

	// Used to find mesh format and derive shader input locations
	// Will not be unloaded because it probably won't be a problem
	combined_vertex_properties: Vec<VertexProperty>,
	combined_instance_properties: Vec<InstanceProperty>,

	// I do not care about unloading these
	bind_group_layouts: Vec<wgpu::BindGroupLayout>,
	bind_group_layouts_bind_group_format: HashMap<Vec<wgpu::BindGroupLayoutEntry>, usize>,
}
impl ShaderManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(), 
			shaders: Arena::new(), 
			shaders_index_by_name: HashMap::new(), 
			shaders_index_by_path: HashMap::new(),

			shader_modules: HashMap::new(),

			combined_vertex_properties: Vec::new(),
			combined_instance_properties: Vec::new(),

			bind_group_layouts: Vec::new(),
			bind_group_layouts_bind_group_format: HashMap::new(),
		}
	}

	/// Does not compile shader pipeline, be use to run update_shaders to do all that stuff.
	pub fn register_path(
		&mut self,
		path: impl AsRef<std::path::Path>,
	) -> Index {
		let path = path.as_ref();
		let path = PathBuf::from(path);
		info!("Registering shader from {:?}", path);
		let prototype = ShaderPrototype::from_specification_path(&path);

		// Add bindings and layout and stuff
		let pipeline_layout = self.shader_pipeline_layout(&prototype);

		// Register modules (no compile)
		for module in prototype.base.modules() {
			self.register_module(module);
		}

		// Update vertex/instance formats
		if let ShaderBaseDescriptor::Polygon { polygon_behaviour: inputs, .. } = &prototype.base {
			let mut anything_changed = false;
			if let Some(polygon_behaviour) = inputs {
				for vp in polygon_behaviour.vertex_properties.iter() {
					if !self.combined_vertex_properties.contains(vp) {
						anything_changed = true;
						self.combined_vertex_properties.push(*vp);
					}
				}
				for ip in polygon_behaviour.instance_properties.iter() {
					if !self.combined_instance_properties.contains(ip) {
						anything_changed = true;
						self.combined_instance_properties.push(*ip);
					}
				}
			}
			if anything_changed {
				debug!("Mesh format changed");
				// Invalidate polygon pipelines
				for (_, (p, _, _, g)) in self.shaders.iter_mut() {
					if p.base.is_polygon() {
						g.take();
					}
				}
			}
		}

		if let Some(&index) = self.shaders_index_by_path.get(&path) {
			index
		} else {
			let name = prototype.name.clone();
			let sp = prototype.specification_path.clone();
			let entry = (prototype, SystemTime::now(), pipeline_layout, None);

			let idx = self.shaders.insert(entry);
			self.shaders_index_by_name.insert(name, idx);
			self.shaders_index_by_path.insert(sp, idx);
			idx
		}		
	}

	pub fn layout(&self, entries: &Vec<wgpu::BindGroupLayoutEntry>) -> Option<&wgpu::BindGroupLayout> {
		self.bind_group_layouts_bind_group_format.get(entries).and_then(|&i| Some(&self.bind_group_layouts[i]))
	}
	pub fn pipeline(&self, index: Index) -> Option<&ShaderPipeline> {
		self.shaders.get(index).and_then(|(_, _, _, g)| g.as_ref())
	}
	pub fn prototype(&self, index: Index) -> Option<&ShaderPrototype> {
		self.shaders.get(index).and_then(|(p, _, _, _)| Some(p))
	}

	/// prototype modified || module reloaded?
	pub fn update_shaders(&mut self) -> Vec<(Index, Result<(), ShaderError>)> {
		// Reload old prototypes
		let mut oldies = Vec::new();
		for (_, (p, f, _, _)) in self.shaders.iter() {
			let last_modified = std::fs::metadata(&p.specification_path).unwrap().modified().unwrap();
			if last_modified > *f {
				oldies.push(p.specification_path.clone());
			}
		}
		for p in oldies {
			self.register_path(p);
		}

		// Load modules
		for module in self.outdated_modules() {
			let l = self.shader_modules[&module].0;
			self.load_module(&module, l);
			// Invalidate any old usages
			for (_, (p, _, _, g)) in self.shaders.iter_mut() {
				if p.base.modules().iter().map(|smd| &smd.path).position(|path| path == &p.specification_path).is_some() {
					g.take();
				}
			}
		}
		
		// Build unbuilt pipleines
		let mut build_pipes = Vec::new();
		for (i, (_, _, _, g)) in self.shaders.iter() {
			if g.is_none() {
				build_pipes.push(i);
			}
		}
		build_pipes.iter().cloned().map(|i| {
			(i, self.create_pipeline(i))
		}).collect::<Vec<_>>()
	}

	/// Was module modified or not loaded
	fn outdated_modules(&self) -> Vec<PathBuf> {
		self.shader_modules.iter().filter_map(|(p, _)| {
			if self.is_module_outdated(p).unwrap().unwrap() {
				Some(p.clone())
			} else {
				None
			}
		}).collect::<Vec<_>>()
	}

	fn register_module(&mut self, smd: &ShaderModuleDescriptor) {
		if !self.shader_modules.contains_key(&smd.path) {
			debug!("Registering new shader module '{:?}'", smd.path);
			self.shader_modules.insert(smd.path.clone(), (smd.language, None));
		}
	}
	fn load_module(&mut self, module_path: &PathBuf, module_language: ShaderLanguage) {
		let t = SystemTime::now();
		let m = load_shader_module(&self.device, module_path, module_language).unwrap();

		let module_path = module_path.canonicalize().unwrap();
		self.shader_modules.insert(module_path, (module_language, Some((m, t))));
	}
	fn get_module(&self, module_path: &PathBuf) -> Option<&wgpu::ShaderModule> {
		self.shader_modules.get(module_path).and_then(|(_, m)| m.as_ref().and_then(|(m, _)| Some(m)))
	}
	fn is_module_outdated(&self, module: &PathBuf) -> Result<std::io::Result<bool>, ShaderError> {
		fn halp(last_loaded: SystemTime, path: &PathBuf) -> std::io::Result<bool> {
			let last_modified = std::fs::metadata(path)?.modified()?;
			Ok(last_loaded < last_modified)
		}

		if let Some((_, g)) = self.shader_modules.get(module) {
			let res = if let Some((_, last_loaded)) = g {
				halp(*last_loaded, module)
			} else {
				Ok(true)
			};
			Ok(res)
		} else {
			Err(ShaderError::MissingModuleError(module.clone()))
		}
	}

	// Attributes and length for vertex and instance
	fn get_attributes(
		&self, 
		vertex_properties: &Vec<VertexProperty>,
		instance_properties: &Vec<InstanceProperty>,
	) -> [(Vec<wgpu::VertexAttribute>, u64); 2] {

		let mut vertex_attributes_length = 0;
		let mut vertex_attributes = Vec::new();
		for vp in self.combined_vertex_properties.iter() {
			for &(size, format) in vp.attribute_segment() {
				if let Some(i) = vertex_properties.iter().position(|v| v == vp) {
					vertex_attributes.push(wgpu::VertexAttribute {
						offset: vertex_attributes_length,
						shader_location: i as u32,
						format,
					});
				}
				vertex_attributes_length += size as wgpu::BufferAddress;
			}
		}

		let mut instance_attributes_length = 0;
		let mut instance_attributes = Vec::new();
		for ip in self.combined_instance_properties.iter() {
			for &(size, format) in ip.attribute_segment() {
				if let Some(i) = instance_properties.iter().position(|v| v == ip) {
					instance_attributes.push(wgpu::VertexAttribute {
						offset: vertex_attributes_length + instance_attributes_length,
						shader_location: (vertex_attributes.len() + i) as u32,
						format,
					});
				}
				instance_attributes_length += size as wgpu::BufferAddress;
			}
		}

		[
			(vertex_attributes, vertex_attributes_length),
			(instance_attributes, instance_attributes_length),
		]
	}

	pub fn index_from_path(&self, path: &PathBuf) -> Option<Index> {
		self.shaders_index_by_path.get(&path.canonicalize().unwrap()).cloned()
	}

	/// Creates a shader pipeline layout, creating bind group layouts if not existing.
	/// Could allow reuse by mapping bind group indices to pipeline, but that's not what I'm doing right now.
	fn shader_pipeline_layout(&mut self, prototype: &ShaderPrototype) -> wgpu::PipelineLayout {
		let mut bind_group_layouts = Vec::new();

		for (&i, _) in &prototype.bind_groups {
			let entries = prototype.bind_group_entries(i).unwrap();
			if let Some(&index) = self.bind_group_layouts_bind_group_format.get(&entries) {
				bind_group_layouts.push(index);
			} else {
				debug!("Creating new bind group layout for bind group {entries:?}");
				let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					label: None,
					entries: &entries[..],
				});
				let index = self.bind_group_layouts.len();
				self.bind_group_layouts.push(bind_group_layout);
				self.bind_group_layouts_bind_group_format.insert(entries, index);
				
				bind_group_layouts.push(index);
			}
		}
		let bind_group_layouts = bind_group_layouts.iter().map(|&i| {
			&self.bind_group_layouts[i]
		}).collect::<Vec<_>>();

		self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &bind_group_layouts[..],
			push_constant_ranges: &prototype.push_constant_ranges[..],
		})
	}

	/// Recreates the pipeline for shader at index.
	/// Reloads modules as it goes
	fn create_pipeline(
		&mut self, 
		index: Index,
	) -> Result<(), ShaderError> {
		let (prototype, _, layout, _) = self.shaders.get(index).unwrap();

		let pipeline = match &prototype.base {
			ShaderBaseDescriptor::Compute(source_file) => {
				let module = self.get_module(&source_file.path)
					.ok_or(ShaderError::MissingModuleError(source_file.path.clone()))?;

				Ok(ShaderPipeline::Compute(compute_pipeline(
					&self.device,
					(&*source_file.entry, module),
					layout,
				)))
			},
			ShaderBaseDescriptor::Polygon { 
				vertex, 
				fragment, 
				polygon_behaviour, 
				attachments,
				multisample_count,
			}=> {
				
				let vertex_module = self.get_module(&vertex.path)
					.and_then(|m| Some((&*vertex.entry, m)))
					.ok_or(ShaderError::MissingModuleError(vertex.path.clone()))?;
				let fragment_module = fragment.as_ref().and_then(|f| 
						Some(self.get_module(&f.path)
							.and_then(|m| Some((&*f.entry, m)))
							.ok_or(ShaderError::MissingModuleError(f.path.clone()))
						)
					)
					.transpose()?;

				let [
					(vertex_attributes, vertex_attributes_length),
					(instance_attributes, instance_attributes_length),
				] = if let Some(pb) = polygon_behaviour {
					self.get_attributes(&pb.vertex_properties, &pb.instance_properties)
				} else {
					[(vec![], 0), (vec![], 0)]
				};
				
				Ok(ShaderPipeline::Polygon(polygon_pipeline(
					&self.device,
					vertex_module,
					fragment_module,
					polygon_behaviour.as_ref(),
					&vertex_attributes,
					vertex_attributes_length,
					&instance_attributes,
					instance_attributes_length,
					attachments,
					*multisample_count,
					&layout,
				)))
			},
		}?;

		let (_, _, _, ppp) = self.shaders.get_mut(index).unwrap();
		*ppp = Some(pipeline);

		Ok(())
	}
}

fn polygon_pipeline(
	device: &wgpu::Device,

	vertex_module: (&str, &wgpu::ShaderModule), 
	fragment_module: Option<(&str, &wgpu::ShaderModule)>,

	polygon_behaviour: Option<&PolygonBehaviour>,
	// Pass nothingburgers if no pb
	vertex_attributes: &Vec<VertexAttribute>,
	vertex_attributes_length: u64,
	instance_attributes: &Vec<VertexAttribute>,
	instance_attributes_length: u64,

	attachments: &Vec<ShaderAttatchmentSpecification>,
	multisample_count: u32,
	layout: &wgpu::PipelineLayout,
) -> wgpu::RenderPipeline {
	// Mesh input
	let vertex_buffer_layouts = if polygon_behaviour.is_some() {
		let vertex_layout = wgpu::VertexBufferLayout {
			array_stride: vertex_attributes_length,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &vertex_attributes[..],
		};
		let instance_layout = wgpu::VertexBufferLayout {
			array_stride: instance_attributes_length,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &instance_attributes[..],
		};
		vec![vertex_layout, instance_layout]
	} else {
		vec![]
	};

	// Depth input
	let depth_stencil = polygon_behaviour.and_then(|svi| {
		svi.depth_write.and_then(|depth_write_enabled| {
			Some(wgpu::DepthStencilState {
				format: BoundTexture::DEPTH_FORMAT.into(),
				depth_write_enabled,
				depth_compare: wgpu::CompareFunction::LessEqual,
				stencil: wgpu::StencilState::default(),
				bias: wgpu::DepthBiasState::default(),
			})
		})			
	});

	// Attachments input
	let attachments = attachments.iter().map(|a| {
		Some(wgpu::ColorTargetState {
			format: a.format.translate(),
			blend: Some(wgpu::BlendState {
				alpha: a.blend_alpha.translate(),
				color: a.blend_colour.translate(),
			}),
			write_mask: wgpu::ColorWrites::ALL,
		})
	}).collect::<Vec<_>>();

	// Modules
	let vertex = wgpu::VertexState {
		module: vertex_module.1,
		entry_point: vertex_module.0,
		buffers: &vertex_buffer_layouts[..],
	};
	let fragment = fragment_module.and_then(|(entry_point, module)| {
		Some(wgpu::FragmentState {
			module,
			entry_point,
			targets: &attachments[..],
		})
	});

	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: None,
		layout: Some(layout),
		vertex,
		fragment,
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: Some(wgpu::Face::Back),
			polygon_mode: polygon_behaviour
				.and_then(|svi| Some(svi.mode.translate()))
				.unwrap_or(wgpu::PolygonMode::Fill),
			unclipped_depth: false,
			conservative: false,
		},
		depth_stencil,
		multisample: wgpu::MultisampleState {
			count: multisample_count,
			mask: !0,
			alpha_to_coverage_enabled: false,
		},
		multiview: None,
	})
}



fn compute_pipeline(
	device: &wgpu::Device,
	module: (&str, &wgpu::ShaderModule),
	layout: &wgpu::PipelineLayout,
) -> wgpu::ComputePipeline {
	device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
		label: None,
		layout: Some(layout),
		module: module.1,
		entry_point: module.0,
	})
}



fn load_shader_module(
	device: &wgpu::Device,
	source_path: &PathBuf, 
	source_type: ShaderLanguage,
) -> Result<wgpu::ShaderModule, ShaderError> {
	match source_type {
		ShaderLanguage::Spirv => {
			warn!("Loading SPIRV module '{source_path:?}'");

			let source = std::fs::read(&source_path).expect("failed to read file");

			Ok(unsafe { device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
				label: source_path.to_str(), 
				source: wgpu::util::make_spirv_raw(&source[..]), 
			})})
		},
		ShaderLanguage::Glsl => {
			use std::process::Command;
			
			warn!("Loading GLSL module '{source_path:?}'");

			let mut source_dest = source_path.clone().into_os_string();
			source_dest.push(".spv");

			// Check if recompilation is not needed
			if let Ok(source_meta) = std::fs::metadata(&source_path) {
				if let Ok(dest_meta) = std::fs::metadata(&source_dest) {
					let source_modified = source_meta.modified().unwrap();
					let dest_modified = dest_meta.modified().unwrap();

					if dest_modified > source_modified {
						let source = std::fs::read(&source_dest)
							.expect("failed to read file");
						return Ok(unsafe { device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
								label: source_path.to_str(), 
								source: wgpu::util::make_spirv_raw(&source[..]), 
						})})
					}
				}
			}

			warn!("Attempting to compile GLSL shaders to SPIRV using glslc");

			// Test if glslc is accessible
			match std::process::Command::new("glslc").arg("--version").status() {
				Ok(exit_status) => {
					if !exit_status.success() {
						return Err(ShaderError::CompilationGlslcError(format!("failed to run glslc, exit status {exit_status:?}")))
					}
				},
				Err(e) => return Err(ShaderError::CompilationGlslcError(format!("failed to run glslc, {e:?}"))),
			}

			let voutput = Command::new("glslc")
				.arg(&source_path)
				.arg("-o")
				.arg(&source_dest)
				.output()
				.expect("glslc command failed somehow");
			if !voutput.status.success() {
				let message = format!("Shader compilation terminated with code {}: {}", 
					voutput.status.code().unwrap(),
					// String::from_utf8_lossy(&voutput.stdout[..]),
					String::from_utf8_lossy(&voutput.stderr[..]),
				);
				error!("{message}");
				return Err(ShaderError::CompilationGlslcError(message))
			}

			let source = std::fs::read(&source_dest)
				.expect("failed to read file");

			Ok(unsafe { device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
				label: source_path.to_str(), 
				source: wgpu::util::make_spirv_raw(&source[..]), 
			})})
		},
		ShaderLanguage::Wgsl => {
			let source = std::fs::read_to_string(&source_path)
				.expect("failed to read file");
			
			Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: source_path.to_str(),
				source: wgpu::ShaderSource::Wgsl(source.into()),
			}))
		},
	}
}



fn needs_reload(last_loaded: SystemTime, prototype: &ShaderPrototype) -> std::io::Result<bool> {
	let spec_modified = std::fs::metadata(&prototype.specification_path)?.modified()?;
	let spec_parent = prototype.specification_path.parent().unwrap();

	// Find max of last modified in fs
	let last_modified = match &prototype.base {
		ShaderBaseDescriptor::Compute(c) => {
			let c_path = spec_parent.join(&c.path);
			let c_modified = std::fs::metadata(&c_path)?.modified()?;

			[spec_modified, c_modified].iter().max().unwrap().clone()
		},
		ShaderBaseDescriptor::Polygon { vertex, fragment, .. } => {
			let v_path = spec_parent.join(&vertex.path);
			let v_modified = std::fs::metadata(&v_path)?.modified()?;

			if let Some(fragment) = fragment {
				let f_path = spec_parent.join(&fragment.path);
				let f_modified = std::fs::metadata(&f_path)?.modified()?;
				[spec_modified, v_modified, f_modified].iter().max().unwrap().clone()
			} else {
				[spec_modified, v_modified].iter().max().unwrap().clone()
			}					
		}
	};
	
	Ok(last_modified > last_loaded)
}
