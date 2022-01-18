use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::{sync::Arc, num::NonZeroU32};
use std::collections::{HashMap, BTreeMap};
use wgpu;
use crate::render::*;



/*
What inputs can it take and what would it use them for?
*/



#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum VertexProperty {
	VertexPosition,
	VertexColour,
	VertexUV,
	VertexTextureID,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceProperty {
	InstanceModelMatrix,
	InstanceColour,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ShaderSourceType {
	Spirv,
	Glsl,
	Wgsl,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSource {
	pub file_type: ShaderSourceType,
	pub vertex_path: PathBuf,
	pub vertex_entry: String,
	pub fragment_path: PathBuf,
	pub fragment_entry: String,	
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Copy, Clone)]
pub enum BindingType {
	Buffer,
	Texture,
	TextureArray,
	ArrayTexture,
	Sampler,
	SamplerArray,
}

// Serializable shader information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSpecification {
	pub name: String,
	pub source: ShaderSource,
	pub vertex_inputs: Vec<VertexProperty>,
	pub instance_inputs: Vec<InstanceProperty>,
	//pub push_constants: Vec<u32>, // Unused for now
	pub bind_groups: BTreeMap<u32, Vec<(u32, String, BindingType)>>
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



#[derive(Debug, Clone)]
#[derive(Derivative)]
#[derivative(PartialEq, Eq, Hash)]
pub struct BindGroupEntryFormat {
	pub binding_type: BindingType,
	pub resource_usage: String,
	// I don't like using another crate's type, but I'll leave that for future me to rectify
	pub layout: wgpu::BindGroupLayoutEntry,
}
impl std::fmt::Display for BindGroupEntryFormat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "('{}', {:?})", self.resource_usage, self.binding_type)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindGroupFormat {
	pub binding_specifications: Vec<BindGroupEntryFormat>,
}
impl BindGroupFormat {
	pub fn create_bind_group_layout(
		&self,
		device: &wgpu::Device,
	) -> wgpu::BindGroupLayout {
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &self.binding_specifications.iter().map(|b| b.layout).collect::<Vec<_>>()[..],
			label: None,
		})
	}
}
impl std::fmt::Display for BindGroupFormat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "[")?;
		if self.binding_specifications.len() != 0 {
			if self.binding_specifications.len() > 1 {
				for bsi in 0..(self.binding_specifications.len()-1) {
					write!(f, "{}, ", self.binding_specifications[bsi])?;
				}
			}
			write!(f, "{}", self.binding_specifications[self.binding_specifications.len()-1])?
		}
		write!(f, "]")
	}
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ShaderBindGroup {
	pub format: BindGroupFormat,
	// If we exclude layout we can derive many things
	pub layout_idx: usize,
}

#[derive(Debug)]
pub struct Shader {
	pub name: String,
	pub path: PathBuf,
	pub vertex_properties: Vec<VertexProperty>,
	pub instance_properties: Vec<InstanceProperty>,
	pub bind_groups: BTreeMap<u32, ShaderBindGroup>,
	pub pipeline_layout: wgpu::PipelineLayout,
	pub pipeline: wgpu::RenderPipeline,
}
impl std::fmt::Display for Shader {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} ({:?}), vp: {:?}, ip: {:?}, {:#?}", &self.name, &self.path, &self.vertex_properties, &self.instance_properties, &self.bind_groups)
	}
}



