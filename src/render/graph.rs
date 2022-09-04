#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unreachable_code)]


use generational_arena::Index;
use serde::{Serialize, Deserialize};
use std::{collections::{HashMap, HashSet, BTreeMap}, time::Instant};
use crate::render::*;
use crate::gpu::GpuData;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;
use anyhow::*;




#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceType {
	Texture(ResourceDescriptor2),
	Buffer(ResourceDescriptor2),

	Meshes(VertexProperties, InstanceProperties),
	
	MaterialTextures((String, ResourceDescriptor2)),
	MaterialFloats((String, ResourceDescriptor2)),

	Float,
	Float3,
	FloatVec,
}



pub trait RunnableNode : Send + Sync {
	fn name(&self) -> &String;
	fn inputs(&self) -> &HashMap<String, Vec<ResourceType>>;
	fn outputs(&self) -> &HashMap<String, Vec<ResourceType>>;
	// update should pull mesh data and create texture data for run()
	// For performance it would be best to only check to reate the resources when initialized or resolution is changed, but I am too lazy
	fn update(&mut self, device: &wgpu::Device, graph_resources: &mut GraphResources, model_resources: &mut ModelQueuesResource, gpu_data: &mut GpuData);
	// Mutate context and encode rendering
	fn run(&self, graph_resources: &mut GraphResources, model_resources: &ModelQueuesResource, gpu_data: &GpuData, encoder: &mut wgpu::CommandEncoder);
}
impl std::fmt::Debug for dyn RunnableNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RunnableNode {}", self.name())
    }
}



pub struct GraphsContext {
	graphs: Vec<Box<dyn RunnableNode>>,
	loaded_graphs: Vec<PathBuf>,
	model_queues: ModelQueuesResource,
	graph_resources: GraphResources,
	camera_buffer_index: usize,
	ssao_buffer_index: usize,
}
impl GraphsContext {

	/// To be called if a graph is added or resolution changes
	pub fn graph_setup(
		&mut self, 
		device: &wgpu::Device,
		queue: &wgpu::Queue, 
		data: &mut crate::gpu::GpuData,
		new_resolution: Option<[u32; 2]>,
	) {
		if let Some(res) = new_resolution {
			self.graph_resources.default_resolution = res;
			// Update ssao
			SSAOUniform::new(res[0], res[1]).update_buffer(
				queue, 
				self.graph_resources.get_buffer(self.ssao_buffer_index),
			);
		}

		// Get model input formats
		let model_inputs: HashSet<(Vec<InstanceProperty>, Vec<VertexProperty>, BindGroupFormat)> = self.graphs.iter().flat_map(|graph| {
			graph.inputs().iter().filter_map(|(_, grt)| {
				match grt {
					ResourceType::Models(model_input) => Some(model_input.clone()),
					_ => None,
				}
			})
		}).collect::<HashSet<_>>();
		
		// Update model formats
		self.model_queues.update_formats(model_inputs);

		// Update graphs
		for graph in self.graphs.iter_mut() {
			graph.update(device, &mut self.graph_resources, &mut self.model_queues, data);
		}
	}

	pub fn set_models(
		&mut self, 
		device: &wgpu::Device, 
		queue: &wgpu::Queue, 
		data: &mut crate::gpu::GpuData,
		model_instances: Vec<ModelInstance>,
	) {
		let update_st = Instant::now();

		let new_graphs = {
			let materials = data.materials.data_manager.read().unwrap();
			model_instances.iter().filter_map(|model_instance| {
				let g = &materials.index(model_instance.material_idx).unwrap().graph;
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
				let graph = Box::new(example_graph_read(&graph_path, &mut data.shaders));
				self.graphs.push(graph);
				self.loaded_graphs.push(graph_path);
			}
			info!("Reinitializing graphs");
			self.graph_setup(device, queue, data, None);
		}
		
		self.model_queues.update_models(model_instances, data);
		self.model_queues.update_instances(device, 0.0);

		let _update_en = Instant::now() - update_st;
	}

	/// Renders some objects from the perspective of a camera
	pub fn encode_render(
		&mut self, 
		queue: &wgpu::Queue, 
		data: &mut crate::gpu::GpuData,
		mut encoder: &mut wgpu::CommandEncoder,
		dest: &wgpu::Texture, 
		width: u32,
		height: u32,
		camera: &Camera, 
		_t: Instant,
	) {
		// Update camera
		CameraUniform::new_from_camera(camera, width as f32, height as f32).update_buffer(
			queue, 
			self.graph_resources.get_buffer(self.camera_buffer_index),
		);

		// Run graphs
		for graph in &mut self.graphs {
			graph.run(
				&mut self.graph_resources, 
				&mut self.model_queues, 
				data, 
				&mut encoder,
			);
		}

		// // Copy output to destination
		// let output_texture = self.graph_resources.get_texture(
		// 	self.graph_resources.get_index_of_id(&"final".to_string(), ResourceType::Texture).unwrap()
		// );
		// encoder.copy_texture_to_texture(
		// 	wgpu::ImageCopyTextureBase { 
		// 		texture: &output_texture.texture, 
		// 		mip_level: 0, 
		// 		origin: wgpu::Origin3d::ZERO, 
		// 		aspect: wgpu::TextureAspect::All, 
		// 	}, 
		// 	wgpu::ImageCopyTextureBase { 
		// 		texture: dest, 
		// 		mip_level: 0, 
		// 		origin: wgpu::Origin3d::ZERO, 
		// 		aspect: wgpu::TextureAspect::All, 
		// 	},
		// 	output_texture.size,
		// );
	}
}






/// A structure which can be configured to hold the transient resources for a graph.
/// These are textures, buffers, and globals but not models. Definitely not models.
#[derive(Debug)]
pub struct GraphResources {
	resources_bgs: Vec<wgpu::BindGroup>,
	resource_bg_index_of_format: HashMap<BindGroupFormat, usize>,

