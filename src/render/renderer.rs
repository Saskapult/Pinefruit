
use crate::{
	geometry::*,
	texture::Texture,
	render::*,
	render::Instance,
	resource_manager::*,
};
use winit::{
	event::*,
	window::Window,
};
use nalgebra::*;
use wgpu::util::DeviceExt;
use wgpu::*;
use std::num::NonZeroU32;



// Information needed to render an object
pub struct RenderData {
	pub material_id: usize,
	pub mesh_id: usize,
	pub instance: Instance,
}


// The renderer renders things
pub struct Renderer {
	pub device: wgpu::Device,	// The GPU instance for this renderer
	pub queue: wgpu::Queue,
	pub camera_uniform: CameraUniform,
	pub camera_uniform_buffer: Buffer,
	pub textures: IndexMap<Texture>,
	pub meshes: IndexMap<Mesh>,
	pub pipelines: IndexMap<wgpu::RenderPipeline>,
	pub pipeline_layouts: IndexMap<wgpu::PipelineLayout>,
	pub bindgroups: IndexMap<wgpu::BindGroup>,
	pub bindgroup_layouts: IndexMap<wgpu::BindGroupLayout>,
}
impl Renderer {
	pub async fn new(adapter: &wgpu::Adapter) -> Self {

		let (device, queue) = adapter.request_device(
			&wgpu::DeviceDescriptor {
				features: 
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

		let textures = IndexMap::new();
		let meshes = IndexMap::new();
		let bindgroup_layouts = IndexMap::new();
		let bindgroups = IndexMap::new();
		let pipeline_layouts = IndexMap::new();
		let pipelines = IndexMap::new();
		
		Self {
			device,
			queue,
			camera_uniform,
			camera_uniform_buffer,
			textures,
			meshes,
			pipelines,
			pipeline_layouts,
			bindgroups,
			bindgroup_layouts,
		}
	}

	
	pub fn load_texture_disk(
		&mut self, 
		name: &String, 
		path: &String,
	) -> usize {
		let image = image::open(path)
			.expect("Failed to open file");
		let texture = Texture::from_image(&self.device, &self.queue, &image, Some(name))
			.expect("Failed to create texture");
		self.textures.insert(name, texture)
	}


	pub fn create_camera_bgl(
		&mut self,
		name: &String,
	) -> usize {
		let camera_bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
			label: Some(name),
		});

		self.bindgroup_layouts.insert(name, camera_bind_group_layout)
	}


	// Specific use case
	pub fn create_tarray_bgl(
		&mut self,
		name: &String,
		max_len: usize,
	) -> usize {
		let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
		});

		self.bindgroup_layouts.insert(name, bind_group_layout)
	}


	pub fn create_camera_bg(
		&mut self,
		name: &String,
		bgl_name: &String,
	) -> usize {
		let camera_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &self.bindgroup_layouts.get_name(bgl_name),
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: self.camera_uniform_buffer.as_entire_binding(),
			}],
			label: Some(name),
		});

		self.bindgroups.insert(name, camera_bind_group)
	}


	// Specific use case
	// Makes a texture array from the specified texture indices
	pub fn create_tarray_bg(
		&mut self, 
		name: &String, 
		texture_indices: &Vec<usize>, 
		bgl_name: &String,
	) -> usize {
		let mut texture_views = Vec::new();
		for i in texture_indices {
			texture_views.push(&self.textures.data[*i].view);
		}

		let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor::default());

		let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
			layout: &self.bindgroup_layouts.get_name(bgl_name),
			label: Some(name),
		});

		self.bindgroups.insert(name, bg)
	}


	pub fn create_pipeline_layout(
		&mut self,
		name: &String,
		bgl_names: &Vec<&String>,
	) -> usize {
		let bgls = Vec::new();
		for bgl_name in bgl_names {
			bgls.push(self.bindgroup_layouts.get_name(bgl_name));
		}

		let pl = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some(name),
			bind_group_layouts: &bgls.as_slice(),
			push_constant_ranges: &[],
		});

		self.pipeline_layouts.insert(name, pl)
	}


	pub fn create_pipeline(
		&mut self,
		name: &String,
		path: &String,
		layout_name: &String,
		color_format: wgpu::TextureFormat,
		depth_format: Option<wgpu::TextureFormat>,
		vertex_layouts: &[wgpu::VertexBufferLayout],
	) -> usize {

		let shader_source = std::fs::read_to_string(path)
			.expect("Failed to open shader source");

		let shader = self.device.create_shader_module(&wgpu::ShaderModuleDescriptor {
			label: Some(name),
			source: wgpu::ShaderSource::Wgsl(shader_source.into()),
		});
	
		let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some(name),
			layout: Some(self.pipeline_layouts.get_name(layout_name)),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: vertex_layouts,
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: "fs_main",
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
				cull_mode: Some(wgpu::Face::Back),
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
		});

		self.pipelines.insert(name, pipeline)
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
		
		self.camera_uniform.update(&camera, width as f32, height as f32);
		
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

			render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

			for thing in data {
			}

		}
		
		

		// 	// For shader bucket
		// 	for (pid, tb) in &shaders {
		// 		let pipeline = self.pipeline_manager.get_index(*pid);
		// 		render_pass.set_pipeline(&pipeline);
		// 		for (tid, mb) in tb {
		// 			let tbg = self.texture_manager.get(*tid);
		// 			render_pass.set_bind_group(0, &tbg, &[]);
		// 			for (mid, range) in mb {
		// 				let mesh = self.mesh_manager.get_index(*mid);
		// 				render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
		// 				render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

		// 				let st = range[0] as u32;
		// 				let en = range[1] as u32;
		// 				render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
						
		// 				render_pass.draw_indexed(0..mesh.num_elements, 0, st..en);
		// 			}
		// 		}
		// 	}

		self.queue.submit(std::iter::once(encoder.finish()));
	}

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



