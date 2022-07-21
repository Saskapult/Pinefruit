use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};
use std::{sync::Arc, num::NonZeroU32};
use std::collections::{HashMap, BTreeMap};
use wgpu;
use crate::render::*;



/*
What inputs can it take and what would it use them for?
*/



/// Serializable shader information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSpecification {
	pub name: String,
	pub source: ShaderSpecificationSource,
	pub bind_groups: BTreeMap<u32, BTreeMap<u32, (String, BindingType)>>
}
impl ShaderSpecification {
	pub fn from_path(
		path: &PathBuf,
	) -> Self {
		let f = std::fs::File::open(path).expect("Failed to open file");
		let info: ShaderSpecification = ron::de::from_reader(f).expect("Failed to read shader ron file");
		info
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ShaderLanguage {
	Spirv,
	Glsl,
	Wgsl,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSpecificationSource {
	pub language: ShaderLanguage,
	pub base: ShaderSpecificationBase,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ShaderSpecificationBase {
	Compute(ShaderSourceFile),
	Polygon{
		vertex: ShaderSourceFile, 
		fragment: Option<ShaderSourceFile>,
		inputs: Option<PolygonBehaviour>,
		attachments: Vec<ShaderAttatchmentSpecification>,
		multisample_count: u32,
	},
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShaderSourceFile {
	pub path: PathBuf,
	pub entry: String,
}


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

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Copy, Clone)]
pub enum BindingType {
	Buffer,
	Texture,
	TextureArray,
	ArrayTexture,
	Sampler,
	SamplerArray,
	StorageTexture(TextureType),
	StorageBuffer,
}






#[derive(Debug)]
pub struct Shader {
	pub name: String,
	pub specification_path: PathBuf,
	pub pipeline_layout: wgpu::PipelineLayout,
	pub pipeline: ShaderPipeline,
	pub bind_groups: BTreeMap<u32, ShaderBindGroup>,
}
// impl std::fmt::Display for Shader {
// 	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// 		write!(f, "{} ({:?}), vp: {:?}, ip: {:?}, {:#?}", &self.name, &self.specification_path, &self.mesh_format.vertex_properties, &self.mesh_format.instance_properties, &self.bind_groups)
// 	}
// }

#[derive(Debug)]
pub enum ShaderPipeline {
	Compute(wgpu::ComputePipeline),
	Polygon{
		pipeline: wgpu::RenderPipeline,
		mesh_format: Option<ShaderMeshFormat>,
		attachments: Vec<ShaderAttatchmentSpecification>,
	},
}

#[derive(Debug)]
pub struct ShaderMeshFormat {
	vertex_properties: Vec<VertexProperty>,
	instance_properties: Vec<InstanceProperty>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ShaderBindGroup {
	pub entries: Vec<ShaderBindGroupEntry>,
	pub layout_idx: usize,
}
impl ShaderBindGroup {
	pub fn format(&self) -> BindGroupFormat {
		BindGroupFormat::from_entries(&self.entries)
	}
}

/// An entry into a bind group coupled with its wgpu layout
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ShaderBindGroupEntry {
	pub format: BindGroupEntryFormat,
	pub layout: wgpu::BindGroupLayoutEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BindGroupEntryFormat {
	pub binding_type: BindingType,
	pub resource_usage: String,
}
impl BindGroupEntryFormat {
	pub fn layout_at(&self, i: u32) -> wgpu::BindGroupLayoutEntry {
		match self.binding_type {
			BindingType::Buffer => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}
			},
			BindingType::Texture => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				}
			}
			BindingType::TextureArray => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: NonZeroU32::new(1024),
				}
			}
			BindingType::ArrayTexture => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2Array,
						multisampled: false,
					},
					count: None,
				}
			}
			BindingType::Sampler => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler (
						wgpu::SamplerBindingType::Filtering,
					),
					count: None,
				}
			}
			BindingType::StorageTexture(format) => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
					ty: wgpu::BindingType::StorageTexture {
						access: wgpu::StorageTextureAccess::ReadWrite,
						format: format.translate(),
						view_dimension: wgpu::TextureViewDimension::D2,
					},
					count: None,
				}
			},
			BindingType::StorageBuffer => {
				wgpu::BindGroupLayoutEntry {
					binding: i,
					visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Storage { read_only: false },
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}
			},
			_ => panic!(),
		}
	}
}
impl std::fmt::Display for BindGroupEntryFormat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "('{}', {:?})", self.resource_usage, self.binding_type)
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