	textures: Vec<BoundTexture>,
	textures_index_of_id: HashMap<String, usize>,
	
	buffers: Vec<wgpu::Buffer>,
	buffers_index_of_id: HashMap<String, usize>,

	pub default_resolution: [u32; 2],
}
impl GraphResources {
	pub fn new() -> Self {
		Self {
			resources_bgs: Vec::new(),
			resource_bg_index_of_format: HashMap::new(),
			textures: Vec::new(),
			textures_index_of_id: HashMap::new(),
			buffers: Vec::new(),
			buffers_index_of_id: HashMap::new(),
			default_resolution: [800, 600],
		}
	}

}



/// A node which contains more nodes
#[derive(Debug)]
pub struct GraphNode {
	name: String,
	nodes: Vec<Box<dyn RunnableNode>>,
	order: Vec<usize>,
	
	inputs: HashSet<(String, ResourceType)>,
	outputs: HashSet<(String, ResourceType)>,

	material_indices: Vec<Vec<usize>>,
	material_format_indices: HashMap<BindGroupFormat, usize>,
	mesh_indices: Vec<Vec<usize>>,
	mesh_format_indices: HashMap<(Vec<VertexProperty>, Vec<InstanceProperty>), usize>,
}
impl GraphNode {
	pub fn new(name: &String) -> Self {
		Self {
			name: name.clone(),
			nodes: Vec::new(),
			order: Vec::new(),
			inputs: HashSet::new(),
			outputs: HashSet::new(),
			material_indices: Vec::new(),
			material_format_indices: HashMap::new(),
			mesh_indices: Vec::new(),
			mesh_format_indices: HashMap::new(),
		}
	}
	
	/// Inserts a node into the graph, recalculates inputs/outputs for the new configuration
	pub fn add_node(&mut self, node: Box<dyn RunnableNode>) {
		// If already in thing then just push to order
		for i in 0..self.nodes.len() {
			if self.nodes[i].name() == node.name() {
				self.order.push(i);
				return
			}
		}
		self.order.push(self.nodes.len());
		self.nodes.push(node);
		// Better to update i/o here than in input()/output()
		let [i, o] = self.calculate_io();
		self.inputs = i;
		self.outputs = o;
	}

	/// Calculates the inputs/outputs of this graph (is not inexpensive, don't use it often)
	pub fn calculate_io(&self) -> [HashSet<(String, ResourceType)>; 2] {

		let mut collected_inputs = HashSet::new();
		let mut collected_outputs = HashSet::new();
		
		for node in &self.nodes {
			for input in node.inputs() {
				collected_inputs.insert(input.clone());
			}
			for output in node.outputs() {
				collected_outputs.insert(output.clone());
			}
		}

		// Filter out inputs that are created internally
		let inputs = collected_inputs.difference(&collected_outputs).cloned().collect::<HashSet<_>>();
		
		[inputs, collected_outputs]
	}
}
impl RunnableNode for GraphNode {
	fn name(&self) -> &String {
		&self.name
	}
	
	fn inputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.outputs
	}
	
	fn update(
		&mut self, 
		device: &wgpu::Device,
		graph_resources: &mut GraphResources, 
		model_resources: &mut ModelQueuesResource, 
		gpu_data: &mut GpuData,
	) {
		debug!("Updating graph {}", &self.name);
		for node in &mut self.nodes {
			node.update(device, graph_resources, model_resources, gpu_data);
		}
	}
	
	fn run(
		&self, 
		context: &mut GraphResources, 
		model_resources: &ModelQueuesResource,
		gpu_data: &GpuData, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running graph {}", &self.name);
		encoder.push_debug_group(&*format!("Graph node '{}'", &self.name));
		for i in &self.order {
			self.nodes[*i].run(context, model_resources, gpu_data, encoder);
		}
		encoder.pop_debug_group();
	}
}



#[derive(Debug, Serialize, Deserialize)]
struct GraphNodeSpecification {
	pub name: String,
	pub order: Vec<String>,
}



#[derive(Debug)]
enum NodeType {
	// Graph(GraphNode),
	Shader(ShaderNode),
	Texture(TextureNode),
}



#[derive(Debug, Serialize, Deserialize)]
enum NodeSpecificationType {
	// Graph(GraphNodeSpecification),
	Shader(ShaderNodeSpecification),
	Texture(TextureNodeSpecification)
}



#[derive(Debug)]
enum QueueType {
	Models(usize),	// Takes meshes and materials
	Meshes(usize),	// Takes only meshes
	FullQuad,		// Only operates on textures
}



/// A node which runs a shader
/// 
/// Currently a mesh queue is just a model queue with an empty material bgf.
/// This is bad.
/// in order to recify it a lot of code needs to be rewritten.
/// It shouldn't affect perfmormance that much so I will leave this until I'm desperate.
/// 
/// A buffer, named 'fug_buff', is used to convince wgpu to let me draw a fullscreen quad.
/// It would be ideal to just draw using vertex indices but wgpu wants a buffer to be bound.
#[derive(Debug)]
struct ShaderNode {
	name: String,
	shader_path: PathBuf,
	shader_idx: Index,

