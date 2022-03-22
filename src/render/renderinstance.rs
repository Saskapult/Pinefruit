use crate::render::*;
use crate::util::DurationHolder;
use wgpu::util::DeviceExt;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::time::Instant;
use crate::mesh::*;
use crate::material::*;
use crate::texture::*;




#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum RenderStage {
	Opaque,
	Transparent,
	Overlay,
}



#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SSAOUniform {
	pub radius: f32,
	pub bias: f32,
	pub contrast: f32,
	pub noise_scale: [f32; 2],
	pub kernel: [[f32; 3]; 16],
}
impl SSAOUniform {
	pub fn new() -> Self {
		Self {
			radius: 1.0,
			bias: 0.01,
			contrast: 1.5,
			noise_scale: [1.0, 1.0],
			kernel: SSAOUniform::make_hemisphere_kernel(),
		}
	}

	pub fn update(&mut self, width: u32, height: u32) {
		let scale_width = width as f32 / 4.0;
		let scale_height = height as f32 / 4.0; 
		self.noise_scale = [scale_width, scale_height];
	}

	pub fn make_hemisphere_kernel() -> [[f32; 3]; 16] {
		use rand::Rng;
		use nalgebra::*;
		let mut kernel = [[0.0; 3]; 16];
		let mut rng = rand::thread_rng();

		for i in 0..16 {
			let mut sample = Vector3::new(
				rng.gen::<f32>() * 2.0 - 1.0, 
				rng.gen::<f32>() * 2.0 - 1.0, 
				rng.gen::<f32>(),
			).normalize();

			//sample *= rng.gen::<f32>();

			let mut scale = (i as f32) / (16 as f32);
			let t = scale * scale;
			//scale = (0.1 * (1.0 - t)) + (1.0 * t);
			scale = 0.1 + t * (1.0 - 0.1);
			sample *= scale;
			
			let s = &mut kernel[i];
			s[0] = sample[0];
			s[1] = sample[1];
			s[2] = sample[2];
		}

		kernel
	}

	pub fn make_noise(amount: u32) -> Vec<[f32; 3]> {
		use rand::Rng;
		let mut rng = rand::thread_rng();
		(0..amount).map(|_| {
			[
				rng.gen::<f32>() * 2.0 - 1.0,
				rng.gen::<f32>() * 2.0 - 1.0,
				rng.gen::<f32>(),
			]
		}).collect::<Vec<_>>()
	}
}




#[derive(Debug)]
pub struct RenderModelInstance {
	pub mesh_idx: usize,
	pub material_idx: usize,
	pub instance_st: Instance,
	pub instance_en: Instance,
}



/// A render instance is a thing that renders stuff on a GPU instance.
/// It holds gpu data which it pulls from other data.
#[derive(Debug)]
pub struct RenderInstance {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,

	camera_uniform: CameraUniform,
	camera_buffer_index: usize,

	ssao_uniform: SSAOUniform,
	ssao_buffer_index: usize,
	
	render_resources: RenderResources,
	
	models_start: Instant,
	models_end: Instant,

	graph_resources: GraphLocals,
	graphs: Vec<Box<dyn RunnableNode>>,
	loaded_graphs: Vec<PathBuf>,	// Workaround for no have path
	// Models should be separated by graph for performance?
	models: ModelsQueueResource,
	opaque_graph: Box<dyn RunnableNode>,

	pub duration_profiling: bool,
	pub update_durations: DurationHolder,
	pub encode_durations: DurationHolder,
}
impl RenderInstance {
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
		let opaque_models = ModelsQueueResource::new(&device, &queue);
		let opaque_graph = Box::new(example_graph_read(&PathBuf::from("resources/graphs/default.ron"), &mut render_resources.shaders));

		// Make camera uniform and save index for updating
		let camera_uniform = CameraUniform::new();
		let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera uniform Buffer"),
			contents: bytemuck::cast_slice(&[camera_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});
		let camera_buffer_index = graph_resources.insert_buffer(camera_uniform_buffer, &"camera_uniform".to_string());

		// Make ssao uniform
		let ssao_uniform = SSAOUniform::new();
		let ssao_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("SSAO Uniform Buffer"),
			contents: bytemuck::cast_slice(&[ssao_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});
		let ssao_buffer_index = graph_resources.insert_buffer(ssao_uniform_buffer, &"ssao_uniform".to_string());

