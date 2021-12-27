
use crate::{
	render::*,
	indexmap::*,
};
use wgpu;
use wgpu::util::DeviceExt;
use std::num::NonZeroU32;
use std::collections::HashMap;
use std::sync::Arc;




// Information needed to render an object
pub struct RenderData {
	pub mesh_id: usize,
	pub pipeline_id: usize,
	pub material_id: usize,
	pub instance: Instance,
}



pub struct Renderer {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,

	pub camera_uniform: CameraUniform,
	pub camera_uniform_buffer: wgpu::Buffer,
	pub camera_bind_group_layout: wgpu::BindGroupLayout,
	pub camera_bind_group: wgpu::BindGroup,
	
	pub texture_manager: TextureManager,
	pub shader_manager: ShaderManager,
	pub material_manager: MaterialManager,
	
	pub mesh_manager: IndexMap<Mesh>,

	depth_cache: HashMap<[u32; 2], Texture>, // Should have a limit for how many to store
}
impl Renderer {
	pub async fn new(adapter: &wgpu::Adapter) -> Self {

		let (device, queue) = adapter.request_device(
			&wgpu::DeviceDescriptor {
				features: 
					wgpu::Features::POLYGON_MODE_LINE | // Wireframe
					wgpu::Features::SPIRV_SHADER_PASSTHROUGH | // wgsl too weak for now
					wgpu::Features::TEXTURE_BINDING_ARRAY |
					wgpu::Features::UNSIZED_BINDING_ARRAY | 
					wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY |
					wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
				limits: wgpu::Limits {
					max_sampled_textures_per_shader_stage: 1024,
					..wgpu::Limits::default()
				},
				label: None,
			},
			None,
		).await.unwrap();
		let device = Arc::new(device);
		let queue = Arc::new(queue);

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

		let texture_manager = TextureManager::new(&device, &queue);
		let shader_manager = ShaderManager::new(&device, &queue);
		let material_manager = MaterialManager::new(&device, &queue);
		let mesh_manager = IndexMap::new();
		

		let depth_cache = HashMap::new();

		Self {
			device,
			queue,
			camera_uniform,
			camera_uniform_buffer,
			camera_bind_group_layout,
			camera_bind_group,
			texture_manager,
			shader_manager,
			material_manager,
			mesh_manager,
			depth_cache,
		}
	}


	fn compile_material(
		&mut self,
		material: &MaterialSpecification,
	) -> Material {
		let shader_id = self.shader_manager.index_path[&material.shader];
		let shader = &self.shader_manager.shaders[shader_id];

		// Collect resource info
		let mut texture_view_collections = Vec::new();
		let mut samplers = Vec::new();
		let mut binding_templates = Vec::new(); // (type, binding position, index)
		// Asssume this is where material happens
		let shader_bind_group = &shader.bind_groups[1];
		for binding in &shader_bind_group.bindings {
			let j = binding.layout.binding;
			match binding.binding_type {
				BindingType::Texture => {
					let texture_key = &binding.resource_id;
					// Todo: If there is no such resource then fill with default
					let texture_idx = self.texture_manager.index_path[&material.textures[texture_key][0]];
					binding_templates.push((BindingType::Texture, j as u32, texture_idx));
				},
				BindingType::ArrayTexture => {
					// Make/get array texture
					todo!("Array texture not done please forgive");
				},
				BindingType::TextureArray => {
					let texture_key = &binding.resource_id;
					let texture_paths = &material.textures[texture_key];
					let texture_indices = texture_paths.iter().map(|p| self.texture_manager.index_path[p]).collect::<Vec<_>>();
					// A texture array is built from a slice of memory containing references to texture views
					let mut texture_views = Vec::new();
					for i in texture_indices {
						let view = &self.texture_manager.textures[i].texture.view;
						texture_views.push(view);
					}
					// pushing to a vec might cause it to be reallocated
					// Any existing slices would become invalid when this happened
					let tv_idx = texture_view_collections.len();
					texture_view_collections.push(texture_views);

					// We defer slice creation until after all texture array data has been allocated
					binding_templates.push((BindingType::TextureArray, j as u32, tv_idx));
				},
				BindingType::Sampler => {
					let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor::default());
					let i = samplers.len();
					samplers.push(sampler);

					binding_templates.push((BindingType::Sampler, j as u32, i));
				},
				_ => panic!("This shader binding type is not (yet?) supported!"),
			}
		}
		// If empty then no material data is used
		let mut bindings = Vec::new();
		// Defered bind group entry creation!
		for (binding_type, position, ridx) in binding_templates {
			match binding_type {
				BindingType::Texture => {
					let texture_view = &self.texture_manager.textures[ridx].texture.view;
					bindings.push(wgpu::BindGroupEntry {
						binding: position,
						resource: wgpu::BindingResource::TextureView(texture_view),
					});
				},
				BindingType::ArrayTexture => {
					todo!("Array texture still not done please forgive again");
				},
				BindingType::TextureArray => {
					let texture_views_slice = &texture_view_collections[ridx][..];
					bindings.push(wgpu::BindGroupEntry {
						binding: position,
						resource: wgpu::BindingResource::TextureViewArray(texture_views_slice),
					});
				},
				BindingType::Sampler => {
					let sr = &samplers[ridx];
					bindings.push(wgpu::BindGroupEntry {
						binding: position,
						resource: wgpu::BindingResource::Sampler(&sr),
					});
				},
				_ => panic!("how did you reach this?"),
			}
		}