	// The input formts for this node, if it needs them
	mesh_input_format: Option<MeshInputFormat>,
	material_input_bgf: Option<BindGroupFormat>,
	resources_idx: Option<usize>,

	// Index of texture, should store bool
	colour_attachments: Vec<(usize, bool)>,
	depth_attachment: Option<(usize, bool)>,
	// The queue index (if initialized) (if necessary)
	render_queue: Option<QueueType>,

	inputs: HashSet<(String, ResourceType)>,
	depth: Option<String>,
	aliases: HashMap<String, String>,
	outputs: HashSet<(String, ResourceType)>,

	fugg_buff: Option<wgpu::Buffer>,
}
impl ShaderNode {
	pub fn from_spec(
		spec: &ShaderNodeSpecification, 
		folder_context: &Path, 
		shaders: &mut ShaderManager,
	) -> Result<Self> {
		let mut inputs = spec.render_inputs.clone();
		if let Some(depth_id) = &spec.depth {
			inputs.insert((depth_id.clone(), ResourceType::Texture));
		}

		let shader_path = folder_context.join(&spec.shader).canonicalize()
			.with_context(|| format!("Failed to canonicalize shader path ('{:?}' + '{:?}')", &folder_context, &spec.shader))?;
			
		let shader_idx = match shaders.index_from_path(&shader_path) {
			Some(idx) => idx,
			None => shaders.register_path(&shader_path),
		};
		let shader = shaders.shader_index(shader_idx);

		/*

		// Add globals input if needed
		if let Some(idx) = shader.resources_bg_index {
			let globals_bgf = ShaderNode::alias_filter(&spec.aliases, shader.bind_groups[&idx].format().clone());
			inputs.insert(("_globals".to_string(), GraphResourceType::Resources(globals_bgf)));
		}

		// Mesh input
		let mesh_input_format = {
			if shader.vertex_properties.len() > 0 || shader.instance_properties.len() > 0 {
				inputs.insert(("meshes".to_string(), GraphResourceType::Meshes(
					shader.vertex_properties.clone(), 
					shader.instance_properties.clone(), 
				)));
				Some((shader.vertex_properties.clone(), shader.instance_properties.clone()))
			} else {
				None
			}
		};
		
		// Material input
		let material_input_bgf = match shader.material_bg_index {
			Some(idx) => {
				inputs.insert(("materials".to_string(), GraphResourceType::Materials(
					shader.bind_groups[&idx].format(),
				)));
				Some(shader.bind_groups[&idx].format())
			},
			None => {
				// Currently a shader needs some material input to generate a queue
				// We still won't bind it or anything but it needs to be here
				inputs.insert(("materials".to_string(), GraphResourceType::Materials(
					BindGroupFormat::empty(),
				)));
				None
			},
		};
		
		// Add model input
		if mesh_input_format.is_some() {
			if material_input_bgf.is_some() {
				inputs.insert(("models".to_string(), GraphResourceType::Models((
					shader.instance_properties.clone(), 
					shader.vertex_properties.clone(), 
					shader.bind_groups[&1].format(),
				))));
			} else {
				inputs.insert(("models".to_string(), GraphResourceType::Models((
					shader.instance_properties.clone(), 
					shader.vertex_properties.clone(), 
					BindGroupFormat::empty(),
				))));
			}
		}
		*/

		Ok(Self {
			name: spec.name.clone(),
			shader_path: spec.shader.clone(),
			shader_idx,
			mesh_input_format: todo!(),
			material_input_bgf: todo!(),
			resources_idx: None,
			render_queue: None,
			colour_attachments: Vec::new(),
			depth_attachment: None,
			inputs,
			depth: spec.depth.clone(),
			aliases: spec.aliases.clone(),
			outputs: spec.outputs.clone(),
			fugg_buff: None,
		})
	}

	/// Checks if there is an alias for something
	fn alias_for(&self, s: &String) -> Option<&String> {
		if self.aliases.contains_key(s) {
			Some(&self.aliases[s])
		} else {
			None
		}
	}

	fn alias_filter_2(
		aliases: &HashMap<String, String>, 
		mut bind_groups: BTreeMap<u32, ShaderBindGroup>,
	) -> BTreeMap<u32, ShaderBindGroup> {
		bind_groups.iter_mut().for_each(|(_, bg)| {
			bg.entries.iter_mut().for_each(|bge| {
				if let Some(alias) = aliases.get(&bge.format.resource_usage) {
					bge.format.resource_usage = alias.clone();
				}
			});
		});
		bind_groups
	}

	/// Filters a bind group to use this node's aliases
	fn alias_filter(
		aliases: &HashMap<String, String>, 
		mut bgf: BindGroupFormat,
	) -> BindGroupFormat {
		for (_, bgef) in &mut bgf.entry_formats {
			if aliases.contains_key(&bgef.resource_usage) {
				let alias = aliases[&bgef.resource_usage].clone();
				trace!("Found alias '{}' -> '{}'", &bgef.resource_usage, &alias);
				bgef.resource_usage = alias;
			}
		}
		bgf
	}
}
impl RunnableNode for ShaderNode {
	fn name(&self) -> &String {
		&self.name
	}

