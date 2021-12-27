use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::{sync::Arc, num::NonZeroU32};
use std::collections::HashMap;
use wgpu;
use crate::render::*;





#[derive(Debug, Serialize, Deserialize)]
pub enum TextureBindingType {
	Texture,
	TextureArray,
	ArrayTexture,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum VertexInput {
	Vertex,
	TexturedVertex,
	Instance,
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

#[derive(Debug, Serialize, Deserialize)]
pub enum ShaderBinding {
	Buffer(String), // "camera"
	Texture(String, TextureBindingType), // "albedo"
	Sampler(String), // "albedo sampler"
}

// Serializable shader information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderSpecification {
	pub name: String,
	pub source: ShaderSource,
	pub vertex_inputs: Vec<VertexInput>,
	pub groups: Vec<Vec<ShaderBinding>>
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



pub enum BindingType {
	Texture,
	TextureArray,
	ArrayTexture,
	Buffer,
	Sampler,
}
pub struct ShaderBindGroupEntry {
	pub binding_type: BindingType, // Buffer, texture, sampler
	pub resource_id: String, // main camera, texture001, albedo sampler, etc
	pub layout: wgpu::BindGroupLayoutEntry,
}
pub struct ShaderBindGroup {
	pub bindings: Vec<ShaderBindGroupEntry>,
	pub location: u32,
	pub layout: wgpu::BindGroupLayout,
}
pub struct Shader {
	pub name: String,
	pub path: PathBuf,
	pub vertex_inputs: Vec<VertexInput>,
	pub bind_groups: Vec<ShaderBindGroup>,
	pub pipeline_layout: wgpu::PipelineLayout,
	pub pipeline: wgpu::RenderPipeline,
}



pub struct ShaderManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	pub shaders: Vec<Shader>,
	pub index_name: HashMap<String, usize>,
	pub index_path: HashMap<PathBuf, usize>,
}
impl ShaderManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();
		let shaders = Vec::new();
		let index_name = HashMap::new();
		let index_path = HashMap::new();
		Self {
			device, queue, shaders, index_name, index_path,
		}
	}

	pub fn register_path(
		&mut self,
		path: &PathBuf,
	) -> usize {
		let specification = ShaderSpecification::from_path(path);
		let shader = self.construct_shader(&specification, path);

		let idx = self.shaders.len();
		self.index_name.insert(shader.name.clone(), idx);
		self.index_path.insert(shader.path.clone(), idx);
		self.shaders.push(shader);
		idx
	}

	fn construct_shader(
		&self, 
		specification: &ShaderSpecification,
		path: &PathBuf, // I forgor
	) -> Shader {
		let name = specification.name.clone();
		let path = path.clone();
		let vertex_inputs = specification.vertex_inputs.clone();
		let mut bind_groups = Vec::new();
		// For each bind group
		for (i, group) in specification.groups.iter().enumerate() {
			let mut bindings = Vec::new();
			// For each binding
			for (j, binding) in group.iter().enumerate() {
				match binding {
					ShaderBinding::Buffer(bufferinfo) => {
						let binding_type = BindingType::Buffer;
						let resource_id = bufferinfo.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: j as u32,
							visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Buffer {
								ty: wgpu::BufferBindingType::Uniform,
								has_dynamic_offset: false,
								min_binding_size: None,
							},
							count: None,
						};
						bindings.push(ShaderBindGroupEntry {
							binding_type, resource_id, layout,
						});
					},
					ShaderBinding::Texture(textureinfo, binding_type) => {
						match binding_type {
							TextureBindingType::Texture => {
								let binding_type = BindingType::Texture;
								let resource_id = textureinfo.clone();
								let layout = wgpu::BindGroupLayoutEntry {
									binding: j as u32,
									visibility: wgpu::ShaderStages::FRAGMENT,
									ty: wgpu::BindingType::Texture {
										sample_type: wgpu::TextureSampleType::Float { filterable: true },
										view_dimension: wgpu::TextureViewDimension::D2,
										multisampled: false,
									},
									count: None,
								};
								bindings.push(ShaderBindGroupEntry {
									binding_type, resource_id, layout,
								});
							},
							TextureBindingType::TextureArray => {
								let binding_type = BindingType::TextureArray;
								let resource_id = textureinfo.clone();
								let layout = wgpu::BindGroupLayoutEntry {
									binding: j as u32,
									visibility: wgpu::ShaderStages::FRAGMENT,
									ty: wgpu::BindingType::Texture {
										sample_type: wgpu::TextureSampleType::Float { filterable: true },
										view_dimension: wgpu::TextureViewDimension::D2,
										multisampled: false,
									},
									count: NonZeroU32::new(1024),
								};
								bindings.push(ShaderBindGroupEntry {
									binding_type, resource_id, layout,
								});
							},
							TextureBindingType::ArrayTexture => {
								let binding_type = BindingType::ArrayTexture;
								let resource_id = textureinfo.clone();
								let layout = wgpu::BindGroupLayoutEntry {
									binding: j as u32,
									visibility: wgpu::ShaderStages::FRAGMENT,
									ty: wgpu::BindingType::Texture {
										sample_type: wgpu::TextureSampleType::Float { filterable: true },
										view_dimension: wgpu::TextureViewDimension::D2Array,
										multisampled: false,
									},
									count: None,
								};
								bindings.push(ShaderBindGroupEntry {
									binding_type, resource_id, layout,
								});
							},
						}
					},
					ShaderBinding::Sampler(samplerinfo) => {
						let binding_type = BindingType::Sampler;
						let resource_id = samplerinfo.clone();
						let layout = wgpu::BindGroupLayoutEntry {
							binding: j as u32,
							visibility: wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Sampler {
								comparison: false,
								filtering: true,
							},
							count: None,
						};
						bindings.push(ShaderBindGroupEntry {
							binding_type, resource_id, layout,
						});
					},
				}
			}

			// Todo: Test if it already exists
			let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &bindings.iter().map(|b| b.layout).collect::<Vec<_>>()[..],
				label: None,
			});
			let location = i as u32;
			let shader_bind_group = ShaderBindGroup {
				bindings, 
				location, 
				layout,
			};
			bind_groups.push(shader_bind_group);
		}

		let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &bind_groups.iter().map(|bg| &bg.layout).collect::<Vec<_>>()[..],
			push_constant_ranges: &[],
		});

		let pipeline = self.shader_pipeline(specification, &pipeline_layout);

		Shader {
			name, path, vertex_inputs, bind_groups, pipeline_layout, pipeline,
		}
	}

	fn shader_pipeline(
		&self, 
		specification: &ShaderSpecification, 
		layout: &wgpu::PipelineLayout,
	) -> wgpu::RenderPipeline {
		let mut vertex_layouts = Vec::new();
		for vertex_input in &specification.vertex_inputs {
			let vbl = match vertex_input {
				VertexInput::Instance => InstanceRaw::desc(),
				VertexInput::Vertex => Vertex::desc(),
				VertexInput::TexturedVertex => Vertex::desc(),
			};
			vertex_layouts.push(vbl);
		}

		let vertex_entry = &*specification.source.vertex_entry;
		let fragment_entry = &*specification.source.fragment_entry;
		// Todo: Test if these are the same file
		let vsrc = std::fs::read(&specification.source.vertex_path).expect("failed to read file");
		let fsrc = std::fs::read(&specification.source.fragment_path).expect("failed to read file");
		let [vshader, fshader] = match specification.source.file_type {
			ShaderSourceType::Spirv => {
				let vert = unsafe { self.device.create_shader_module_spirv( &wgpu::ShaderModuleDescriptorSpirV { 
					label: specification.source.vertex_path.to_str(), 
					source: wgpu::util::make_spirv_raw(&vsrc[..]), 
				})};
				let frag = unsafe { self.device.create_shader_module_spirv(
				&wgpu::ShaderModuleDescriptorSpirV { 
					label: specification.source.fragment_path.to_str(), 
					source: wgpu::util::make_spirv_raw(&fsrc[..]), 
				})};
				[vert, frag]
			},
			_ => panic!("Non-spirv shaders are not done yet"),
		};

		let pipeline_label = format!("{} pipeline", specification.name);

		self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some(&*pipeline_label),
			layout: Some(layout),
			vertex: wgpu::VertexState {
				module: &vshader,
				entry_point: vertex_entry,
				buffers: &vertex_layouts[..],
			},
			fragment: Some(wgpu::FragmentState {
				module: &fshader,
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
			depth_stencil: Some(Texture::DEPTH_FORMAT).map(|format| wgpu::DepthStencilState {
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
		ShaderSpecification {
			name: "example".into(),
			source: ShaderSource {
				file_type: ShaderSourceType::Glsl,
				vertex_path: "vertex.vert".into(),
				vertex_entry: "main".into(),
				fragment_path: "fragment.frag".into(),
				fragment_entry: "main".into(),
			},
			vertex_inputs: vec!(VertexInput::Vertex, VertexInput::Instance),
			groups: vec!(
				vec!(
					ShaderBinding::Buffer("camera uniform".into()),
				),
				vec!(
					ShaderBinding::Texture("albedo".into(), TextureBindingType::Texture),
					ShaderBinding::Sampler("albedo sampler".into()),
				)
			)
		}
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



// impl ShaderBindGroup {
// 	pub fn layoutlabel(&self) -> String {
// 		let mut bind_group_label = String::from("Bind Group Layout with ");
// 		let binding_count = self.bindings.len();
// 		if binding_count > 0 {
// 			for i in 0..(self.bindings.len()-1) {
// 				let binding = &self.bindings[i];
// 				bind_group_label.push_str(&format!("{} {}, ", &binding.binding_type, &binding.resource_id));
// 			}
// 		}
// 		let last = &self.bindings[binding_count-1];
// 		bind_group_label.push_str(&format!("{} {}, ", &last.binding_type, &last.resource_id));
// 		bind_group_label
// 	}
// }