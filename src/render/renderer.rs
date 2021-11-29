
use crate::{
	geometry::*,
	texture::Texture,
	render::*,
	render::Instance,
	resource_manager::*,
};
use wgpu::util::DeviceExt;
use wgpu::*;
use std::num::NonZeroU32;



// Information needed to render an object
pub struct RenderData {
	pub mesh_id: usize,
	pub instance: Instance,
}


// The renderer renders things
pub struct Renderer {
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub camera_uniform: CameraUniform,
	pub camera_uniform_buffer: Buffer,
	pub camera_bind_group_layout: BindGroupLayout,
	pub camera_bind_group: BindGroup,
	pub textures: IndexMap<Texture>,
	pub meshes: IndexMap<Mesh>,
	pub pipeline: wgpu::RenderPipeline,
	pub pipeline_layout: wgpu::PipelineLayout,
	pub texture_bindgroup: wgpu::BindGroup,
	pub texture_bindgroup_layout: wgpu::BindGroupLayout,
}
impl Renderer {
	pub async fn new(adapter: &wgpu::Adapter) -> Self {

		let (device, queue) = adapter.request_device(
			&wgpu::DeviceDescriptor {
				features: 
					wgpu::Features::SPIRV_SHADER_PASSTHROUGH | // wgsl too weak for now
					wgpu::Features::TEXTURE_BINDING_ARRAY |
					wgpu::Features::UNSIZED_BINDING_ARRAY | 
					wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY |
					wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
				limits: wgpu::Limits {
					max_sampled_textures_per_shader_stage: 1024,
					..Limits::default()
				},
				label: None,
			},
			None,
		).await.unwrap();

		let camera_uniform = CameraUniform::new();
		let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Uniform Buffer"),
			contents: bytemuck::cast_slice(&[camera_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});
		let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
			label: Some("Camera Bind Group Layout"),
		});
		let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &camera_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: camera_uniform_buffer.as_entire_binding(),
			}],
			label: Some("Camera Bind Group"),
		});

		let mut textures = IndexMap::new();
		let meshes = IndexMap::new();
		
		let texture_bindgroup_layout = create_tarray_bgl(&device, &"fuk".to_string(), 1024);

		let pipeline_layout = create_pipeline_layout(
			&device, 
			&"tarray render pipeline layout".to_string(),
			&[
				&texture_bindgroup_layout, 
				&camera_bind_group_layout,
			],
		);

		let pipeline = create_pipeline_disk(
			&device,
			&"tarray_pipeline".to_string(), 
			&"resources/shaders/texture_array_shader.wgsl".to_string(), 
			&pipeline_layout, 
			wgpu::TextureFormat::Bgra8UnormSrgb, // Todo: make betterer
			Some(crate::texture::Texture::DEPTH_FORMAT),
			&[
				crate::geometry::Vertex::desc(),
				crate::render::InstanceRaw::desc(),
				],
		);

		// Won't let me make empty thingy
		let debug_name = "debug texture".to_string();
		let debug_image = image::open("resources/debug.png")
			.expect("Failed to open file");
		let debug_texture = Texture::from_image(&device, &queue, &debug_image, Some(&debug_name))
			.expect("Failed to create texture");
		textures.insert(&debug_name, debug_texture);

		let texture_bindgroup = create_tarray_bg(
			&device,
			&"tarray_bg".to_string(), 
			&textures.data,
			&texture_bindgroup_layout,
		);
		
		Self {
			device,
			queue,
			camera_uniform,
			camera_uniform_buffer,
			camera_bind_group_layout,
			camera_bind_group,
			textures,
			meshes,
			pipeline_layout,
			pipeline,
			texture_bindgroup_layout,
			texture_bindgroup,
		}
	}


	pub fn load_texture_disk(
		&mut self, 
		name: &String, 
		path: &String,
	) -> usize {
		info!("Loading texture {} from {}", &name, &path);
		let image = image::open(path)
			.expect("Failed to open file");
		let texture = Texture::from_image(&self.device, &self.queue, &image, Some(name))
			.expect("Failed to create texture");
		self.textures.insert(name, texture)
	}


	pub fn recreate_tbg(
		&mut self,
	) {
		self.texture_bindgroup = create_tarray_bg(
			&self.device,
			&"tarray_bg".to_string(), 
			&self.textures.data,
			&self.texture_bindgroup_layout,
		);
	}


	pub fn add_mesh(
		&mut self,
		name: &String,
		mesh: Mesh,
	) -> usize {
		self.meshes.insert(name, mesh)
	}


	// Renders some objects from the perspective of a camera
	pub fn render(
		&mut self, 
		view: &TextureView, 
		width: u32,
		height: u32,
		camera: &Camera, 
		data: &Vec<RenderData>,
	) {
		let depth_texture = crate::texture::Texture::create_depth_texture(&self.device, width, height, "depth_texture");
		
		let mut instance_data = Vec::new();
		for thing in data {
			instance_data.push(thing.instance.to_raw());
		}

		let instance_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Instance Buffer"),
			contents: bytemuck::cast_slice(&instance_data),
			usage: wgpu::BufferUsages::VERTEX,
		});

		self.camera_uniform.update(&camera, width as f32, height as f32);
		self.queue.write_buffer(
			&self.camera_uniform_buffer, 
			0, 
			bytemuck::cast_slice(&[self.camera_uniform]),
		);
		
		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Render Encoder"),
		});

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Opaque Pass"),
				color_attachments: &[wgpu::RenderPassColorAttachment {
					view: &view, // Texture to save to
					resolve_target: None, // Same as view unless using multisampling
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 0.1,
							g: 0.2,
							b: 0.3,
							a: 1.0,
						}),
						store: true,
					},
				}],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &depth_texture.view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: true,
					}),
					stencil_ops: None,
				}),
			});

			render_pass.set_pipeline(&self.pipeline);

			render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
			
			render_pass.set_bind_group(0, &self.texture_bindgroup, &[]);
			render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

			for (i, thing) in data.iter().enumerate() {
				let mesh = self.meshes.get_index(thing.mesh_id);

				render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
				
				render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

				let idx = i as u32;
				render_pass.draw_indexed(0..mesh.num_elements, 0, idx..idx+1);
			}

		}

		self.queue.submit(std::iter::once(encoder.finish()));
	}

}



