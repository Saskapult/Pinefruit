use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use egui_wgpu_backend::RenderPass;
use crate::render::*;
use crate::mesh::*;
use crate::texture::*;
use crate::material::*;




/// A handle to all GPU-based stuff.
/// Can pull data from other systems.
pub struct GPUResource {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
	pub egui_rpass: RenderPass,

	// Resources held in GPU memory
	pub data: GPUData,

	graphs: Vec<Box<dyn RunnableNode>>,
	loaded_graphs: Vec<PathBuf>,
	model_queues: ModelQueuesResource,
	graph_resources: GraphResources,
	camera_buffer_index: usize,
	ssao_buffer_index: usize,

	// Needed for graph resources textures 
	output_resolution: [u32; 2],
}
impl GPUResource {
	pub fn new(
		adapter: &wgpu::Adapter,
		textures_data_manager: &Arc<RwLock<TextureManager>>,
		meshes_data_manager: &Arc<RwLock<MeshManager>>,
		materials_data_manager: &Arc<RwLock<MaterialManager>>,
	) -> Self {
		let (device, queue) = pollster::block_on(adapter.request_device(
			&wgpu::DeviceDescriptor {
				features: 
					wgpu::Features::POLYGON_MODE_LINE | 
					wgpu::Features::SPIRV_SHADER_PASSTHROUGH | 
					wgpu::Features::TEXTURE_BINDING_ARRAY | 
					// wgpu::Features::UNSIZED_BINDING_ARRAY | 
					wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY | 
					wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING |
					wgpu::Features::TIMESTAMP_QUERY |
					wgpu::Features::WRITE_TIMESTAMP_INSIDE_PASSES,
				limits: wgpu::Limits {
					max_sampled_textures_per_shader_stage: 1024,
					..wgpu::Limits::default()
				},
				label: None,
			},
			None,
		)).unwrap();
		let device = Arc::new(device);
		let queue = Arc::new(queue);
		let egui_rpass = RenderPass::new(
			&device, 
			wgpu::TextureFormat::Bgra8UnormSrgb, 
			1,
		);

		let data = GPUData::new(
			&device,
			&queue,
			textures_data_manager,
			meshes_data_manager,
			materials_data_manager,
		);

		let mut graph_resources = GraphResources::new();
		let camera_buffer_index = graph_resources.insert_buffer(
			CameraUniform::new().make_buffer(&device), 
			&"camera_uniform".to_string(),
		);
		let ssao_buffer_index = graph_resources.insert_buffer(
			SSAOUniform::new(0, 0).make_buffer(&device), 
			&"ssao_uniform".to_string(),
		);
		let _ssao_texture_index = graph_resources.insert_texture(
			SSAOUniform::make_noise_texture(&device, &queue, [4, 4]), 
			&"ssao_noise".to_string(),
		);

		Self {
			device,
			queue,
			egui_rpass,

			data,

			graphs: Vec::new(),
			loaded_graphs: Vec::new(),
			model_queues: ModelQueuesResource::new(),
			graph_resources: GraphResources::new(),
			camera_buffer_index,
			ssao_buffer_index,

			output_resolution: [800, 600],
		}
	}

	/// To be called if a graph is added or resolution changes
	pub fn graph_setup(&mut self, new_resolution: Option<[u32; 2]>) {
		if let Some(res) = new_resolution {
			self.graph_resources.default_resolution = res;
			// Update ssao
			SSAOUniform::new(res[0], res[1]).update_buffer(
				&self.queue, 
				self.graph_resources.get_buffer(self.ssao_buffer_index),
			);
		}

		// Get model input formats
		let model_inputs: HashSet<(Vec<InstanceProperty>, Vec<VertexProperty>, BindGroupFormat)> = self.graphs.iter().flat_map(|graph| {
			graph.inputs().iter().filter_map(|(_, grt)| {
				match grt {
					GraphResourceType::Models(model_input) => Some(model_input.clone()),
					_ => None,
				}
			})
		}).collect::<HashSet<_>>();
		
		// Update model formats
		self.model_queues.update_formats(model_inputs);

		// Update graphs
		for graph in self.graphs.iter_mut() {
			graph.update(&mut self.graph_resources, &mut self.model_queues, &mut self.data);
		}
	}