		// Make ssao texture
		let ssao_noise_texture = BoundTexture::new_with_format(
			&device, &"ssao noise".to_string(), 
			wgpu::TextureFormat::Rgba8Unorm, 
			4, 4,
		);
		const NUM_PIXELS: usize = 4 * 4;
		let random_stuff = {
			use rand::Rng;
			let mut rng = rand::thread_rng();
			let u8max = u8::MAX as f32;
			(0..NUM_PIXELS).map(|_| {
				// [ // Random
				// 	(rng.gen::<f32>() * u8max) as u8,
				// 	(rng.gen::<f32>() * u8max) as u8,
				// 	(rng.gen::<f32>() * u8max) as u8,
				// 	(rng.gen::<f32>() * u8max) as u8,
				// ]
				[ // Rotate on z axis in tangent space
					((rng.gen::<f32>() * 2.0 - 1.0) * u8max) as u8,
					((rng.gen::<f32>() * 2.0 - 1.0) * u8max) as u8,
					0,
					0,
				]
			}).collect::<Vec<_>>().concat()
		};
		queue.write_texture(
			wgpu::ImageCopyTexture {
				aspect: wgpu::TextureAspect::All,
				texture: &ssao_noise_texture.texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
			},
			random_stuff.as_slice(),
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: std::num::NonZeroU32::new(4 * ssao_noise_texture.size.width),
				rows_per_image: std::num::NonZeroU32::new(ssao_noise_texture.size.height),
			}, 
			ssao_noise_texture.size,
		);
		graph_resources.insert_texture(ssao_noise_texture, &"ssao_noise".to_string());

		Self {
			device,
			queue,
			camera_uniform,
			camera_buffer_index,
			ssao_uniform,
			ssao_buffer_index,
			render_resources,
			models_start: Instant::now(),
			models_end: Instant::now(),
			graph_resources,
			graphs: Vec::new(),
			loaded_graphs: Vec::new(),
			models: opaque_models,
			opaque_graph,

			duration_profiling: true,
			update_durations: DurationHolder::new(5),
			encode_durations: DurationHolder::new(5),
		}
	}

	/// (Re)initializes the graphs.
	/// Run this when a graph is added or the resolution changes
	pub fn init_graphs(&mut self) {
		let [width, height] = [800, 600];

		self.graph_resources.default_resolution = [width, height]; // needed for texture making

		// Get model input formats
		let model_inputs: HashSet<(Vec<InstanceProperty>, Vec<VertexProperty>, BindGroupFormat)> = self.graphs.iter()
			.flat_map(|graph| {
				graph.inputs().iter().filter_map(|(_, grt)| {
					match grt {
						GraphResourceType::Models(model_input) => Some(model_input.clone()),
						_ => None,
					}
				})
			})
			.collect::<HashSet<_>>();
		
		// Update model formats
		self.models.update_formats(model_inputs);

		// Update graphs
		self.graphs.iter_mut().for_each(|graph| {
			graph.update(&mut self.graph_resources, &mut self.models, &mut self.render_resources);
		});
		
		// Update ssao
		self.ssao_uniform.update(width, height);
		self.queue.write_buffer(
			&self.graph_resources.get_buffer(self.ssao_buffer_index), 
			0, 
			bytemuck::cast_slice(&[self.ssao_uniform]),
		);
	}

	pub fn set_data(&mut self, model_instances: Vec<ModelInstance>) {
		let update_st = Instant::now();

		let new_graphs = {
			let materials = self.render_resources.materials.material_manager.read().unwrap();
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
				let graph = Box::new(example_graph_read(&graph_path, &mut self.render_resources.shaders));
				self.graphs.push(graph);
				self.loaded_graphs.push(graph_path);
			}
			info!("Reinitializing graphs");
			self.init_graphs();
		}
		
		self.models.update_models(model_instances, &mut self.render_resources);
		self.models.update_instances(0.0);		

		self.update_durations.record(Instant::now() - update_st);
	}

	/// Renders some objects from the perspective of a camera
	pub fn render(
		&mut self, 
		mut encoder: &mut wgpu::CommandEncoder,
		dest: &wgpu::Texture, 
		width: u32,
		height: u32,
		camera: &Camera, 
		_t: Instant,
	) {
		let encode_st = Instant::now();

		// Update camera
		self.camera_uniform.update(&camera, width as f32, height as f32);
		self.queue.write_buffer(
			&self.graph_resources.get_buffer(self.camera_buffer_index), 
			0, 
			bytemuck::cast_slice(&[self.camera_uniform]),
		);

		// // Update instance buffer to the current time
		// let render_frac = self.get_render_frac(t);
		// self.opaque_models.update_instances(render_frac);

		// Run graphs
		for graph in &mut self.graphs {
			graph.run(&mut self.graph_resources, &mut self.models, &mut self.render_resources, &mut encoder);
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

		self.encode_durations.record(Instant::now() - encode_st);
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