	fn inputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.outputs
	}
	
	fn update(
		&mut self, 
		device: &wgpu::Device,
		graph_resources: &mut GraphResources, 
		model_resources: &mut ModelQueuesResource, 
		gpu_data: &mut GpuData,
	) {	
		/* 
		debug!("Updating shader node '{}'", &self.name);

		let shader = gpu_data.shaders.index(self.shader_idx);
		let instance_properties = shader.instance_properties.clone();
		let vertex_properties = shader.vertex_properties.clone();
		let globals_bgf = shader.bind_groups[&0].format();
		let materials_bgf = match shader.material_bg_index {
			Some(idx) => Some(shader.bind_groups[&idx].format().clone()),
			None => None,
		};
		drop(shader);

		let filtered_globals_bgf = ShaderNode::alias_filter(&self.aliases, globals_bgf);
		self.resources_idx = Some(
			graph_resources.resource_bg_of_format_create(&filtered_globals_bgf, gpu_data).unwrap()
		);
		trace!("Shader node {} chose globals idx {}", &self.name, &self.resources_idx.unwrap());

		// Update render queue
		self.render_queue = Some({
			// If takes mesh input find the mesh bit
			let takes_meshes = vertex_properties.len() > 0 || instance_properties.len() > 0;
			if takes_meshes {
				match materials_bgf {
					Some(materials_bgf) => {
						// Takes materials too, so model input
						let model_format = (instance_properties, vertex_properties, materials_bgf);
						let queue_index = model_resources.queue_index_of_format(&model_format).unwrap();
						trace!("Shader node {} chose model queue idx {} (format: {:?})", &self.name, &queue_index, &model_format);
						QueueType::Models(queue_index)
					},
					None => {
						// Only mesh input
						let model_format = (instance_properties, vertex_properties, BindGroupFormat::empty());
						let queue_index = match model_resources.queue_index_of_format(&model_format) {
							Some(idx) => idx,
							None => {
								// This should really not be done here but whatever
								// model_resources.add_format(&model_format, render_resources)
								panic!("I told you not to do that anymore!")
							}
						};
						trace!("Shader node {} chose mesh queue idx {} (format: {:?})", &self.name, &queue_index, &model_format);
						QueueType::Meshes(queue_index)
					},
				}
			} else {
				trace!("Shader node {} uses FullQuad", &self.name);
				QueueType::FullQuad
			}
		});

		let shader = gpu_data.shaders.index(self.shader_idx);
		self.colour_attachments = shader.attachments.iter().map(|attachment| {
			let resource_name = {
				match self.alias_for(&attachment.usage) {
					Some(alias) => alias.clone(),
					None => attachment.usage.clone(),
				}
			};
			let attachment_key = (resource_name.clone(), GraphResourceType::Texture);
			if self.inputs.contains(&attachment_key) {
				let store = self.outputs.contains(&attachment_key);
				let idx = graph_resources.get_index_of_id(&attachment_key.0, GraphResourceType::Texture).expect("Attachment not found!");
				(idx, store)
			} else {
				panic!("Shader requires attachment not found in node inputs!")
			}
		}).collect::<Vec<_>>();

		self.depth_attachment = match &self.depth {
			Some(depth_id) => {
				let depth_key = (depth_id.clone(), GraphResourceType::Texture);
				let depth_write = self.outputs.contains(&depth_key);
				let idx = graph_resources.get_index_of_id(&depth_key.0, GraphResourceType::Texture).expect("Depth attatchment not found!?");
				Some((idx, depth_write))
			},
			_ => None,
		};

		// Workaround buffer for FullQuad because wgpu needs something to be bound in vertex and instance in order to draw
		if self.fugg_buff.is_none() {
			self.fugg_buff = Some(gpu_data.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("fugg Buffer"),
				contents: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
				usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
			}));
		}
		*/
	}
	
	fn run(
		&self, 
		context: &mut GraphResources, 
		model_resources: &ModelQueuesResource,
		data: &GpuData, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running shader node {}", &self.name);
		encoder.push_debug_group(&*format!("Shader node '{}'", &self.name));
		
		let shader = data.shaders.shader_index(self.shader_idx);

		let colour_attachments = self.colour_attachments.iter().cloned().map(|(idx, store)| {
			Some(wgpu::RenderPassColorAttachment {
				view: &context.get_texture(idx).view,
				resolve_target: None, // Same as view unless using multisampling
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store,
				},
			})
		}).collect::<Vec<_>>();

		let depth_stencil_attachment = match self.depth_attachment.clone() {
			Some((idx, store)) => {
				Some(wgpu::RenderPassDepthStencilAttachment {
					view: &context.get_texture(idx).view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Load,
						store,
					}),
					stencil_ops: None,
				})
			},
			_ => None,
		};

		let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some(&*self.name),
			color_attachments: &colour_attachments[..],
			depth_stencil_attachment,
		});

		// render_pass.set_pipeline(&shader.pipeline);

		// Globals
		if let Some(idx) = self.resources_idx {
			let globals_bind_group = context.resource_bg(idx);
			render_pass.set_bind_group(0, globals_bind_group, &[]);
		}

		match self.render_queue.as_ref().unwrap() {
			QueueType::Models(models_queue_idx) => {
				trace!("This uses a model queue");
				let (instances_idx, models) = model_resources.queue(*models_queue_idx);

				// Instances
				let instance_buffer = model_resources.instances_buffer(*instances_idx);
				render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

				let mut instance_position = 0;
				for (material_idx, mesh_idx, instance_count) in models {
					// Material
					let material = data.materials.index(*material_idx).unwrap();
					render_pass.set_bind_group(1, &material.bind_group, &[]);
					
					// Mesh
					let mesh = data.meshes.index(*mesh_idx).unwrap();
					render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
					render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
					
					// Draw
					let instance_end = instance_position + instance_count;
					render_pass.draw_indexed(0..mesh.n_vertices, 0, instance_position..instance_end);
					instance_position = instance_end;
				}
			},
			QueueType::Meshes(meshes_queue_idx) => {
				// Mesh queues are just model queues with empty material bgf
				// It's a crude workaround but it works around
				trace!("This uses a mesh queue");
				let (instances_idx, models) = model_resources.queue(*meshes_queue_idx);
				
				// Instances
				let instance_buffer = model_resources.instances_buffer(*instances_idx);
				render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

				let mut instance_position = 0;
				for (_, mesh_idx, instance_count) in models {
					// Mesh
					let mesh = data.meshes.index(*mesh_idx).unwrap();
					render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
					render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
					
					// Draw
					let instance_end = instance_position + instance_count;
					render_pass.draw_indexed(0..mesh.n_vertices, 0, instance_position..instance_end);
					instance_position = instance_end;
				}
			},
			QueueType::FullQuad => {
				// This must be a fullquad type game.
				trace!("This must be a fullquad type game.");

				let g = self.fugg_buff.as_ref().unwrap();
				render_pass.set_vertex_buffer(0, g.slice(..));
				render_pass.set_vertex_buffer(1, g.slice(..));
				render_pass.draw(0..3, 0..1);
			},
		}

		drop(render_pass);
		encoder.pop_debug_group();
	}
}