		let name = format!("Bind Group for shader {}, material {}, index {}", &shader.name, &material.name, &shader_bind_group.location);
		
		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &bindings[..],
			layout: &shader_bind_group.layout,
			label: Some(&*name),
		});

		let bind_group_id = self.material_manager.bind_groups.insert(&name, bind_group);

		Material {
			name, shader_id, bind_group_id,
		}
	}


	// Renders some objects from the perspective of a camera
	pub fn render(
		&mut self, 
		view: &wgpu::TextureView, 
		width: u32,
		height: u32,
		camera: &Camera, 
		data: &Vec<RenderData>,
	) {
		// Fetch/create depth texture
		let dims = [width, height];
		if !self.depth_cache.contains_key(&dims) {
			info!("Creating depth texture width: {} height: {}", width, height);
			let t = Texture::create_depth_texture(&self.device, width, height, &format!("depth texture {} {}", width, height));
			self.depth_cache.insert(dims, t);
			
		}
		let depth_texture = &self.depth_cache[&dims];
		
		// Push instance data
		let mut instance_data = Vec::new();
		for thing in data {
			instance_data.push(thing.instance.to_raw());
		}
		let instance_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Instance Buffer"),
			contents: bytemuck::cast_slice(&instance_data),
			usage: wgpu::BufferUsages::VERTEX,
		});

		// Update camera
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

			render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

			render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

			for (i, render_data) in data.iter().enumerate() {
				// Material
				let material = self.material_manager.materials.index(render_data.material_id);
				
				// Shader
				let shader_id = material.shader_id;
				let shader = &self.shader_manager.shaders[shader_id];
				
				// Pipeline
				let pipeline = &shader.pipeline;
				render_pass.set_pipeline(pipeline);
				// How to set vertex buffer for pipeline?

				// Bind groups
				let bind_group_id = material.bind_group_id;
				let bind_group = self.material_manager.bind_groups.index(bind_group_id);
				render_pass.set_bind_group(0, bind_group, &[]);
				
				// Mesh
				let mesh = self.mesh_manager.get_index(render_data.mesh_id);
				render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
				render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

				// Drawing
				let idx = i as u32;
				render_pass.draw_indexed(0..mesh.num_elements, 0, idx..idx+1);
			}

		}

		self.queue.submit(std::iter::once(encoder.finish()));
	}

	// pub fn render_oit(
	// 	&mut self, 
	// 	view: &wgpu::TextureView, 
	// 	width: u32,
	// 	height: u32,
	// 	camera: &Camera, 
	// 	data: &Vec<RenderData>,
	// ) {
	// 	let depth_texture = Texture::create_depth_texture(&self.device, width, height, &format!("depth texture {} {}", width, height));

	// 	let texture_size = wgpu::Extent3d {
	// 		width,
	// 		height,
	// 		depth_or_array_layers: 1,
	// 	};

	// 	let accum_texture = self.device.create_texture(
	// 		&wgpu::TextureDescriptor {
	// 			label: None,
	// 			size: texture_size,
	// 			mip_level_count: 1,
	// 			sample_count: 1,
	// 			dimension: wgpu::TextureDimension::D2,
	// 			format: wgpu::TextureFormat::Rgba8UnormSrgb, // Rgba16Float
	// 			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
	// 		}
	// 	);
	// 	let accum_view = accum_texture.create_view(&wgpu::TextureViewDescriptor::default());
	// 	let accum_depth = Texture::create_depth_texture(&self.device, width, height, "accum depth");

	// 	let revealage_texture = self.device.create_texture(
	// 		&wgpu::TextureDescriptor {
	// 			label: None,
	// 			size: texture_size,
	// 			mip_level_count: 1,
	// 			sample_count: 1,
	// 			dimension: wgpu::TextureDimension::D2,
	// 			format: wgpu::TextureFormat::R8Unorm,
	// 			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
	// 		}
	// 	);
	// 	let revealage_view = revealage_texture.create_view(&wgpu::TextureViewDescriptor::default());
	// 	let revealage_depth = Texture::create_depth_texture(&self.device, width, height, "revealage depth");


	// 	let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
	// 		label: Some("Render Encoder"),
	// 	});
	// 	{
	// 		let mut accum_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
	// 			label: Some("accum pass"),
	// 			color_attachments: &[wgpu::RenderPassColorAttachment {
	// 				view: &view, // Texture to save to
	// 				resolve_target: None, // Same as view unless using multisampling
	// 				ops: wgpu::Operations {
	// 					load: wgpu::LoadOp::Clear(wgpu::Color {
	// 						r: 0.1,
	// 						g: 0.2,
	// 						b: 0.3,
	// 						a: 1.0,
	// 					}),
	// 					store: true,
	// 				},
	// 			}],
	// 			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
	// 				view: &depth_texture.view,
	// 				depth_ops: Some(wgpu::Operations {
	// 					load: wgpu::LoadOp::Clear(1.0),
	// 					store: true,
	// 				}),
	// 				stencil_ops: None,
	// 			}),
	// 		});

	// 		let mut revealage_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
	// 			label: Some("revealage pass"),
	// 			color_attachments: &[wgpu::RenderPassColorAttachment {
	// 				view: &view,
	// 				resolve_target: None,
	// 				ops: wgpu::Operations {
	// 					load: wgpu::LoadOp::Clear(wgpu::Color {
	// 						r: 1.0,
	// 						g: 0.0,
	// 						b: 0.0,
	// 						a: 0.0,
	// 					}),
	// 					store: true,
	// 				},
	// 			}],
	// 			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
	// 				view: &depth_texture.view,
	// 				depth_ops: Some(wgpu::Operations {
	// 					load: wgpu::LoadOp::Clear(1.0),
	// 					store: true,
	// 				}),
	// 				stencil_ops: None,
	// 			}),
	// 		});
	// 	}
	// }

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