	pub fn set_data(&mut self, model_instances: Vec<ModelInstance>) {
		let update_st = Instant::now();

		let new_graphs = {
			let materials = self.data.materials.data_manager.read().unwrap();
			model_instances.iter().filter_map(|model_instance| {
				let g = &materials.index(model_instance.material_idx).graph;
				if !self.loaded_graphs.contains(g) {
					Some(g.clone())
				} else {
					None
				}
			}).collect::<HashSet<_>>()
		};
		if new_graphs.len() > 0 {
			for graph_path in new_graphs {
				info!("Loading graph {graph_path:?}");
				let graph = Box::new(example_graph_read(&graph_path, &mut self.data.shaders));
				self.graphs.push(graph);
				self.loaded_graphs.push(graph_path);
			}
			info!("Reinitializing graphs");
			self.graph_setup(None);
		}
		
		self.model_queues.update_models(model_instances, &mut self.data);
		self.model_queues.update_instances(&self.device, 0.0);

		let _update_en = Instant::now() - update_st;
	}

	/// Renders some objects from the perspective of a camera
	pub fn encode_render(
		&mut self, 
		mut encoder: &mut wgpu::CommandEncoder,
		dest: &wgpu::Texture, 
		width: u32,
		height: u32,
		camera: &Camera, 
		_t: Instant,
	) {
		// Update camera
		CameraUniform::new_from_camera(camera, width as f32, height as f32).update_buffer(
			&self.queue, 
			self.graph_resources.get_buffer(self.camera_buffer_index),
		);

		// Run graphs
		for graph in &mut self.graphs {
			graph.run(
				&mut self.graph_resources, 
				&mut self.model_queues, 
				&self.data, 
				&mut encoder,
			);
		}

		// Copy output to destination
		let output_texture = self.graph_resources.get_texture(
			self.graph_resources.get_index_of_id(&"final".to_string(), GraphResourceType::Texture).unwrap()
		);
		encoder.copy_texture_to_texture(
			wgpu::ImageCopyTextureBase { 
				texture: &output_texture.texture, 
				mip_level: 0, 
				origin: wgpu::Origin3d::ZERO, 
				aspect: wgpu::TextureAspect::All, 
			}, 
			wgpu::ImageCopyTextureBase { 
				texture: dest, 
				mip_level: 0, 
				origin: wgpu::Origin3d::ZERO, 
				aspect: wgpu::TextureAspect::All, 
			},
			output_texture.size,
		);
	}
}



/// A thing to hold other things because of the borrow checker.
pub struct GPUData {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
	pub textures: BoundTextureManager,
	pub shaders: ShaderManager,
	pub materials: BoundMaterialManager,
	pub meshes: BoundMeshManager,
}
impl GPUData {
	pub fn new(
		device: &Arc::<wgpu::Device>, 
		queue: &Arc::<wgpu::Queue>,
		textures_data_manager: &Arc<RwLock<TextureManager>>,
		meshes_data_manager: &Arc<RwLock<MeshManager>>,
		materials_data_manager: &Arc<RwLock<MaterialManager>>,
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();
		let textures = BoundTextureManager::new(&device, &queue, textures_data_manager);
		let materials = BoundMaterialManager::new(&device, &queue, materials_data_manager);
		let meshes = BoundMeshManager::new(&device, &queue, meshes_data_manager);
		let shaders = ShaderManager::new(&device, &queue);

		Self {
			device, 
			queue,
			textures,
			materials,
			meshes,
			shaders,
		}
	}
}
