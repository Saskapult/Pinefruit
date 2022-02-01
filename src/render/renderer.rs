use crate::render::*;
use wgpu::util::DeviceExt;
use std::collections::{HashMap, BTreeSet, HashSet};
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::time::Instant;




#[derive(Debug)]
pub struct RenderModelInstance {
	pub mesh_idx: usize,
	pub material_idx: usize,
	pub instance_st: Instance,
	pub instance_en: Instance,
}



#[derive(Debug)]
pub struct Renderer {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,

	camera_uniform: CameraUniform,
	camera_buffer_index: usize,
	
	render_resources: RenderResources,
	
	models_start: Instant,
	models_end: Instant,

	graph_resources: GraphLocals,
	opaque_models: ModelsResource,
	opaque_graph: Box<dyn RunnableNode>,

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

		let mut render_resources = RenderResources::new(&device, &queue, textures_data_manager, meshes_data_manager, materials_data_manager);
		let mut graph_resources = GraphLocals::new(&device, &queue);
		let opaque_models = ModelsResource::new(&device, &queue);
		let opaque_graph = Box::new(example_graph_read(&PathBuf::from("resources/graphs/default.ron"), &mut render_resources.shaders));

		let camera_uniform = CameraUniform::new();
		let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera uniform Buffer"),
			contents: bytemuck::cast_slice(&[camera_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});
		let camera_buffer_index = graph_resources.insert_buffer(camera_uniform_buffer, &"_camera".to_string());

		Self {
			device,
			queue,
			camera_uniform,
			camera_buffer_index,
			render_resources,
			models_start: Instant::now(),
			models_end: Instant::now(),
			graph_resources,
			opaque_models,
			opaque_graph,
			depth_cache: HashMap::new(),
		}
	}

	pub fn set_data(&mut self, mut model_instances: Vec<ModelInstance>) {
		info!("We have {} model instances", model_instances.len());
		
		// Load materials if not loaded?

		// Collect all using kdefault
		// Should collect by graph and load, but that's not a task for for current me
		let materials = self.render_resources.materials.material_manager.read().unwrap();
		let opaque_models = model_instances.drain_filter(|model_instance| {
			materials.index(model_instance.material_idx).graph == PathBuf::from("graphs/default.ron")
		}).collect::<Vec<_>>();
		drop(materials);
		info!("We have {} opaque model instances", opaque_models.len());

		let graph_stuff = self.opaque_graph.inputs().clone();
		
		// Make model inputs
		let model_inputs = graph_stuff.iter().filter_map(|(_, grt)| {
			match grt {
				GraphResourceType::Models(model_input) => Some(model_input.clone()),
				_ => None,
			}
		}).collect::<HashSet<_>>();
		self.opaque_models.update_formats(model_inputs);
		self.opaque_models.update_models(opaque_models, &mut self.render_resources);
		self.opaque_models.update_instances(0.0);

		// Make resource groups
		graph_stuff.iter().filter_map(|(_, grt)| {
			match grt {
				GraphResourceType::Resources(resources_input) => Some(resources_input.clone()),
				_ => None,
			}
		}).for_each(|f| {
			self.graph_resources.create_resources_group(&f, &mut self.render_resources);
		});

		// Update graph
		self.opaque_graph.update(&self.graph_resources, &self.opaque_models, &mut self.render_resources);
	}

	/// Renders some objects from the perspective of a camera
	pub fn render(
		&mut self, 
		dest: &wgpu::Texture, 
		width: u32,
		height: u32,
		camera: &Camera, 
		t: Instant,
	) {
		// Create depth
		let depth_texture = BoundTexture::create_depth_texture(&self.device, width, height, &format!("depth texture {} {}", width, height));
		self.graph_resources.insert_texture(depth_texture, &"_depth".to_string());

		// Update camera
		self.camera_uniform.update(&camera, width as f32, height as f32);
		self.queue.write_buffer(
			&self.graph_resources.get_buffer(self.camera_buffer_index), 
			0, 
			bytemuck::cast_slice(&[self.camera_uniform]),
		);

		// Create albedo
		let albedo_texture = BoundTexture::new(
			&self.device,
			&"albedo".to_string(),
			wgpu::TextureFormat::Bgra8UnormSrgb,
			width, 
			height,
		);
		self.graph_resources.insert_texture(albedo_texture, &"albedo".to_string());

		// // Update instance buffer to the current time
		// let render_frac = self.get_render_frac(t);
		// self.opaque_models.update_instances(render_frac);

		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Render Encoder"),
		});

		// Run opaque graph
		encoder.push_debug_group("opaque");
		self.opaque_graph.run(&mut self.graph_resources, &self.opaque_models, &mut self.render_resources, &mut encoder);
		encoder.pop_debug_group();

		// Copy output to destination
		let output_texture = self.graph_resources.get_texture(
			self.graph_resources.get_index_of_id(&"albedo".to_string(), GraphResourceType::Texture).unwrap()
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

		// Submit queue to make all that stuff happen
		self.queue.submit(std::iter::once(encoder.finish()));
	}

	fn get_render_frac(&self, t: Instant) -> f32 {		
		if t > self.models_end {
			1.0
		} else if t < self.models_start {
			0.0
		} else {
			(self.models_end - t).as_secs_f32() / (self.models_end - self.models_start).as_secs_f32()
		}
	}

}