/// Serializable shader node info
#[derive(Debug, Serialize, Deserialize)]
struct ShaderNodeSpecification {
	pub name: String,
	pub shader: PathBuf,
	// Things pulled from global data and put into this shader's input places
	pub render_inputs: HashSet<(String, ResourceType)>,	// ("resource name", resource type)
	pub depth: Option<String>,
	// When assigning resources to the shader inputs we look to see if the input name exists as an alias
	// If no match is found we look through inputs as a fallback, avoiding manadtory overspecification
	pub aliases: HashMap<String, String>,	// ("name in resources", "name in shader")
	pub outputs: HashSet<(String, ResourceType)>,
}



#[derive(Debug, Serialize, Deserialize)]
pub struct TextureNodeSpecification {
	pub name: String,
	pub resource_name: String,
	pub texture_format: TextureFormat,
	pub resolution: Option<[u32; 2]>,	// Takes render res if none
	pub fill_with: Option<Vec<f32>>,
}



/// Creates a texture or clears it if it already exists
#[derive(Debug)]
pub struct TextureNode {
	name: String,
	resource_id: String,
	texture_format: TextureFormat,
	resolution: Option<[u32; 2]>,
	fill_with: Option<Vec<f32>>,

	texture_idx: usize,

	inputs: HashSet<(String, ResourceType)>,
	outputs: HashSet<(String, ResourceType)>,
}
impl TextureNode {
	pub fn from_spec(spec: &TextureNodeSpecification) -> Self {
		// error!("New texturenode '{}'", &spec.name);
		Self {
			name: spec.name.clone(),
			resource_id: spec.resource_name.clone(),
			texture_format: spec.texture_format,
			resolution: spec.resolution,
			fill_with: spec.fill_with.clone(),
			inputs: HashSet::new(),
			outputs: [(spec.resource_name.clone(), ResourceType::Texture)].iter().cloned().collect::<HashSet<_>>(),
			texture_idx: 0,
		}
	}

	fn create_texture(
		&self,
		device: &wgpu::Device,
		context: &mut GraphResources, 
		gpu_data: &mut GpuData, 
	) -> usize {
		// debug!("Creating texture for texture node '{}'", &self.name);
		let [width, height] = match self.resolution {
			Some(r) => r,
			None => context.default_resolution,
		};
		let t = BoundTexture::new(
			device,
			self.texture_format,
			width,
			height,
			1,
			&self.resource_id,
			// Todo: derive this from the graph
			wgpu::TextureUsages::RENDER_ATTACHMENT
				| wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::COPY_DST,
		);
		context.insert_texture(t, &self.resource_id)
	}
}
impl RunnableNode for TextureNode {
	fn name(&self) -> &String {
		&self.name
	}
	
	fn inputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.outputs
	}

	fn update(&mut self, device: &wgpu::Device, context: &mut GraphResources, _: &mut ModelQueuesResource, gpu_data: &mut GpuData) {
		self.texture_idx = self.create_texture(device, context, gpu_data);
		debug!("Texture node '{}' pulled texture idx {}", &self.name, &self.texture_idx);
	}

	fn run(
		&self, 
		context: &mut GraphResources, 
		_: &ModelQueuesResource,
		_: &GpuData, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running texture node '{}'", &self.name);
		encoder.push_debug_group(&*format!("Texture node '{}'", &self.name));

		// Fill data if needed
		if let Some(fill_with) = &self.fill_with {
			let texxy = context.get_texture(self.texture_idx);
			let colour_attachments = match self.texture_format.is_depth() {
				true => vec![],
				false => vec![
					Some(wgpu::RenderPassColorAttachment {
						view: &texxy.view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color {
								r: fill_with[0] as f64,
								g: fill_with[1] as f64,
								b: fill_with[2] as f64,
								a: fill_with[3] as f64,
							}),
							store: true,
						},
					}),
				],
			};
	
			let depth_stencil_attachment = match self.texture_format.is_depth() {
				true => Some(
					wgpu::RenderPassDepthStencilAttachment {
						view: &texxy.view,
						depth_ops: Some(wgpu::Operations {
							load: wgpu::LoadOp::Clear(fill_with[0]),
							store: true,
						}),
						stencil_ops: None,
					}
				),
				false => None,
			};
			
			// Render pass to clear the texture
			encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some(&*self.name),
				color_attachments: &colour_attachments[..],
				depth_stencil_attachment,
			});
		}

		encoder.pop_debug_group();
	}
}



