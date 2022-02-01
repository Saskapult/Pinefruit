use crate::render::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wgpu::util::DeviceExt;



#[derive(Debug)]
pub struct ModelInstance {
	// Indices to the UNBOUND mesh and material
	pub material_idx: usize,
	pub mesh_idx: usize,
	pub instance: Instance,
}



/// Shader inputs are simplified to instance properties, vertex properties, and material format
pub type ShaderInput = (InstanceProperties, VertexProperties, BindGroupFormat);
/// instance buffer idx, [material idx, mesh idx, count])
pub type ModelQueue = (usize, Vec<(usize, usize, u32)>);



/// The model resource maps shader input to a model queue.
/// Function call order should be (update_formats -> update_models -> update_instances).
/// 
/// Currently instancing has been neglected and vector capacity has been optimized.
/// If you want to change that, you must fight the allocator!
/// 
/// Instances have been set up to support interpolation based on time.
/// This could be useful in the future.
/// 
/// Oh also a lot of this could be parallelized with only minor changes.
#[derive(Debug)]
pub struct ModelsResource {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,

	raw_models: Vec<ModelInstance>,
	// Instance buffer index, [bound mesh idx, bound material idx, numer of instances]
	queues: Vec<ModelQueue>,
	queue_index_of_format: HashMap<ShaderInput, usize>,

	instances_buffers: Vec<wgpu::Buffer>,
	instances_buffer_index_of_format: HashMap<InstanceProperties, usize>,
}
impl ModelsResource {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		Self {
			device: device.clone(),
			queue: queue.clone(),
			raw_models: Vec::new(),
			queues: Vec::new(),
			queue_index_of_format: HashMap::new(),
			instances_buffers: Vec::new(),
			instances_buffer_index_of_format: HashMap::new(),
		}
	}

	pub fn update_formats(
		&mut self, 
		formats: HashSet<ShaderInput>, 
	) {
		// Resize queues
		self.queue_index_of_format = formats.iter().cloned().enumerate().map(|(i, f)| (f, i)).collect::<HashMap<_,_>>();
		// Create any needed new queues assuming a length equal to the last known model set
		self.queues.resize(formats.len(), (0, Vec::with_capacity(self.raw_models.len())));

		// Resize instances
		// Does not add new buffers
		self.instances_buffer_index_of_format = formats.iter().map(|(ip, _, _)| {
			(ip.clone(), 0)
		}).collect::<HashMap<_,_>>();
		//self.instances_buffers.resize(formats.len(), None);
	}

	pub fn update_models(
		&mut self, 
		models: Vec<ModelInstance>,
		resources: &mut RenderResources,
	) {
		info!("Given {} models", models.len());
		for ((_, vp, mbgf), &queue_idx) in self.queue_index_of_format.iter() {
			// I don't want to allocate a new vector
			// The existing vector should either be extended or truncated
			let queue_content = &mut self.queues.get_mut(queue_idx).unwrap().1;
			queue_content.resize(models.len(), (0,0,0));

			models.iter().enumerate().for_each(|(i, model)| {
				let mesh_idx = resources.meshes.index_from_index_properites_bind(model.mesh_idx, vp);
				let material_idx = resources.materials.index_from_index_format_bind(model.material_idx, mbgf, &mut resources.shaders, &mut resources.textures);
				let count = 1;
				queue_content[i] = (material_idx, mesh_idx, count);
			});
			info!("{} models for queue {}", queue_content.len(), queue_idx);
		}
		self.raw_models = models;
		
	}

	pub fn update_instances(
		&mut self, 
		t: f32,
	) {
		self.instances_buffers = Vec::with_capacity(self.queue_index_of_format.len());

		for (instance_properties, _, _) in self.queue_index_of_format.keys() {
			// Could use Vec::with_capacity() if we added a way to get the number of bytes from instance properties
			let mut instances_data = Vec::new();
			for model in &self.raw_models {
				let instance_data = model.instance.data(&instance_properties);
				instances_data.extend_from_slice(&instance_data[..]);
			}

			let instances_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("Instance Buffer"),
				contents: &instances_data[..],
				usage: wgpu::BufferUsages::VERTEX,
			});

			let idx = self.instances_buffers.len();
			// Should overwrite entry
			self.instances_buffer_index_of_format.insert(instance_properties.clone(), idx);
			self.instances_buffers.push(instances_buffer);
		}
	}

	pub fn queue(&self, i: usize) -> &ModelQueue {
		&self.queues[i]
	}

	pub fn queue_index_of_format(
		&self, 
		format: &ShaderInput,
	) -> Option<usize> {
		if self.queue_index_of_format.contains_key(format) {
			Some(self.queue_index_of_format[format])
		} else {
			None
		}
	}

	pub fn instances_buffer(&self, i: usize) -> &wgpu::Buffer {
		&self.instances_buffers[i]
	}

	pub fn instances_buffer_index_of_format(&self, ip: &InstanceProperties) -> Option<usize> {
		if self.instances_buffer_index_of_format.contains_key(ip) {
			Some(self.instances_buffer_index_of_format[ip])
		} else {
			None
		}
	}
}



#[derive(Debug)]
pub struct Model {
	pub name: String,
	pub mesh_idx: usize,	// Must match material's shader vertex input
	pub material_idx: usize,
}



#[derive(Debug)]
pub struct ModelManager {
	models: Vec<Model>,
	index_name: HashMap<String, usize>,
}
impl ModelManager {
	pub fn new() -> Self {
		Self {
			models: Vec::new(),
			index_name: HashMap::new(),
		}
	}

	pub fn index(&self, i: usize) -> &Model {
		&self.models[i]
	}

	pub fn index_name(&self, name: &String) -> usize {
		self.index_name[name]
	}

	pub fn insert(&mut self, model: Model) -> usize {
		let idx = self.models.len();
		self.index_name.insert(model.name.clone(), idx);
		self.models.push(model);
		idx
	}
}
