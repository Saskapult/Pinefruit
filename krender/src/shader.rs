use arrayvec::ArrayVec;
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};
use slotmap::{SlotMap, SparseSecondaryMap};
use thiserror::Error;
use std::path::{PathBuf, Path};
use std::time::SystemTime;
use std::num::NonZeroU32;
use std::collections::{HashMap, BTreeMap};

use crate::MaterialKey;
use crate::mesh::MeshManager;
use crate::prelude::BindGroupManager;
use crate::vertex::{VertexAttribute, InstanceAttribute};
use crate::{ShaderKey, BindGroupLayoutKey, texture::TextureFormat, MeshFormatKey};



/// We can have shader specification and shader prototype. 
/// The difference is that the prototype holds more information and is not serializable. 
/// Don't make supersets of any other types! 
#[derive(Debug)]
pub struct ShaderEntry {
	pub source: PathBuf,
	pub specification: ShaderSpecification,
	pub registered: SystemTime, 

	pub dependent_materials: Mutex<SparseSecondaryMap<MaterialKey, ()>>, // Set-style

	pub mesh_format_key: Option<MeshFormatKey>,
	pub bind_group_layout_keys: Option<[Option<BindGroupLayoutKey>; 4]>,
	pub pipeline_layout: Option<wgpu::PipelineLayout>,
	pub pipeline: Option<ShaderPipeline>,
}
impl ShaderEntry {
	pub fn from_path(path: impl Into<PathBuf>) -> Self {
		let path: PathBuf = path.into().canonicalize().unwrap();
		let specification = ShaderSpecification::read(&path).unwrap();

		Self {
			source: path,
			specification,
			registered: SystemTime::now(),
			dependent_materials: Mutex::new(SparseSecondaryMap::new()),
			mesh_format_key: None,
			bind_group_layout_keys: None,
			pipeline_layout: None,
			pipeline: None,
		}
	}

	pub fn add_dependent_material(&self, key: MaterialKey) {
		let mut dependent_materials = self.dependent_materials.lock();
		dependent_materials.insert(key, ());
	}

	pub fn remove_dependent_material(&self, key: MaterialKey) {
		let mut dependent_materials = self.dependent_materials.lock();
		dependent_materials.remove(key);
	}

	pub(self) fn register_modules(&self, modules: &mut ShaderModuleManager) {
		match &self.specification.base {
			ShaderBase::Compute(base) => modules.register_module(&base.module),
			ShaderBase::Polygonal(base) => {
				modules.register_module(&base.vertex);
				if let Some(fragment) = &base.fragment {
					modules.register_module(fragment);
				}
			}
		}
	}

	pub(self) fn register_layouts(&mut self, bind_groups: &mut BindGroupManager) {
		let mut bind_group_layout_keys = [None; 4];
		for (&i, t) in self.specification.bind_groups.iter() {
			let mut layout_entries = ArrayVec::new();
			t.iter().for_each(|(&i, entry)| layout_entries.try_push(entry.entry(i)).unwrap());
			let key = bind_groups.i_need_a_layout(layout_entries);
			bind_group_layout_keys[i as usize] = Some(key);
		}
		
		self.bind_group_layout_keys = Some(bind_group_layout_keys);
	}

	pub(self) fn register_mesh_format(&mut self, meshes: &mut MeshManager) {
		if let ShaderBase::Polygonal(base) = &self.specification.base {
			if let PolygonInput::Mesh(attributes) = &base.polygon_input {
				if self.mesh_format_key.is_none() {
					let key = meshes.format_new_or_create(attributes);
					self.mesh_format_key = Some(key);
				}
			}
		}
		
	}