pub fn example_graph_read(
	path: &PathBuf,
	shaders: &mut ShaderManager,
) -> GraphNode {
	info!("Reading graph file {:?}", path);
	let f = std::fs::File::open(path).expect("Failed to open file");
	let (graph_specification, node_specifications): (GraphNodeSpecification, Vec<NodeSpecificationType>) = ron::de::from_reader(f)
		.expect("Failed to read graph ron file");

	let mut graph = GraphNode::new(&graph_specification.name);

	let context = path.parent().unwrap();
	// Add nodes
	for name in graph_specification.order {
		for ns in &node_specifications {
			match ns {
				NodeSpecificationType::Shader(spec) => {
					if name == spec.name {
						graph.add_node(Box::new(ShaderNode::from_spec(spec, context, shaders).unwrap()));
					}
				},
				NodeSpecificationType::Texture(spec) => {
					if name == spec.name {
						graph.add_node(Box::new(TextureNode::from_spec(spec)));
					}
				},
			};
		}
	}

	graph
}



// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	#[test]
// 	fn test_thing() {
// 		//read_graph_file(&PathBuf::from("resources/graphs/kdefault.ron"));

// 		let g = GraphNodeSpecification {
// 			name: "fug".into(),
// 			order: vec![],
// 		};
// 		let ns = vec![
// 			NodeSpecificationType::Shader(ShaderNodeSpecification {
// 				name: "fug".into(),
// 				shader: "g".into(),
// 				render_inputs: HashSet::from([
// 					("g".into(), GraphResourceType::Texture),
// 				]),
// 				aliases: HashMap::new(),
// 				outputs: HashSet::new(),
// 			})
// 		];
// 		let gns = (g, ns);
		
// 		let pretty = ron::ser::PrettyConfig::new().depth_limit(3).separate_tuple_members(true).enumerate_arrays(false);
// 		let s = ron::ser::to_string_pretty(&gns, pretty).expect("Serialization failed");
// 		println!("{}", &s);

// 		let f: (GraphNodeSpecification, Vec<NodeSpecificationType>) = ron::de::from_str(&s)
// 		.expect("Failed to read graph ron file");
// 		println!("{:?}", &f);
		
// 		assert!(true);
// 	}
// }

#[derive(Debug)]
enum ResourceLocation {
	Global,
	MaterialQueue(Vec<String>),
	BuffersQueue(Vec<String>),
}


#[derive(Debug)]
struct PolygonShaderNode {
	name: String,
	shader_path: PathBuf,
	shader_idx: Index,

	// The mesh input descriptor and its corresponding mesh queue index
	mesh_input_format: Option<(ShaderMeshFormat, Option<usize>)>,

	// Material input queue
	
	bind_group_things: Vec<Option<(ResourceLocation, BindGroupFormat)>>,
	// Stored indices to bind group data
	bind_group_indices: Vec<Option<(ResourceType, usize)>>,

	// Index of texture, should store bool
	colour_attachments: Vec<(usize, bool)>,
	depth_attachment: Option<(usize, bool)>,
	// The queue index (if initialized) (if necessary)
	

	inputs: HashSet<(String, ResourceType)>,
	depth: Option<String>,
	aliases: HashMap<String, String>,
	outputs: HashSet<(String, ResourceType)>,

