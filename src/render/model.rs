use crate::render::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wgpu::util::DeviceExt;
// use rapier3d::prelude::*;
// use anyhow::*;
// use crate::mesh::*;
// use crate::material::*;



#[derive(Debug)]
pub struct ModelInstance {
	// Indices to the UNBOUND mesh and material
	pub material_idx: usize,
	pub mesh_idx: usize,
	pub instance: Instance,
}



// #[derive(Debug)]
// pub struct Model {
// 	pub material_idx: usize,
// 	pub mesh_idx: usize,
// }
// impl Model {
// 	pub fn make_collider(
// 		&self,
// 		meshes: &MeshManager,
// 		materials: &MaterialManager,
// 	) -> Result<Collider> {
// 		let shape = meshes.index(self.mesh_idx).collider_shape.unwrap().clone();
// 		let mut collider = ColliderBuilder::new(shape);
// 		let material = materials.index(self.material_idx);
// 		if material.floats.contains_key("restitution") {
// 			let restitution = material.floats["restitution"][0];
// 			collider = collider.restitution(restitution);
// 		}
// 		if material.floats.contains_key("friction") {
// 			let friction = material.floats["friction"][0];
// 			collider = collider.friction(friction);
// 		}
		
// 		Ok(collider.build())
// 	}
// }



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



/// Shader inputs are simplified to instance properties, vertex properties, and material format
pub type ShaderInput = (InstanceProperties, VertexProperties, BindGroupFormat);
/// instance buffer idx, [material idx, mesh idx, count])
pub type ModelQueue = (usize, Vec<(usize, usize, u32)>);
/// Instance idx, mesh, count
pub type MeshQueue = (usize, Vec<(usize, u32)>);

pub fn meshq_from_modelq(model_queue: &ModelQueue) -> MeshQueue {
	let (instance_buffer_idx, model_stuff) = model_queue;

	let mut mesh_queue_1 = Vec::new();
	let mut prev_mesh = model_stuff[0].1;
	let mut count = 0;
	for (_mat, mesh, model_count) in model_stuff {
		if *mesh == prev_mesh {
			count += *model_count;
		} else {
			mesh_queue_1.push((*mesh, count));
			count = 0;
			prev_mesh = *mesh;
		}
	}
	
	(*instance_buffer_idx, mesh_queue_1)
}

pub fn modelq_from_meshq(mesh_queue: MeshQueue, materials: Vec<usize>) -> ModelQueue {

	let (instance_buffer_idx, mesh_stuff) = mesh_queue;

	// Expand mesh indices because I'm not smart enough to firgure out the other way
	let mut expanded_mesh_indices = Vec::new();
	for (i, c) in mesh_stuff {
		for _ in 0..c {
			expanded_mesh_indices.push(i)
		}
	}

	// Check that we can map the two lists
	if expanded_mesh_indices.len() != materials.len() {
		panic!("Meshes and materials are not of same length!");
	}

	let mut model_queue_1 = Vec::new();
	
	// For each material
	let mut i = 0;
	while i < materials.len() {

		// Find the length of the segment where material and mesh are the same
		let mut count = 0;
		while expanded_mesh_indices[i + count as usize] == expanded_mesh_indices[i] && materials[i + count as usize] == materials[i] {
			count += 1;
		}

		model_queue_1.push((materials[i], expanded_mesh_indices[i], count));

		i += count as usize;
	}

	(instance_buffer_idx, model_queue_1)
}





/// The model resource maps shader input to a model queue.
/// Function call order should be (update_formats -> update_models -> update_instances).
/// 
/// Currently instancing has been neglected and vector capacity has been optimized.
/// If you want to change that, you must fight the allocator!
/// 
/// Instances have been set up to support interpolation based on time.
/// This could be useful in the future.
#[derive(Debug)]
pub struct ModelsQueueResource {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,

	raw_models: Vec<ModelInstance>,
	queues: Vec<ModelQueue>,
	queue_index_of_format: HashMap<ShaderInput, usize>,

	instances_buffers: Vec<wgpu::Buffer>,
	instances_buffer_index_of_format: HashMap<InstanceProperties, usize>,
}
impl ModelsQueueResource {
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

	/// Update the list of formats in which models should be provided
	pub fn update_formats(
		&mut self, 
		formats: HashSet<ShaderInput>, 
	) {
		// Resize queues
		self.queue_index_of_format = formats.iter().cloned().enumerate().map(|(i, f)| (f, i)).collect::<HashMap<_,_>>();
		// Create any needed new queues assuming a length equal to the last known model set
		self.queues.resize(formats.len(), (0, Vec::with_capacity(self.raw_models.len())));

		// Resize instances
		// Does not add or remove buffers
		self.instances_buffer_index_of_format = formats.iter().map(|(ip, _, _)| {
			(ip.clone(), 0)
		}).collect::<HashMap<_,_>>();
	}