	pub(self) fn make_pipeline(
		&mut self,
		device: &wgpu::Device,
		bind_groups: &BindGroupManager,
		modules: &ShaderModuleManager,
	) -> Result<(), ShaderError> {
		trace!("Fetch bind group layouts");
		let bind_group_layouts = self.bind_group_layout_keys.as_ref().unwrap().iter().copied()
			.filter_map(|g| g)
			.map(|key| bind_groups.layout(key).unwrap())
			.collect::<Vec<_>>();

		trace!("Create piepeline layout");
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some(&*format!("Pipeline layout for shader {:?}", self.specification.name)),
			bind_group_layouts: bind_group_layouts.as_slice(),
			push_constant_ranges: &[],
		});

		let pipeline = match &self.specification.base {
			ShaderBase::Compute(ComputeShaderBase { module }) => {	
				trace!("Create pipeline (compute)");
				ShaderPipeline::Compute(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
					label: Some(&*format!("Pipeline for shader {:?}", self.specification.name)),
					layout: Some(&pipeline_layout),
					module: modules.get_module(&module.path).unwrap(),
					entry_point: &*module.entry,
				}))
			},
			ShaderBase::Polygonal(base) => {
				trace!("Create pipeline (polygonal)");
				trace!("Collect vertex attributes");
				let mut shader_location = 0; // used for vertex and instance vertex locations
				
				let mut vertex_attributes = Vec::new();
				let mut vertex_attributes_length = 0;
				let vertex_layout = match &base.polygon_input {
					PolygonInput::Mesh(requested_attributes) => {
						for ad in requested_attributes.iter() {
							for &format in ad.fields.iter() {
								let format = format.into();
								vertex_attributes.push(wgpu::VertexAttribute {
									offset: vertex_attributes_length,
									shader_location: shader_location as u32,
									format,
								});
								vertex_attributes_length += format.size() as wgpu::BufferAddress;
								shader_location += 1;
							}
						}
						assert_ne!(0, vertex_attributes_length, "You probably want to extract some data from that mesh!");
						(vertex_attributes_length > 0).then(|| wgpu::VertexBufferLayout {
							array_stride: vertex_attributes_length,
							step_mode: wgpu::VertexStepMode::Vertex,
							attributes: vertex_attributes.as_slice(),
						})
					},
					PolygonInput::Generative(_) => None,
				};
				trace!("vertex_layout = {vertex_layout:?}");

				trace!("Collect instance attributes");
				let mut instance_attributes = Vec::new();
				let mut instance_attributes_length = 0;
				let instance_layout = {
					for ad in base.instance_attributes.iter() {
						for &format in ad.fields.iter() {
							let format = format.into();
							instance_attributes.push(wgpu::VertexAttribute {
								offset: instance_attributes_length,
								shader_location: shader_location as u32,
								format,
							});
							instance_attributes_length += format.size() as wgpu::BufferAddress;
							shader_location += 1;
						}
					}
					(instance_attributes_length > 0).then(|| wgpu::VertexBufferLayout {
						array_stride: instance_attributes_length,
						step_mode: wgpu::VertexStepMode::Instance,
						attributes: instance_attributes.as_slice(),
					})
				};
				trace!("instance_layout = {instance_layout:?}");
				
				// I did it this way because I am unsure of what I'm doing
				// If I did the wrong thing I want it to be easy to correct
				let vertex_buffer_layouts: ArrayVec<wgpu::VertexBufferLayout, 2> = match (vertex_layout, instance_layout) {
					(Some(vl), Some(il)) => [vl, il].into(),
					(Some(vl), None) => [
						vl,
						wgpu::VertexBufferLayout {
							array_stride: 0,
							step_mode: wgpu::VertexStepMode::Instance,
							attributes: &[],
						},
					].into(),
					(None, Some(il)) => [
						wgpu::VertexBufferLayout {
							array_stride: 0,
							step_mode: wgpu::VertexStepMode::Vertex,
							attributes: &[],
						},
						il,
					].into(),
					(None, None) => ArrayVec::new(),
				};
				trace!("vertex_buffer_layouts = {vertex_buffer_layouts:?}");

				trace!("Get vertex module");
				let vertex = wgpu::VertexState {
					module: modules.get_module(&base.vertex.path).unwrap(),
					entry_point: &base.vertex.entry,
					buffers: vertex_buffer_layouts.as_slice(),
				};

				trace!("Collect attachments");
				let attachments = base.attachments.iter().map(|a| {
					Some(wgpu::ColorTargetState {
						format: a.format.into(),
						blend: Some(wgpu::BlendState {
							alpha: a.blend_alpha.into(),
							color: a.blend_colour.into(),
						}),
						write_mask: wgpu::ColorWrites::ALL,
					})
				}).collect::<Vec<_>>();

				trace!("Get fragment module");
				let fragment = base.fragment.as_ref().and_then(|f| Some(wgpu::FragmentState {
					module: modules.get_module(&f.path).unwrap(),
					entry_point: &f.entry,
					targets: attachments.as_slice(),
				}));

				trace!("Collect depth");
				let depth_stencil = base.depth.as_ref().and_then(|depth| Some(depth.stencil_state()));

				trace!("Create render pipeline");
				ShaderPipeline::Polygon(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
					label: Some(&*format!("Pipeline for shader {:?}", self.specification.name)),
					layout: Some(&pipeline_layout),
					vertex,
					fragment,
					primitive: base.primitive(),
					depth_stencil,
					multisample: wgpu::MultisampleState {
						count: base.multisample_count,
						mask: !0,
						alpha_to_coverage_enabled: false,
					},
					multiview: None,
				}))
			},
		};

		self.pipeline_layout = Some(pipeline_layout);
		self.pipeline = Some(pipeline);

		Ok(())
	}

	/// Outdated iff something in {specification, modules} was modified after this entry was registered. 
	pub fn outdated(&self) -> std::io::Result<bool> {
		let spec_modified = std::fs::metadata(&self.source)?.modified()?;
		let spec_parent = self.source.parent().unwrap();
	
		// Find max of last modified in fs
		let last_modified = match &self.specification.base {
			ShaderBase::Compute(ComputeShaderBase { module }) => {
				let c_path = spec_parent.join(&module.path);
				let c_modified = std::fs::metadata(&c_path)?.modified()?;
	
				[spec_modified, c_modified].iter().max().unwrap().clone()
			},
			ShaderBase::Polygonal(PolygonalShaderBase { vertex, fragment, .. }) => {
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
		
		Ok(last_modified > self.registered)
	}
	
}


#[derive(Debug)]
pub enum ShaderPipeline {
	Polygon(wgpu::RenderPipeline),
	Compute(wgpu::ComputePipeline),
}
impl ShaderPipeline {
	pub fn polygon(&self) -> Option<&wgpu::RenderPipeline> {
		match self {
			Self::Polygon(pl) => Some(pl),
			_ => None,
		}
	}
	pub fn compute(&self) -> Option<&wgpu::ComputePipeline> {
		match self {
			Self::Compute(pl) => Some(pl),
			_ => None,
		}
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSpecification {
	pub name: String,
	pub base: ShaderBase,
	pub bind_groups: BTreeMap<u32, BTreeMap<u32, BindGroupEntry>>,
	pub push_constant_range: Vec<(Vec<ShaderStages>, u32, u32)>,
}
impl ShaderSpecification {
	pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		trace!("Reading '{:?}'", path.as_ref());
		let path = path.as_ref().canonicalize()?;
		let f = std::fs::File::open(&path)?;
		let s = ron::de::from_reader::<std::fs::File, Self>(f)?
			.canonicalize(path.parent().unwrap())?;
		Ok(s)
	}

	pub fn push_constant_ranges(&self) -> impl Iterator<Item = wgpu::PushConstantRange> + '_ {
		self.push_constant_range.iter().map(|(stages, st, en)| wgpu::PushConstantRange {
			stages: ShaderStages::to_stages(stages.as_slice()),
			range: *st..*en,
		})
	}

	fn canonicalize(mut self, context: impl AsRef<Path>) -> Result<Self, std::io::Error> {
		trace!("Canonicalize shader specification for {}", self.name);
		let context: &Path = context.as_ref();
		match &mut self.base {
			ShaderBase::Compute(base) => {
				trace!("Canonicalize compute module");
				base.module.path = context.join(&base.module.path).canonicalize()?;
			},
			ShaderBase::Polygonal(base) => {
				trace!("Canonicalize vertex module");
				base.vertex.path = context.join(&base.vertex.path).canonicalize()?;
				if let Some(f) = base.fragment.as_mut() {
					trace!("Canonicalize fragment module");
					f.path = context.join(&f.path).canonicalize()?;
				}
			}
		}
		Ok(self)
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub enum ShaderBase {
	Polygonal(PolygonalShaderBase),
	Compute(ComputeShaderBase),
}
impl ShaderBase {
	/// Will panic if the enum is not [ShaderBase::Polygonal].
	pub fn polygonal(&self) -> &PolygonalShaderBase {
		match self {
			Self::Polygonal(p) => p,
			_ => panic!(),
		}
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub struct PolygonalShaderBase {
	pub vertex: ShaderModule,
	pub fragment: Option<ShaderModule>,
	pub polygon_input: PolygonInput,
	pub polygon_mode: PolygonMode,
	pub instance_attributes: Vec<InstanceAttribute>,
	pub attachments: Vec<ShaderColourAttachment>,
	pub depth: Option<ShaderDepthAttachment>,
	pub multisample_count: u32,
	pub topology: Topology,
	pub face_culling: CullingMode,
	pub unclipped_depth: bool,
	pub conservative: bool,
}
impl PolygonalShaderBase {
	pub fn primitive(&self) -> wgpu::PrimitiveState {
		wgpu::PrimitiveState {
			topology: self.topology.into(),
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: self.face_culling.into(),
			unclipped_depth: self.unclipped_depth,
			polygon_mode: self.polygon_mode.into(),
			conservative: self.conservative,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Topology {
	PointList, LineList, LineStrip, TriangleList, TriangleStrip,
}
impl Into<wgpu::PrimitiveTopology> for Topology {
	fn into(self) -> wgpu::PrimitiveTopology {
		match self {
			Self::PointList => wgpu::PrimitiveTopology::PointList, 
			Self::LineList => wgpu::PrimitiveTopology::LineList, 
			Self::LineStrip => wgpu::PrimitiveTopology::LineStrip, 
			Self::TriangleList => wgpu::PrimitiveTopology::TriangleList, 
			Self::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PolygonMode {
	Fill, Line, Point, 
}
impl Into<wgpu::PolygonMode> for PolygonMode {
	fn into(self) -> wgpu::PolygonMode {
		match self {
			Self::Fill => wgpu::PolygonMode::Fill,
			Self::Line => wgpu::PolygonMode::Line,
			Self::Point => wgpu::PolygonMode::Point,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum CullingMode {
	None, Back, Front, 
}
impl Into<Option<wgpu::Face>> for CullingMode {
	fn into(self) -> Option<wgpu::Face> {
		match self {
			Self::None => None,
			Self::Back => Some(wgpu::Face::Back),
			Self::Front => Some(wgpu::Face::Front),
		}
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ComputeShaderBase {
	pub module: ShaderModule, 
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum AddressMode {
	ClampToEdge,
    Repeat,
    MirrorRepeat,
}
impl Into<wgpu::AddressMode> for AddressMode {
	fn into(self) -> wgpu::AddressMode {
		match self {
			Self::ClampToEdge => wgpu::AddressMode::ClampToEdge,
			Self::Repeat => wgpu::AddressMode::Repeat,
			Self::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum FilterMode {
	Nearest,
    Linear,
}
impl Into<wgpu::FilterMode> for FilterMode {
	fn into(self) -> wgpu::FilterMode {
		match self {
			Self::Nearest => wgpu::FilterMode::Nearest,
    		Self::Linear => wgpu::FilterMode::Linear,
		}
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub enum BindGroupEntry {
	UniformBuffer(String, Vec<ShaderStages>),
	UniformBuffers(Vec<String>, Vec<ShaderStages>),
	UniformBufferArray(String, u32, Vec<ShaderStages>),
	Texture(
		String, 
		TextureFormat, ViewDimension, bool, SampleType, 
		Vec<ShaderStages>,
	),
	Textures(
		Vec<String>, 
		TextureFormat, ViewDimension, bool, SampleType, 
		Vec<ShaderStages>,
	),
	TextureArray(
		String, u32,
		TextureFormat, ViewDimension, bool, SampleType, 
		Vec<ShaderStages>,
	),
	// Array versions?
	StorageBuffer(
		String, 
		bool,
		Vec<ShaderStages>,
	),
	StorageTexture(
		String, 
		TextureFormat, ViewDimension, bool,
		StorageTextureAccessType, 
		Vec<ShaderStages>,
	),
	Sampler(
		String, // Remove this please
		AddressMode, // out of bounds access
		FilterMode, // mag filter
		FilterMode, // min filter
		FilterMode, // mip filter
		f32, // lod min
		f32, // lod max
		Vec<ShaderStages>,
	)
}
impl BindGroupEntry {
	pub fn entry(&self, i: u32) -> wgpu::BindGroupLayoutEntry {
		match self {
			Self::UniformBuffer(_, stages) => wgpu::BindGroupLayoutEntry {
				binding: i,
				visibility: ShaderStages::to_stages(stages),
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			},
			Self::UniformBuffers(b, stages) => wgpu::BindGroupLayoutEntry {
				binding: i,
				visibility: ShaderStages::to_stages(stages),
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: Some(NonZeroU32::new(b.len() as u32).unwrap()),
			},
			Self::UniformBufferArray(_, count, stages) => wgpu::BindGroupLayoutEntry {
				binding: i,
				visibility: ShaderStages::to_stages(stages),
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: Some(NonZeroU32::new(*count).unwrap()),
			},
			Self::Texture(_, _, view_dimension, multisampled, sample_type, stages) => wgpu::BindGroupLayoutEntry {
				binding: i,
				visibility: ShaderStages::to_stages(stages),
				ty: wgpu::BindingType::Texture {
					sample_type: (*sample_type).into(),
					view_dimension: (*view_dimension).into(),
					multisampled: *multisampled,
				},
				count: None,
			},
			Self::Sampler(_, _, _, _, _, _, _, stages) => wgpu::BindGroupLayoutEntry {
				binding: i,
				visibility: ShaderStages::to_stages(stages),
				ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
				count: None,
			},
			Self::StorageBuffer(_, read_only, stages) => wgpu::BindGroupLayoutEntry {
				binding: i,
				visibility: ShaderStages::to_stages(stages),
				ty: wgpu::BindingType::Buffer { 
					ty: wgpu::BufferBindingType::Storage { read_only: *read_only }, 
					has_dynamic_offset: false, 
					min_binding_size: None, 
				},
				count: None,
			},
			_ => todo!(),
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum StorageTextureAccessType {
    WriteOnly,
    ReadOnly,
    ReadWrite,
}
impl Into<wgpu::StorageTextureAccess> for StorageTextureAccessType {
	fn into(self) -> wgpu::StorageTextureAccess {
		match self {
			Self::WriteOnly => wgpu::StorageTextureAccess::WriteOnly,
			Self::ReadOnly => wgpu::StorageTextureAccess::ReadOnly,
			Self::ReadWrite => wgpu::StorageTextureAccess::ReadWrite,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ViewDimension {
	D1, D2, D3, D2Array, Cube, CubeArray, 
}
impl Into<wgpu::TextureViewDimension> for ViewDimension {
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


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum SampleType {
	Float, Depth, Sint, Uint,
}
impl Into<wgpu::TextureSampleType> for SampleType {
	fn into(self) -> wgpu::TextureSampleType {
		match self {
			Self::Float => wgpu::TextureSampleType::Float { filterable: true },
			Self::Depth => wgpu::TextureSampleType::Depth,
			Self::Sint => wgpu::TextureSampleType::Sint,
			Self::Uint => wgpu::TextureSampleType::Uint,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ShaderStages {
	Vertex, Fragment, Compute
}
impl ShaderStages {
	fn to_stages(stages: &[ShaderStages]) -> wgpu::ShaderStages {
		stages.iter().fold(wgpu::ShaderStages::NONE, |a, v| a | match v {
			ShaderStages::Vertex => wgpu::ShaderStages::VERTEX,
			ShaderStages::Fragment => wgpu::ShaderStages::FRAGMENT,
			ShaderStages::Compute => wgpu::ShaderStages::COMPUTE,
		})
	}
}



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShaderModule {
	pub language: ShaderLanguage,
	pub path: PathBuf,
	pub entry: String,
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ShaderLanguage {
	Spirv,
	Glsl,
	Wgsl,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PolygonInput {
	Mesh(Vec<VertexAttribute>),
	Generative(u32),
}
impl PolygonInput {
	pub fn generative_vertices(&self) -> u32 {
		match self {
			&Self::Generative(v) => v,
			_ => panic!(),
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShaderColourAttachment {
	pub source: String,
	pub format: TextureFormat,
	pub blend_colour: BlendComponent,
	pub blend_alpha: BlendComponent,
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BlendComponent {
	Specific {
		src_factor: BlendFactor,
		dst_factor: BlendFactor,
		operation: BlendOperation,
	},
	Replace,
	Over,
}
impl Into<wgpu::BlendComponent> for BlendComponent {
	fn into(self) -> wgpu::BlendComponent {
		match self {
			Self::Specific { 
				src_factor, 
				dst_factor, 
				operation, 
			} => wgpu::BlendComponent {
				src_factor: src_factor.into(),
				dst_factor: dst_factor.into(),
				operation: operation.into(),
			},
			Self::Replace => wgpu::BlendComponent::REPLACE,
			Self::Over => wgpu::BlendComponent::OVER,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BlendFactor {
	Zero, One, OneMinusSrcAlpha,
}
impl Into<wgpu::BlendFactor> for BlendFactor {
	fn into(self) -> wgpu::BlendFactor {
		match self {
			Self::Zero => wgpu::BlendFactor::Zero,
			Self::One => wgpu::BlendFactor::One,
			Self::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BlendOperation {
	Add, Subtract, ReverseSubtract, Min, Max,
}
impl Into<wgpu::BlendOperation> for BlendOperation {
	fn into(self) -> wgpu::BlendOperation {
		match self {
			Self::Add => wgpu::BlendOperation::Add,
			Self::Subtract => wgpu::BlendOperation::Subtract,
			Self::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
			Self::Min => wgpu::BlendOperation::Min,
			Self::Max => wgpu::BlendOperation::Max,
		}
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderDepthAttachment {
	pub source: String,
	pub format: TextureFormat,
	pub write: bool,
	pub comparison: CompareFunction,
	// These are disabled because I don't want to make more wrapper types!
	// pub stencil: DefaultOption<IDK>,
	// pub bias: DefaultOption<IDK>,
}
impl ShaderDepthAttachment {
	pub fn stencil_state(&self) -> wgpu::DepthStencilState {
		wgpu::DepthStencilState {
			format: self.format.into(),
			depth_write_enabled: self.write,
			depth_compare: self.comparison.into(),
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}
	}
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum CompareFunction {
	Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always, 
}
impl Into<wgpu::CompareFunction> for CompareFunction {
	fn into(self) -> wgpu::CompareFunction {
		match self {
			Self::Never => wgpu::CompareFunction::Never, 
			Self::Less => wgpu::CompareFunction::Less, 
			Self::Equal => wgpu::CompareFunction::Equal, 
			Self::LessEqual => wgpu::CompareFunction::LessEqual, 
			Self::Greater => wgpu::CompareFunction::Greater, 
			Self::NotEqual => wgpu::CompareFunction::NotEqual, 
			Self::GreaterEqual => wgpu::CompareFunction::GreaterEqual, 
			Self::Always => wgpu::CompareFunction::Always, 
		}
	}
}


#[derive(Debug, Error)]
pub enum ShaderError {
	#[error("glslc error: {0}")]
	CompilationGlslcError(String),
	// #[error("failed to locate file: {0}")]
	// FileMissingError(PathBuf),
	#[error("module '{0}' not found in loaded modules")]
	MissingModuleError(PathBuf),
	// #[error("shader requires attribute '{0}' not found in loaded attributes")]
	// MissingAttributeError(String),
}


// Remember to only change index if shader is changed, not updated
#[derive(Debug, Default)]
pub struct ShaderManager {
	// Prototype, last prototype load, layout, ?pipeline
	shaders: SlotMap<ShaderKey, ShaderEntry>,
	shaders_index_by_name: HashMap<String, ShaderKey>,
	shaders_index_by_path: HashMap<PathBuf, ShaderKey>,

	modules: ShaderModuleManager,
}
impl ShaderManager {
	pub fn new() -> Self {
		Self {
			shaders: SlotMap::with_key(), 
			shaders_index_by_name: HashMap::new(), 
			shaders_index_by_path: HashMap::new(),
			modules: ShaderModuleManager::default(),
		}
	}

	pub fn read(&mut self, path: impl Into<PathBuf>) -> ShaderKey {
		self.insert(ShaderEntry::from_path(path))
	}

	pub fn insert(&mut self, entry: ShaderEntry) -> ShaderKey {
		// I *think* that this is borrow-safe, but we'll see if the borrow checker agrees
		self.shaders.insert_with_key(|k| {
			self.shaders_index_by_name.insert(entry.specification.name.clone(), k);
			self.shaders_index_by_path.insert(entry.source.clone(), k);
			entry.register_modules(&mut self.modules);
			entry
		})
	}

	pub fn get(&self, key: ShaderKey) -> Option<&ShaderEntry> {
		self.shaders.get(key)
	}

	/// Register mesh format, register bind group layouts
	pub(crate) fn load_and_register(
		&mut self,
		meshes: &mut MeshManager,
		bind_groups: &mut BindGroupManager,
	) {
		self.shaders.values_mut()
			.for_each(|s| {
				if s.bind_group_layout_keys.is_none() {
					info!("Register bind group layouts for shader {}", s.specification.name);
					s.register_layouts(bind_groups);
				}
				info!("Register mesh format for shader {}", s.specification.name);
				s.register_mesh_format(meshes);
			});
	}
	
	pub(crate) fn build_pipelines(&mut self, device: &wgpu::Device, bind_groups: &BindGroupManager) {
		// This is wild, wtf
		for module_path in self.modules.outdated_modules() {
			self.modules.load_module(device, &module_path, self.modules.get_language(&module_path).unwrap());
		}

		self.shaders.values_mut()
			.filter(|s| s.pipeline.is_none())
			.for_each(|s| {
				info!("Creating pipeline for shader {}", s.specification.name);
				s.make_pipeline(device, bind_groups, &self.modules).unwrap();
			});
	}
	
	pub fn index_from_path(&self, path: &PathBuf) -> Option<ShaderKey> {
		self.shaders_index_by_path.get(&path.canonicalize()
			.expect(&*format!("Could not canonicalize {path:?}"))).cloned()
	}
}


#[derive(Debug, Default)]
struct ShaderModuleManager {
	// Path -> Language, ?(Module, LoadedAt)
	shader_modules: HashMap<PathBuf, (ShaderLanguage, Option<(wgpu::ShaderModule, SystemTime)>)>,
}
impl ShaderModuleManager {
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

	fn register_module(&mut self, smd: &ShaderModule) {
		if !self.shader_modules.contains_key(&smd.path) {
			debug!("Registering new shader module '{:?}'", smd.path);
			self.shader_modules.insert(smd.path.clone(), (smd.language, None));
		}
	}

	fn load_module(&mut self, device: &wgpu::Device, module_path: &PathBuf, module_language: ShaderLanguage) {
		let t = SystemTime::now();
		let m = load_shader_module(device, module_path, module_language).unwrap();

		let module_path = module_path.canonicalize().unwrap();
		self.shader_modules.insert(module_path, (module_language, Some((m, t))));
	}
	
	fn get_module(&self, module_path: &PathBuf) -> Option<&wgpu::ShaderModule> {
		self.shader_modules.get(module_path).and_then(|(_, m)| m.as_ref().and_then(|(m, _)| Some(m)))
	}
	
	fn get_language(&self, module_path: &PathBuf) -> Option<ShaderLanguage> {
		self.shader_modules.get(module_path).and_then(|(m, _)| Some(m)).cloned()
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