	fugg_buff: Option<wgpu::Buffer>,
}
impl PolygonShaderNode {
	pub fn from_spec(
		spec: &ShaderNodeSpecification, 
		folder_context: &Path, 
		shaders: &mut ShaderManager,
	) -> Result<Self> {
		let mut inputs = spec.render_inputs.clone();
		if let Some(depth_id) = &spec.depth {
			inputs.insert((depth_id.clone(), ResourceType::Texture));
		}

		let shader_path = folder_context.join(&spec.shader).canonicalize()
			.with_context(|| format!("Failed to canonicalize shader path ('{:?}' + '{:?}')", &folder_context, &spec.shader))?;
		let shader_idx = match shaders.index_from_path(&shader_path) {
			Some(idx) => idx,
			None => shaders.register_path(&shader_path),
		};
		let shader = shaders.shader_index(shader_idx);

		/*
		match shader.pipeline {
			ShaderPipeline::Compute(pl) => {},
			ShaderPipeline::Polygon{
				pipeline,
				mesh_format,
				attachments,
			} => {
				let accepts_meshes = mesh_format.is_some();
				// let skinned = shader.bind_groups.iter().

			},
		}

		// Add globals input if needed
		if let Some(idx) = shader.resources_bg_index {
			let globals_bgf = ShaderNode::alias_filter(&spec.aliases, shader.bind_groups[&idx].format().clone());
			inputs.insert(("_globals".to_string(), GraphResourceType::Resources(globals_bgf)));
		}
		
		// Mesh input
		let mesh_input_format = {
			if shader.vertex_properties.len() > 0 || shader.instance_properties.len() > 0 {
				inputs.insert(("meshes".to_string(), GraphResourceType::Meshes(
					shader.vertex_properties.clone(), 
					shader.instance_properties.clone(), 
				)));
				Some((shader.vertex_properties.clone(), shader.instance_properties.clone()))
			} else {
				None
			}
		};
		
		// Material input
		let material_input_bgf = match shader.material_bg_index {
			Some(idx) => {
				inputs.insert(("materials".to_string(), GraphResourceType::Materials(
					shader.bind_groups[&idx].format(),
				)));
				Some(shader.bind_groups[&idx].format())
			},
			None => {
				// Currently a shader needs some material input to generate a queue
				// We still won't bind it or anything but it needs to be here
				inputs.insert(("materials".to_string(), GraphResourceType::Materials(
					BindGroupFormat::empty(),
				)));
				None
			},
		};
		*/

		// // Add model input
		// if mesh_input_format.is_some() {
		// 	if material_input_bgf.is_some() {
		// 		inputs.insert(("models".to_string(), GraphResourceType::Models((
		// 			shader.instance_properties.clone(), 
		// 			shader.vertex_properties.clone(), 
		// 			shader.bind_groups[&1].format(),
		// 		))));
		// 	} else {
		// 		inputs.insert(("models".to_string(), GraphResourceType::Models((
		// 			shader.instance_properties.clone(), 
		// 			shader.vertex_properties.clone(), 
		// 			BindGroupFormat::empty(),
		// 		))));
		// 	}
		// }

		Ok(Self {
			name: spec.name.clone(),
			shader_path: spec.shader.clone(),
			shader_idx,
			mesh_input_format: todo!(),
			// material_input_bgf,
			// resources_idx: None,
			// render_queue: None,
			colour_attachments: Vec::new(),
			depth_attachment: None,
			inputs,
			depth: spec.depth.clone(),
			aliases: spec.aliases.clone(),
			outputs: spec.outputs.clone(),
			fugg_buff: None,

			// Temporary stuff
			bind_group_things: vec![],
			bind_group_indices: vec![],
		})
	}

	/// Checks if there is an alias for something
	fn alias_for(&self, s: &String) -> Option<&String> {
		if self.aliases.contains_key(s) {
			Some(&self.aliases[s])
		} else {
			None
		}
	}

	fn alias_filter_2(
		aliases: &HashMap<String, String>, 
		mut bind_groups: BTreeMap<u32, ShaderBindGroup>,
	) -> BTreeMap<u32, ShaderBindGroup> {
		bind_groups.iter_mut().for_each(|(_, bg)| {
			bg.entries.iter_mut().for_each(|bge| {
				if let Some(alias) = aliases.get(&bge.format.resource_usage) {
					bge.format.resource_usage = alias.clone();
				}
			});
		});
		bind_groups
	}

	/// Filters a bind group to use this node's aliases
	fn alias_filter(
		aliases: &HashMap<String, String>, 
		mut bgf: BindGroupFormat,
	) -> BindGroupFormat {
		for (_, bgef) in &mut bgf.entry_formats {
			if aliases.contains_key(&bgef.resource_usage) {
				let alias = aliases[&bgef.resource_usage].clone();
				trace!("Found alias '{}' -> '{}'", &bgef.resource_usage, &alias);
				bgef.resource_usage = alias;
			}
		}
		bgf
	}
}
impl RunnableNode for PolygonShaderNode {
	fn name(&self) -> &String {
		&self.name
	}