#[derive(Debug)]
pub struct ShaderManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	shaders: Vec<Shader>,
	shaders_index_name: HashMap<String, usize>,
	shaders_index_path: HashMap<PathBuf, usize>,
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
			shaders_index_name: HashMap::new(), 
			shaders_index_path: HashMap::new(),
			bind_group_layouts: Vec::new(),
			bind_group_layouts_bind_group_format: HashMap::new(),
		}
	}

	pub fn register_path(
		&mut self,
		path: &PathBuf,
	) -> usize {
		info!("Registering shader from {:?}", path);
		let specification = ShaderSpecification::from_path(path);
		let shader = self.construct_shader(&specification, path);

		let idx = self.shaders.len();
		self.shaders_index_name.insert(shader.name.clone(), idx);
		self.shaders_index_path.insert(shader.path.clone(), idx);
		self.shaders.push(shader);
		idx
	}

	pub fn index(&self, i: usize) -> &Shader {
		&self.shaders[i]
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<usize> {
		if self.shaders_index_path.contains_key(path) {
			Some(self.shaders_index_path[path])
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

	pub fn bind_group_layout_index_bind_group_format(&self, bgf: &BindGroupFormat) -> Option<usize> {
		if self.bind_group_layouts_bind_group_format.contains_key(bgf) {
			Some(self.bind_group_layouts_bind_group_format[bgf])
		} else {
			None
		}
	}

	fn construct_shader(
		&mut self, 
		specification: &ShaderSpecification,
		specification_path: &PathBuf, // Not in specification because not loaded from file
	) -> Shader {
		let name = specification.name.clone();
		let path = specification_path.clone();
		let vertex_properties = specification.vertex_inputs.clone();
		let instance_properties = specification.instance_inputs.clone();

		let mut bind_groups = BTreeMap::new();
		// For each bind group
		for (i, group) in &specification.bind_groups {
			let mut bindings = Vec::new();
			// For each binding
			for (j, resource_thing, binding) in group {
				match binding {
					BindingType::Buffer => {
						let binding_type = BindingType::Buffer;
						let resource_usage = resource_thing.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: *j as u32,
							visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Buffer {
								ty: wgpu::BufferBindingType::Uniform,
								has_dynamic_offset: false,
								min_binding_size: None,
							},
							count: None,
						};
						bindings.push(BindGroupEntryFormat {
							binding_type, resource_usage, layout,
						});
					},
					BindingType::Texture => {
						let binding_type = BindingType::Texture;
						let resource_usage = resource_thing.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: *j as u32,
							visibility: wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Texture {
								sample_type: wgpu::TextureSampleType::Float { filterable: true },
								view_dimension: wgpu::TextureViewDimension::D2,
								multisampled: false,
							},
							count: None,
						};
						bindings.push(BindGroupEntryFormat {
							binding_type, resource_usage, layout,
						});
					},
					BindingType::TextureArray => {
						let binding_type = BindingType::TextureArray;
						let resource_usage = resource_thing.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: *j as u32,
							visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Texture {
								sample_type: wgpu::TextureSampleType::Float { filterable: true },
								view_dimension: wgpu::TextureViewDimension::D2,
								multisampled: false,
							},
							count: NonZeroU32::new(1024),
						};
						bindings.push(BindGroupEntryFormat {
							binding_type, resource_usage, layout,
						});
					},
					BindingType::ArrayTexture => {
						let binding_type = BindingType::ArrayTexture;
						let resource_usage = resource_thing.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: *j as u32,
							visibility: wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Texture {
								sample_type: wgpu::TextureSampleType::Float { filterable: true },
								view_dimension: wgpu::TextureViewDimension::D2Array,
								multisampled: false,
							},
							count: None,
						};
						bindings.push(BindGroupEntryFormat {
							binding_type, resource_usage, layout,
						});
					},
					BindingType::Sampler => {
						let binding_type = BindingType::Sampler;
						let resource_usage = resource_thing.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: *j as u32,
							visibility: wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Sampler {
								comparison: false,
								filtering: true,
							},
							count: None,
						};
						bindings.push(BindGroupEntryFormat {
							binding_type, resource_usage, layout,
						});
					},
					_ => todo!("Okay so I missed something here"),
				}
			}

			let bg_format = BindGroupFormat {
				binding_specifications: bindings,
			};
			let layout_idx = match self.bind_group_layout_index_bind_group_format(&bg_format) {
				Some(index) => index,
				None => self.bind_group_layout_create(&bg_format),
			};
			
			let location = *i as u32;
			let shader_bind_group = ShaderBindGroup {
				format: bg_format, 
				layout_idx,
			};
			bind_groups.insert(location, shader_bind_group);
		}

		let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &bind_groups.iter().map(|(_, bg)| self.bind_group_layout_index(bg.layout_idx)).collect::<Vec<_>>()[..],
			push_constant_ranges: &[],
		});

		let pipeline = self.shader_pipeline(specification, specification_path, &pipeline_layout);

		Shader {
			name, path, vertex_properties, instance_properties, bind_groups, pipeline_layout, pipeline,
		}
	}

	// Make pipeline from shader specification
	fn shader_pipeline(
		&self, 
		specification: &ShaderSpecification, 
		specification_path: &PathBuf,
		layout: &wgpu::PipelineLayout,
	) -> wgpu::RenderPipeline {
		let pipeline_label = format!("{} pipeline", specification.name);

		// Vertex input
		let mut vertex_attributes_length = 0;
		let mut vertex_attributes = Vec::new();
		for vertex_input in &specification.vertex_inputs {
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
					format,
				});
				vertex_attributes_length += size as wgpu::BufferAddress;
			}
		}
		let vertex_layout = wgpu::VertexBufferLayout {
			array_stride: vertex_attributes_length,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &vertex_attributes[..],
		};

		// Instance input
		let mut instance_attributes_length = 0;
		let mut instance_attributes = Vec::new();
		for instance_input in &specification.instance_inputs {
			use crate::render::vertex::*;
			let attribute_segment = match instance_input {
				InstanceProperty::InstanceModelMatrix => InstanceModelMatrix::attributes(),
				InstanceProperty::InstanceColour => InstanceColour::attributes(),
				_ => panic!("Unimplemented instance property"),
			};
			for (size, format) in attribute_segment {
				instance_attributes.push(wgpu::VertexAttribute {
					offset: instance_attributes_length,
					shader_location: (vertex_attributes.len() + instance_attributes.len()) as u32,
					format,
				});
				instance_attributes_length += size as wgpu::BufferAddress;
			}
		}
		let instance_layout = wgpu::VertexBufferLayout {
			array_stride: instance_attributes_length,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &instance_attributes[..],
		};

		// Vertex and instance input
		let vertex_buffer_layouts = &[vertex_layout, instance_layout];

		// Shader compilation
		let vertex_entry = &*specification.source.vertex_entry;
		let fragment_entry = &*specification.source.fragment_entry;
		// Todo: Test if these are the same file
		let specification_base = specification_path.parent().unwrap();
		let vpath = specification_base.join(&specification.source.vertex_path);
		let fpath = specification_base.join(&specification.source.fragment_path);
		let [vertex_source, fragment_source] = match specification.source.file_type {
			ShaderSourceType::Spirv => {
				let vsrc = std::fs::read(&vpath).expect("failed to read file");
				let fsrc = std::fs::read(&fpath).expect("failed to read file");
				[vsrc, fsrc]
			},
			ShaderSourceType::Glsl => {
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

				let mut vdest = vpath.clone().into_os_string();
				vdest.push(".spv");
				let mut fdest = fpath.clone().into_os_string();
				fdest.push(".spv");

				let voutput = Command::new("glslc")
					.arg(&vpath)
					.arg("-o")
					.arg(&vdest)
					.output()
					.expect("glslc command failed (vertex)");
				if !voutput.status.success() {
					error!("Vertex shader compilation terminated with code {}, output is as follows:", voutput.status.code().unwrap());
					io::stdout().write_all(&voutput.stdout).unwrap();
					io::stderr().write_all(&voutput.stderr).unwrap();
					panic!("Try again if you dare");
				}				

				let foutput = Command::new("glslc")
					.arg(&fpath)
					.arg("-o")
					.arg(&fdest)
					.output()
					.expect("glslc command failed (fragment)");
				if !foutput.status.success() {
					error!("Fragment shader compilation terminated with code {}, output is as follows:", foutput.status.code().unwrap());
					io::stdout().write_all(&foutput.stdout).unwrap();
					io::stderr().write_all(&foutput.stderr).unwrap();
					panic!("Try again if you dare");
				}

				let vsrc = std::fs::read(&vdest).expect("failed to read file");
				let fsrc = std::fs::read(&fdest).expect("failed to read file");
				[vsrc, fsrc]
			}
			_ => panic!("Unimplemented shader type!"),
		};
		
		info!("Vertex module");
		let vertex_shader_module = unsafe { self.device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
			label: vpath.to_str(), 
			source: wgpu::util::make_spirv_raw(&vertex_source[..]), 
		})};
		info!("Fragment module");
		let fragment_shader_module = unsafe { self.device.create_shader_module_spirv(
		&wgpu::ShaderModuleDescriptorSpirV { 
			label: vpath.to_str(), 
			source: wgpu::util::make_spirv_raw(&fragment_source[..]), 
		})};

		// The pipeline itself
		self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some(&*pipeline_label),
			layout: Some(layout),
			vertex: wgpu::VertexState {
				module: &vertex_shader_module,
				entry_point: vertex_entry,
				buffers: vertex_buffer_layouts,
			},
			fragment: Some(wgpu::FragmentState {
				module: &fragment_shader_module,
				entry_point: fragment_entry,
				targets: &[wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					blend: Some(wgpu::BlendState {
						alpha: wgpu::BlendComponent::REPLACE,
						color: wgpu::BlendComponent::REPLACE,
					}),
					write_mask: wgpu::ColorWrites::ALL,
				}],
			}),
			// Should we pass this as an optional function argument?
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: None, // Some(wgpu::Face::Back),
				polygon_mode: wgpu::PolygonMode::Fill, // "Line" for wireframe
				// Requires Features::DEPTH_CLAMPING
				clamp_depth: false,
				// Requires Features::CONSERVATIVE_RASTERIZATION
				conservative: false,
			},
			depth_stencil: Some(BoundTexture::DEPTH_FORMAT).map(|format| wgpu::DepthStencilState {
				format,
				depth_write_enabled: true,
				depth_compare: wgpu::CompareFunction::Less,
				stencil: wgpu::StencilState::default(),
				bias: wgpu::DepthBiasState::default(),
			}),
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
		})
	}
}