/// A map of binding locations to their types and usages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BindGroupFormat {
	pub entry_formats: BTreeMap<u32, BindGroupEntryFormat>,
}
impl BindGroupFormat {
	pub fn empty() -> Self {
		Self {
			entry_formats: BTreeMap::new(),
		}
	}
	pub fn from_entries(entries: &Vec<ShaderBindGroupEntry>) -> Self {
		Self {
			entry_formats: entries.iter().map(|e| (e.layout.binding, e.format.clone())).collect::<BTreeMap<_, _>>(),
		}
	}
	pub fn create_bind_group_layout(&self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &self.entry_formats.iter().map(|(i, b)| {
				b.layout_at(*i)
			}).collect::<Vec<_>>()[..],
			label: Some(&*format!("BGL for {}", &self)),
		})
	}
}
impl std::fmt::Display for BindGroupFormat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let g = self.entry_formats.iter().map(|(_, v)| format!("{v}")).collect::<Vec<_>>().join(", ");
		write!(f, "[{g}]")
	}
}






#[derive(Debug)]
pub struct ShaderManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	shaders: Vec<Shader>,
	shaders_loaded_at: Vec<SystemTime>,
	shaders_index_by_name: HashMap<String, usize>,
	shaders_index_by_path: HashMap<PathBuf, usize>,
	bind_group_layouts: Vec<wgpu::BindGroupLayout>,
	bind_group_layouts_bind_group_format: HashMap<BindGroupFormat, usize>,
}
impl ShaderManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(), 
			shaders: Vec::new(), 
			shaders_loaded_at: Vec::new(),
			shaders_index_by_name: HashMap::new(), 
			shaders_index_by_path: HashMap::new(),
			bind_group_layouts: Vec::new(),
			bind_group_layouts_bind_group_format: HashMap::new(),
		}
	}

	pub fn check_reload(&mut self) -> anyhow::Result<()> {
		info!("Running shader refresh");
		for i in 0..self.shaders.len() {
			let last_loaded = self.shaders_loaded_at[i];

			let spec_path = self.index(i).specification_path.clone();
			let specification = ShaderSpecification::from_path(&spec_path);
			let spec_modified = std::fs::metadata(&spec_path)?.modified()?;
			let spec_parent = spec_path.parent().unwrap();

			// Find max of last modified in fs
			let last_modified = match &specification.source.base {
				ShaderSpecificationBase::Compute(c) => {
					let c_path = spec_parent.join(&c.path);
					let c_modified = std::fs::metadata(&c_path)?.modified()?;

					[spec_modified, c_modified].iter().max().unwrap().clone()
				},
				ShaderSpecificationBase::Polygon { vertex, fragment, .. } => {
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
			
			if last_modified > last_loaded {
				info!("Reloading shader {spec_path:?}");

				let specification = ShaderSpecification::from_path(&spec_path);
				let new_shader = self.construct_shader(&specification, &spec_path);

				self.shaders[i] = new_shader;
				self.shaders_loaded_at[i] = last_modified;
			}
		}
		
		Ok(())
	}

	pub fn register_path(
		&mut self,
		path: &PathBuf,
	) -> usize {
		info!("Registering shader from {:?}", path);
		let specification = ShaderSpecification::from_path(path);
		let shader = self.construct_shader(&specification, path);

		let idx = self.shaders.len();
		self.shaders_loaded_at.push(SystemTime::now());
		self.shaders_index_by_name.insert(shader.name.clone(), idx);
		self.shaders_index_by_path.insert(shader.specification_path.clone(), idx);
		self.shaders.push(shader);
		idx
	}

	pub fn index(&self, i: usize) -> &Shader {
		&self.shaders[i]
	}

	pub fn index_from_path(&self, path: &PathBuf) -> Option<usize> {
		if self.shaders_index_by_path.contains_key(path) {
			Some(self.shaders_index_by_path[path])
		} else {
			None
		}
	}

	pub fn bind_group_layout_create(&mut self, format: &BindGroupFormat) -> usize {
		info!("Creating bind group layout for format '{}'", format);
		let layout = format.create_bind_group_layout(&self.device);
		let idx = self.bind_group_layouts.len();
		self.bind_group_layouts.push(layout);
		self.bind_group_layouts_bind_group_format.insert(format.clone(), idx);
		idx
	}

	pub fn bind_group_layout_index(&self, i: usize) -> &wgpu::BindGroupLayout {
		&self.bind_group_layouts[i]
	}

	pub fn bind_group_layout_index_from_bind_group_format(&self, bgf: &BindGroupFormat) -> Option<usize> {
		if self.bind_group_layouts_bind_group_format.contains_key(bgf) {
			Some(self.bind_group_layouts_bind_group_format[bgf])
		} else {
			None
		}
	}

	/// Makes shader bind group from shader specifcation part
	fn get_sbg(&mut self, group_spec: &BTreeMap<u32, (String, BindingType)>) -> ShaderBindGroup {
		let mut entries = Vec::new();
		for (j, (resource_thing, binding_type)) in group_spec {
			let resource_usage = resource_thing.clone();
			let format = BindGroupEntryFormat {
				binding_type: binding_type.clone(), resource_usage,
			};
			let layout = format.layout_at(*j);
			entries.push(ShaderBindGroupEntry {
				format, layout,
			});
		}

		let bg_format = BindGroupFormat::from_entries(&entries);

		let layout_idx = match self.bind_group_layout_index_from_bind_group_format(&bg_format) {
			Some(index) => index,
			None => self.bind_group_layout_create(&bg_format),
		};

		ShaderBindGroup {
			entries, 
			layout_idx,
		}
	}

	/// Makes shader from specification
	fn construct_shader(
		&mut self, 
		specification: &ShaderSpecification,
		specification_path: &PathBuf, // Needed for relative file paths
	) -> Shader {
		let name = specification.name.clone();
		let specification_path = specification_path.clone();

		let mut bind_groups = BTreeMap::new();
		for (i, group) in &specification.bind_groups {
			let shader_bind_group = self.get_sbg(group);
			let location = *i as u32;
			bind_groups.insert(location, shader_bind_group);
		}
		let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &bind_groups.iter().map(|(_, bg)| self.bind_group_layout_index(bg.layout_idx)).collect::<Vec<_>>()[..],
			push_constant_ranges: &[],
		});

		let pipeline = match &specification.source.base {
			ShaderSpecificationBase::Compute(source_file) => {
				let specification_context = specification_path.parent().unwrap();
				let g = specification_context.join(&source_file.path);

				let module = self.make_shader_module(&g, specification.source.language);

				let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
					label: Some(&*format!("{name} compute pipeline")),
					layout: Some(&pipeline_layout),
					module: &module,
					entry_point: &*source_file.entry,
				});
				
				ShaderPipeline::Compute(pipeline)
			},
			ShaderSpecificationBase::Polygon { 
				vertex, 
				fragment, 
				inputs: polygon_behaviour, 
				attachments,
				multisample_count,
			}=> {
				
				let mesh_format = polygon_behaviour.as_ref().and_then(|pb| {
					Some(ShaderMeshFormat{
						vertex_properties: pb.vertex_properties.clone(),
						instance_properties: pb.instance_properties.clone(),
					})
				});
				let pipeline = self.polygon_pipeline(
					&name,
					&specification_path,
					specification.source.language,
					vertex.clone(),
					fragment.clone(),
					polygon_behaviour.clone(),
					attachments.clone(),
					&pipeline_layout,
				);

				ShaderPipeline::Polygon {
					pipeline, 
					mesh_format, 
					attachments: attachments.clone(), 
				}
			},
		};

		Shader {
			name, specification_path, bind_groups, pipeline_layout, pipeline,
		}
	}

	fn make_shader_module(
		&self, 
		source_path: &PathBuf, 
		source_type: ShaderLanguage,
	) -> wgpu::ShaderModule {
		match source_type {
			ShaderLanguage::Spirv => {
				let source = std::fs::read(&source_path).expect("failed to read file");

				unsafe { self.device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
					label: source_path.to_str(), 
					source: wgpu::util::make_spirv_raw(&source[..]), 
				})}
			},
			ShaderLanguage::Glsl => {
				use std::io::{self, Write};
				use std::process::Command;

				warn!("Attempting to compile GLSL shaders to SPIRV using glslc");

				// Test if glslc is accessible
				match std::process::Command::new("glslc").arg("--version").status() {
					Ok(exit_status) => {
						if !exit_status.success() {
							panic!("glslc seems wrong!")
						}
					},
					Err(_) => panic!("glslc cannot run!"),
				}

				let mut source_dest = source_path.clone().into_os_string();
				source_dest.push(".spv");

				let voutput = Command::new("glslc")
					.arg(&source_path)
					.arg("-o")
					.arg(&source_dest)
					.output()
					.expect("glslc command failed (vertex)");
				if !voutput.status.success() {
					error!("Shader compilation terminated with code {}, output is as follows:", voutput.status.code().unwrap());
					io::stdout().write_all(&voutput.stdout).unwrap();
					io::stderr().write_all(&voutput.stderr).unwrap();
					panic!("Try again if you dare");
				}

				let source = std::fs::read(&source_dest)
					.expect("failed to read file");

				unsafe { self.device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
					label: source_path.to_str(), 
					source: wgpu::util::make_spirv_raw(&source[..]), 
				})}
			},
			ShaderLanguage::Wgsl => {
				let source = std::fs::read_to_string(&source_path)
					.expect("failed to read file");
				
				self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
					label: source_path.to_str(),
					source: wgpu::ShaderSource::Wgsl(source.into()),
				})
			},
			_ => {
				error!("Unimplemented shader language: {source_type:?}");
				panic!("Unimplemented shader language!");
			},
		}
	}

	/// Makes pipeline from shader specification
	fn polygon_pipeline(
		&self, 
		spec_name: &String,
		specification_path: &PathBuf,
		language: ShaderLanguage,
		vertex: ShaderSourceFile, 
		fragment: Option<ShaderSourceFile>,
		polygon_behaviour: Option<PolygonBehaviour>,
		attachments: Vec<ShaderAttatchmentSpecification>,
		layout: &wgpu::PipelineLayout,
	) -> wgpu::RenderPipeline {
		let pipeline_label = format!("{spec_name} pipeline");

		// Mesh input
		let (
			vertex_attributes, vertex_attributes_length,
			instance_attributes, instance_attributes_length,
		) = if let Some(mesh_format) = polygon_behaviour.as_ref() {
			// Vertex input
			let mut vertex_attributes_length = 0;
			let mut vertex_attributes = Vec::new();
			for vertex_input in &mesh_format.vertex_properties {
				use crate::render::vertex::*;
				let attribte_segment = match vertex_input {
					VertexProperty::VertexPosition => VertexPosition::attributes(),
					VertexProperty::VertexColour => VertexColour::attributes(),
					VertexProperty::VertexUV => VertexUV::attributes(),
					_ => panic!("Unimplemented vertex property"),
				};
				for (size, format) in attribte_segment {
					vertex_attributes.push(wgpu::VertexAttribute {
						offset: vertex_attributes_length,
						shader_location: vertex_attributes.len() as u32,
						format: *format,
					});
					vertex_attributes_length += *size as wgpu::BufferAddress;
				}
			}
	
			// Instance input
			let mut instance_attributes_length = 0;
			let mut instance_attributes = Vec::new();
			for instance_input in &mesh_format.instance_properties {
				use crate::render::vertex::*;
				let attribute_segment = match instance_input {
					InstanceProperty::InstanceModelMatrix => InstanceModelMatrix::attributes(),
					InstanceProperty::InstanceColour => InstanceColour::attributes(),
				};
				for (size, format) in attribute_segment {
					instance_attributes.push(wgpu::VertexAttribute {
						offset: instance_attributes_length,
						shader_location: (vertex_attributes.len() + instance_attributes.len()) as u32,
						format: *format,
					});
					instance_attributes_length += *size as wgpu::BufferAddress;
				}
			}
			
			(
				vertex_attributes, vertex_attributes_length,
				instance_attributes, instance_attributes_length,
			)
		} else {
			(
				vec![], 0,
				vec![], 0,
			)
		};
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
		let vertex_buffer_layouts = &[vertex_layout, instance_layout];

		// Depth input
		let depth_stencil = polygon_behaviour.as_ref().and_then(|svi| {
			svi.depth_write.and_then(|depth_write_enabled| {
				Some(wgpu::DepthStencilState {
					format: BoundTexture::DEPTH_FORMAT,
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

		// Shader compilation
		let specification_context = specification_path.parent().unwrap();
		
		let vertex_entry = &*vertex.entry;
		let vpath = specification_context.join(&vertex.path);
		let vertex_module = self.make_shader_module(&vpath, language);
		let vertex = wgpu::VertexState {
			module: &vertex_module,
			entry_point: vertex_entry,
			buffers: vertex_buffer_layouts,
		};

		let fragment_data = fragment.and_then(|stuff| {
			let fragment_entry = stuff.entry;
			let fpath = specification_context.join(&stuff.path);
			let fragment_module = if vpath == fpath {
				info!("Fragment shader is in same file, saving work");
				None
			} else {
				Some(self.make_shader_module(&fpath, language))
			};

			Some((fragment_entry, fpath, fragment_module))
		});
		let fragment = fragment_data.as_ref().and_then(|(entry, _, module)| {

			Some(wgpu::FragmentState {
				module: module.as_ref().unwrap_or(&vertex_module),
				entry_point: &*entry,
				targets: &attachments[..],
			})
		});

		self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some(&*pipeline_label),
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
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			multiview: None,
		})
	}
}