fn create_tarray_bg(
	device: &wgpu::Device, 
	name: &String, 
	textures: &Vec<Texture>, 
	bgl: &wgpu::BindGroupLayout,
) -> wgpu::BindGroup {
	let mut texture_views = Vec::new();
	for t in textures {
		texture_views.push(&t.view);
	}

	let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

	device.create_bind_group(&wgpu::BindGroupDescriptor {
		entries: &[
			wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::TextureViewArray(texture_views.as_slice()),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: wgpu::BindingResource::Sampler(&sampler),
			},
		],
		layout: bgl,
		label: Some(name),
	})
}



fn create_pipeline_layout(
	device: &wgpu::Device,
	name: &String,
	bind_group_layouts: &[&wgpu::BindGroupLayout],
) -> wgpu::PipelineLayout {
	device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some(name),
		bind_group_layouts,
		push_constant_ranges: &[],
	})
}



pub fn create_pipeline_disk(
	device: &wgpu::Device,
	name: &String,
	path: &String,
	pipeline_layout: &wgpu::PipelineLayout,
	color_format: wgpu::TextureFormat,
	depth_format: Option<wgpu::TextureFormat>,
	vertex_layouts: &[wgpu::VertexBufferLayout],
) -> wgpu::RenderPipeline {

	// let betterpath = std::path::Path::new(path);
	// println!("Reading shader from path {:?}", &betterpath);
	// let shader_source = std::fs::read_to_string(betterpath)
	// 	.expect("Failed to open shader source");

	// let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
	// 	label: Some(name),
	// 	source: wgpu::ShaderSource::Wgsl(shader_source.into()),
	// });

	info!("making shader");
	// wgsl doesn't let me do the things (bindless texture things) that I want to do
	let vshader = unsafe { device.create_shader_module_spirv(
		&wgpu::include_spirv_raw!("../../resources/shaders/opaque.vert.spv"),
	)};
	let fshader = unsafe { device.create_shader_module_spirv(
		&wgpu::include_spirv_raw!("../../resources/shaders/opaque.frag.spv"),
	)};
	info!("made shader");

	info!("making render pipeline");
	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some(name),
		layout: Some(pipeline_layout),
		vertex: wgpu::VertexState {
			module: &vshader,
			entry_point: "main",
			buffers: vertex_layouts,
		},
		fragment: Some(wgpu::FragmentState {
			module: &fshader,
			entry_point: "main",
			targets: &[wgpu::ColorTargetState {
				format: color_format,
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
			// Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
			polygon_mode: wgpu::PolygonMode::Fill,
			// Requires Features::DEPTH_CLAMPING
			clamp_depth: false,
			// Requires Features::CONSERVATIVE_RASTERIZATION
			conservative: false,
		},
		depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
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


pub fn create_tarray_bgl(
	device: &wgpu::Device,
	name: &String,
	max_len: usize,
) -> wgpu::BindGroupLayout {
	device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some(name),
		entries: &[
			wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					sample_type: wgpu::TextureSampleType::Float { filterable: true },
					view_dimension: wgpu::TextureViewDimension::D2,
					multisampled: false,
				},
				count: NonZeroU32::new(max_len as u32),
			},
			wgpu::BindGroupLayoutEntry {
				binding: 1,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Sampler {
					comparison: false,
					filtering: true,
				},
				count: None,
			},
		],
	})
}


	// pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
	// 	if new_size.width > 0 && new_size.height > 0 {
	// 		self.size = new_size;
	// 		self.config.width = new_size.width;
	// 		self.config.height = new_size.height;
	// 		self.surface.configure(&self.device, &self.config);

	// 		self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
	// 	}
	// }



