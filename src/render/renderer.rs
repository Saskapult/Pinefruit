
use crate::render::*;
use wgpu::util::DeviceExt;
use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, RwLock};




pub struct Renderer {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,

	pub camera_uniform: CameraUniform,
	pub camera_uniform_buffer: wgpu::Buffer,
	pub camera_bind_group_layout_idx: usize,
	pub camera_bind_group: wgpu::BindGroup,
	
	textures: BoundTextureManager,
	shaders: ShaderManager,
	materials: BoundMaterialManager,
	models: ModelManager,
	meshes: BoundMeshManager,

	pub check_bind_group_format: bool,
	pub check_vertex_properties: bool,

	depth_cache: HashMap<[u32; 2], BoundTexture>, // Should have a limit for how many to store
}
impl Renderer {
	pub async fn new(
		adapter: &wgpu::Adapter,
		textures_data_manager: &Arc<RwLock<TextureManager>>,
		meshes_data_manager: &Arc<RwLock<MeshManager>>,
		materials_data_manager: &Arc<RwLock<MaterialManager>>,
	) -> Self {

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

		let textures = BoundTextureManager::new(&device, &queue, textures_data_manager);
		let materials = BoundMaterialManager::new(&device, &queue, materials_data_manager);
		let meshes = BoundMeshManager::new(&device, &queue, meshes_data_manager);
		let mut shaders = ShaderManager::new(&device, &queue);
		let models = ModelManager::new();

		let camera_uniform = CameraUniform::new();
		let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Uniform Buffer"),
			contents: bytemuck::cast_slice(&[camera_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let camera_bind_group_layout_idx = shaders.bind_group_layout_create(&BindGroupFormat {
			binding_specifications: vec![BindGroupEntryFormat {
				binding_type: BindingType::Buffer,
				resource_usage: "camera".to_string(),
				layout: wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
			}]
		});

		let camera_bind_group_layout = shaders.bind_group_layout_index(camera_bind_group_layout_idx);
		let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &camera_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: camera_uniform_buffer.as_entire_binding(),
			}],
			label: Some("Camera Bind Group"),
		});
		
		let depth_cache = HashMap::new();

		Self {
			device,
			queue,
			camera_uniform,
			camera_uniform_buffer,
			camera_bind_group_layout_idx,
			camera_bind_group,
			textures,
			shaders,
			materials,
			models,
			meshes,
			check_bind_group_format: true,
			check_vertex_properties: true,
			depth_cache,
		}
	}

	// The one true render contex creator
	// Does everything automatically
	pub fn add_model(&mut self, name: &String, mesh: &Mesh, material: &Material) -> usize {
		let name = name.clone();
		
		let shader_idx = match self.shaders.index_path(&material.shader) {
			Some(index) => index,
			None => self.shaders.register_path(&material.shader),
		};
		let shader = self.shaders.index(shader_idx);
		let vp = shader.vertex_properties.clone();
		let bgf = shader.bind_groups[&1].format.clone();

		let material_idx = match self.materials.index_name_format(&material.name, &bgf) {
			Some(index) => index,
			None => {
				let mat = self.materials.bind_material(&material, &mut self.shaders, &mut self.textures);
				self.materials.insert(mat)
			},
		};

		let mesh_idx = match self.meshes.index_name_properites(&mesh.name, &vp) {
			Some(index) => index,
			None => {
				let mesh = self.meshes.bind(&mesh, &vp);
				self.meshes.insert(mesh)
			},
		};
		
		let model = Model {
			name, material_idx, mesh_idx,
		};

		self.models.insert(model)
	}

	// ModelInstance creation with verification
	pub fn instance(
		&self, 
		model_idx: usize, 
		instance_properties: Vec<InstanceProperty>, 
		instance_properties_data: Vec<u8>,
	) -> ModelInstance {
		let model = self.models.index(model_idx);
		let material = self.materials.index(model.material_idx);
		let shader = self.shaders.index(material.shader_idx);
		if shader.instance_properties != instance_properties {
			panic!("ModelInstance properties do not match those of shader!");
		}
		ModelInstance {
			model_idx, instance_properties, instance_properties_data,
		}
	}

	// Renders some objects from the perspective of a camera
	pub fn render(
		&mut self, 
		view: &wgpu::TextureView, 
		width: u32,
		height: u32,
		camera: &Camera, 
		data: &mut Vec<ModelInstance>,
	) {
		// info!("Sorting part!");
		// Sort by instance properties and if equal sort by material
		// Would be best to implement insertion sort
		data.sort_unstable_by(|a, b| {
			match a.instance_properties.cmp(&b.instance_properties) {
				std::cmp::Ordering::Equal => a.model_idx.cmp(&b.model_idx),
				std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
				std::cmp::Ordering::Less => std::cmp::Ordering::Less,
			}
		});

		// Fetch/create depth texture
		let dims = [width, height];
		if !self.depth_cache.contains_key(&dims) {
			info!("Creating depth texture width: {} height: {}", width, height);
			let t = BoundTexture::create_depth_texture(&self.device, width, height, &format!("depth texture {} {}", width, height));
			self.depth_cache.insert(dims, t);
			
		}
		let depth_texture = &self.depth_cache[&dims];
		
		// info!("Instance part!");
		// Maps properties to collected buffer data
		let mut instance_buckets: BTreeMap<Vec<InstanceProperty>, (u32, Vec<u8>)> = BTreeMap::new();
		for i in 0..data.len() {
			let entry = &data[i];
			if instance_buckets.contains_key(&entry.instance_properties) {
				let bucket = instance_buckets.get_mut(&entry.instance_properties).unwrap();
				bucket.0 = bucket.0 + 1;
				bucket.1.append(&mut entry.instance_properties_data.clone());
			} else {
				instance_buckets.insert(entry.instance_properties.clone(), (1, entry.instance_properties_data.clone()));
			}
		}
		let mut instance_buffers = Vec::new();
		let mut instance_counts = Vec::new();
		// Sorting is important here
		for (id, (instance_count, instance_data)) in instance_buckets {
			//info!("ib: {:?} count {}", &id, instance_count);
			instance_counts.push(instance_count);
			let instance_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("Instance Buffer"),
				contents: &instance_data[..],
				usage: wgpu::BufferUsages::VERTEX,
			});
			instance_buffers.push(instance_buffer);
		}

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

			render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

			// For each set of instance data
			let mut count = 0;
			for i in 0..instance_buffers.len() {
				render_pass.set_vertex_buffer(1, instance_buffers[i].slice(..));

				// Get range of render data using this instance buffer
				let ib_st = count as usize;
				count = count + instance_counts[i];
				let ib_en = count as usize;

				let mut material: Option<&BoundMaterial> = None;
				let mut mesh: Option<&BoundMesh> = None;
				for data_idx in ib_st..ib_en {
					let data = &data[data_idx];
					let this_model = self.models.index(data.model_idx);
					let this_material = self.materials.index(this_model.material_idx);
					let this_shader = self.shaders.index(this_material.shader_idx);
					let this_mesh = self.meshes.index(this_model.mesh_idx);

					// Material switching
					if material.is_none() || (this_material.name != material.unwrap().name) {
						//info!("Setting material to '{}'!", &this_material);
						
						render_pass.set_bind_group(1, &this_material.bind_group, &[]);

						// Todo: Shader change testing
						//info!("\tSetting shader to '{}'!", &this_shader);
						if self.check_vertex_properties {
							if this_shader.bind_groups[&1].format != this_material.bind_group_format {
								panic!("Material and shader not compatible!");
							}
						}
						render_pass.set_pipeline(&this_shader.pipeline);

						material = Some(this_material);
					}

					// Mesh switching
					if mesh.is_none() || (this_mesh.name != mesh.unwrap().name) {
						// Set the mesh
						if self.check_vertex_properties {
							if this_shader.vertex_properties != this_mesh.vertex_properties {
								panic!("Mesh and shader not compatible!");
							}
						}
						//info!("Setting mesh to '{}'!", &this_mesh);
						render_pass.set_vertex_buffer(0, this_mesh.vertex_buffer.slice(..));
						render_pass.set_index_buffer(this_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
						mesh = Some(this_mesh);
					} 

					//info!("Drawing '{}'!", &this_model.name);
					let idx = (data_idx - ib_st) as u32;
					render_pass.draw_indexed(0..mesh.unwrap().n_vertices, 0, idx..idx+1);
				}
			}
		}

		self.queue.submit(std::iter::once(encoder.finish()));
	}

}