	/// Adds a format, compiling models and instances, without prior setup.
	/// Calling this at all will cause at least one model resource reallocation.
	/// Please just don't use this.
	#[deprecated]
	pub fn add_format(
		&mut self,
		format: &ShaderInput,
		resources: &mut RenderResources,
	) -> usize {
		warn!("Adding format without setup");
		match self.queue_index_of_format(format) {
			Some(idx) => idx,
			None => {
				let instance_idx = {
					let mut instances_data = Vec::new();
					for model in &self.raw_models {
						let instance_data = model.instance.data(&format.0);
						instances_data.extend_from_slice(&instance_data[..]);
					}

					let instances_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
						label: Some("Instance Buffer"),
						contents: &instances_data[..],
						usage: wgpu::BufferUsages::VERTEX,
					});

					let idx = self.instances_buffers.len();
					self.instances_buffer_index_of_format.insert(format.0.clone(), idx);
					self.instances_buffers.push(instances_buffer);
					idx
				};

				let queue_idx = {
					// Once again neglect instancing for simplicity
					let queue_contents = self.raw_models.iter().map(|model| {
						let mesh_idx = resources.meshes.index_from_index_properites_bind(model.mesh_idx, &format.1);
						let material_idx = resources.materials.index_from_index_format_bind(model.material_idx, &format.2, &mut resources.shaders, &mut resources.textures);
						let count = 1;
						(material_idx, mesh_idx, count)
					}).collect::<Vec<_>>();
					let idx = self.queues.len();
					self.queue_index_of_format.insert(format.clone(), idx);
					self.queues.push((instance_idx, queue_contents));
					idx
				};

				queue_idx
			}
		}
	}

	/// Update the models that might be needed, binding each for each format
	pub fn update_models(
		&mut self, 
		models: Vec<ModelInstance>,
		resources: &mut RenderResources,
	) {
		for ((_, vp, mbgf), &queue_idx) in self.queue_index_of_format.iter() {
			// I don't want to allocate a new vector
			// The existing vector should either be extended or truncated
			let queue_content = &mut self.queues.get_mut(queue_idx).unwrap().1;
			queue_content.resize(models.len(), (0,0,0));

			// Another way of doing stuff
			// queue_content.iter_mut().enumerate().map(|(i, c)| {
			// 	c.0 = resources.materials.index_from_index_format_bind(models[i].material_idx, mbgf, &mut resources.shaders, &mut resources.textures);
			// 	c.1 = resources.meshes.index_from_index_properites_bind(models[i].mesh_idx, vp);
			// 	c.2 = 1;
			// });

			models.iter().enumerate().for_each(|(i, model)| {
				let mesh_idx = resources.meshes.index_from_index_properites_bind(model.mesh_idx, vp);
				let material_idx = resources.materials.index_from_index_format_bind(model.material_idx, mbgf, &mut resources.shaders, &mut resources.textures);
				let count = 1;
				queue_content[i] = (material_idx, mesh_idx, count);
			});
		}
		self.raw_models = models;
	}

	/// Updates the instances of the loaded models.
	/// Is meant to interpolate between two timesteps, currently does not.
	pub fn update_instances(
		&mut self, 
		_t: f32,
	) {
		// This is usually very small so I'm okay with reallocation
		self.instances_buffers = Vec::with_capacity(self.queue_index_of_format.len());

		for (instance_properties, _, _) in self.queue_index_of_format.keys() {
			// Find the size of the vector we should allocate
			let mut instances_entry_length = 0;
			for instance_property in instance_properties {
				let attributes = match instance_property {
					InstanceProperty::InstanceModelMatrix => InstanceModelMatrix::attributes(),
					InstanceProperty::InstanceColour => InstanceColour::attributes(),
				};
				for (size, _) in attributes {
					instances_entry_length += *size;
				}
			}

			// Allocate intermediate vector and fill
			let mut instances_data = Vec::with_capacity(instances_entry_length * self.raw_models.len());
			for model in &self.raw_models {
				instances_data.append(&mut model.instance.data(&instance_properties));
			}

			// Load into buffer
			let instances_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("Instance Buffer"),
				contents: &instances_data[..],
				usage: wgpu::BufferUsages::VERTEX,
			});

			// Overwrite old entry
			let idx = self.instances_buffers.len();
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