	fn inputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, ResourceType)> {
		&self.outputs
	}
	
	fn update(
		&mut self, 
		device: &wgpu::Device,
		graph_resources: &mut GraphResources, 
		model_resources: &mut ModelQueuesResource, 
		gpu_data: &mut GpuData,
	) {	
		debug!("Updating shader node '{}'", &self.name);

		/*
		let shader = gpu_data.shaders.index(self.shader_idx);
		let instance_properties = shader.instance_properties.clone();
		let vertex_properties = shader.vertex_properties.clone();
		let globals_bgf = shader.bind_groups[&0].format();
		let materials_bgf = match shader.material_bg_index {
			Some(idx) => Some(shader.bind_groups[&idx].format().clone()),
			None => None,
		};
		drop(shader);

		let filtered_globals_bgf = ShaderNode::alias_filter(&self.aliases, globals_bgf);
		self.resources_idx = Some(
			graph_resources.resource_bg_of_format_create(&filtered_globals_bgf, gpu_data).unwrap()
		);
		trace!("Shader node {} chose globals idx {}", &self.name, &self.resources_idx.unwrap());

		// Update render queue
		self.render_queue = Some({
			// If takes mesh input find the mesh bit
			let takes_meshes = vertex_properties.len() > 0 || instance_properties.len() > 0;
			if takes_meshes {
				match materials_bgf {
					Some(materials_bgf) => {
						// Takes materials too, so model input
						let model_format = (instance_properties, vertex_properties, materials_bgf);
						let queue_index = model_resources.queue_index_of_format(&model_format).unwrap();
						trace!("Shader node {} chose model queue idx {} (format: {:?})", &self.name, &queue_index, &model_format);
						QueueType::Models(queue_index)
					},
					None => {
						// Only mesh input
						let model_format = (instance_properties, vertex_properties, BindGroupFormat::empty());
						let queue_index = match model_resources.queue_index_of_format(&model_format) {
							Some(idx) => idx,
							None => {
								// This should really not be done here but whatever
								// model_resources.add_format(&model_format, render_resources)
								panic!("I told you not to do that anymore!")
							}
						};
						trace!("Shader node {} chose mesh queue idx {} (format: {:?})", &self.name, &queue_index, &model_format);
						QueueType::Meshes(queue_index)
					},
				}
			} else {
				trace!("Shader node {} uses FullQuad", &self.name);
				QueueType::FullQuad
			}
		});

		let shader = gpu_data.shaders.index(self.shader_idx);
		self.colour_attachments = shader.attachments.iter().map(|attachment| {
			let resource_name = {
				match self.alias_for(&attachment.usage) {
					Some(alias) => alias.clone(),
					None => attachment.usage.clone(),
				}
			};
			let attachment_key = (resource_name.clone(), GraphResourceType::Texture);
			if self.inputs.contains(&attachment_key) {
				let store = self.outputs.contains(&attachment_key);
				let idx = graph_resources.get_index_of_id(&attachment_key.0, GraphResourceType::Texture).expect("Attachment not found!");
				(idx, store)
			} else {
				panic!("Shader requires attachment not found in node inputs!")
			}
		}).collect::<Vec<_>>();

		self.depth_attachment = match &self.depth {
			Some(depth_id) => {
				let depth_key = (depth_id.clone(), GraphResourceType::Texture);
				let depth_write = self.outputs.contains(&depth_key);
				let idx = graph_resources.get_index_of_id(&depth_key.0, GraphResourceType::Texture).expect("Depth attatchment not found!?");
				Some((idx, depth_write))
			},
			_ => None,
		};

		// Workaround buffer for FullQuad because wgpu needs something to be bound in vertex and instance in order to draw
		if self.fugg_buff.is_none() {
			self.fugg_buff = Some(gpu_data.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("fugg Buffer"),
				contents: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
				usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
			}));
		}
	
		*/
	}
	
	fn run(
		&self, 
		context: &mut GraphResources, 
		model_resources: &ModelQueuesResource,
		data: &GpuData, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running shader node {}", &self.name);
		encoder.push_debug_group(&*format!("Shader node '{}'", &self.name));
		
		let shader = data.shaders.shader_index(self.shader_idx);

		let colour_attachments = self.colour_attachments.iter().cloned().map(|(idx, store)| {
			Some(wgpu::RenderPassColorAttachment {
				view: &context.get_texture(idx).view,
				resolve_target: None, // Same as view unless using multisampling
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store,
				},
			})
		}).collect::<Vec<_>>();

		let depth_stencil_attachment = match self.depth_attachment.clone() {
			Some((idx, store)) => {
				Some(wgpu::RenderPassDepthStencilAttachment {
					view: &context.get_texture(idx).view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Load,
						store,
					}),
					stencil_ops: None,
				})
			},
			_ => None,
		};

		/* 
		let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some(&*self.name),
			color_attachments: &colour_attachments[..],
			depth_stencil_attachment,
		});

		render_pass.set_pipeline(&shader.pipeline);

		// Globals
		if let Some(idx) = self.resources_idx {
			let globals_bind_group = context.resource_bg(idx);
			render_pass.set_bind_group(0, globals_bind_group, &[]);
		}

		match self.render_queue.as_ref().unwrap() {
			QueueType::Models(models_queue_idx) => {
				trace!("This uses a model queue");
				let (instances_idx, models) = model_resources.queue(*models_queue_idx);

				// Instances
				let instance_buffer = model_resources.instances_buffer(*instances_idx);
				render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

				let mut instance_position = 0;
				for (material_idx, mesh_idx, instance_count) in models {
					// Material
					let material = data.materials.index(*material_idx);
					render_pass.set_bind_group(1, &material.bind_group, &[]);
					
					// Mesh
					let mesh = data.meshes.index(*mesh_idx);
					render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
					render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
					
					// Draw
					let instance_end = instance_position + instance_count;
					render_pass.draw_indexed(0..mesh.n_vertices, 0, instance_position..instance_end);
					instance_position = instance_end;
				}
			},
			QueueType::Meshes(meshes_queue_idx) => {
				// Mesh queues are just model queues with empty material bgf
				// It's a crude workaround but it works around
				trace!("This uses a mesh queue");
				let (instances_idx, models) = model_resources.queue(*meshes_queue_idx);
				
				// Instances
				let instance_buffer = model_resources.instances_buffer(*instances_idx);
				render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

				let mut instance_position = 0;
				for (_, mesh_idx, instance_count) in models {
					// Mesh
					let mesh = data.meshes.index(*mesh_idx);
					render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
					render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
					
					// Draw
					let instance_end = instance_position + instance_count;
					render_pass.draw_indexed(0..mesh.n_vertices, 0, instance_position..instance_end);
					instance_position = instance_end;
				}
			},
			QueueType::FullQuad => {
				// This must be a fullquad type game.
				trace!("This must be a fullquad type game.");

				let g = self.fugg_buff.as_ref().unwrap();
				render_pass.set_vertex_buffer(0, g.slice(..));
				render_pass.set_vertex_buffer(1, g.slice(..));
				render_pass.draw(0..3, 0..1);
			},
		}

		drop(render_pass);
		encoder.pop_debug_group();
		*/
	}
}