#[cfg(test)]
mod tests {
	use super::*;

	fn create_example_shader() -> ShaderSpecification {
		// ShaderSpecification {
		// 	name: "example".into(),
		// 	source: ShaderSource {
		// 		file_type: ShaderSourceType::Glsl,
		// 		vertex_path: "vertex.vert".into(),
		// 		vertex_entry: "main".into(),
		// 		fragment_path: "fragment.frag".into(),
		// 		fragment_entry: "main".into(),
		// 	},
		// 	vertex_inputs: vec!(VertexInput::Vertex, VertexInput::Instance),
		// 	bind_groups: vec!(
		// 		vec!(
		// 			BindingType::Buffer("camera uniform".into()),
		// 		),
		// 		vec!(
		// 			BindingType::Texture("albedo".into(), TextureBindingType::Texture),
		// 			BindingType::Sampler("albedo sampler".into()),
		// 		)
		// 	)
		// }
		todo!()
	}

	#[test]
	fn test_serialize() {
		let data = create_example_shader();
		let pretty = ron::ser::PrettyConfig::new()
			.depth_limit(3)
			.separate_tuple_members(true)
			.enumerate_arrays(false);
		let s = ron::ser::to_string_pretty(&data, pretty).expect("Serialization failed");
		println!("{}", s);
		assert!(true);
	}
}
