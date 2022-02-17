use serde::{Serialize, Deserialize};
use std::{collections::{HashMap, HashSet}, sync::Arc};
use crate::render::*;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;
use anyhow::*;


/*
A graph is a DAG of nodes which mutate a context
There is incredible potential for optimization here
(intermediate resource creation, resource aliasing, etc)
You will not use any of those optimizations here
Except for the necessary ones, as rendering must be fast
Just make it work please
*/



pub trait RunnableNode : Send + Sync {
	fn name(&self) -> &String;
	fn inputs(&self) -> &HashSet<(String, GraphResourceType)>;
	fn outputs(&self) -> &HashSet<(String, GraphResourceType)>;
	// update should pull mesh data and create texture data for run()
	// For performance it would be best to only check to reate the resources when initialized or resolution is changed, but I am too lazy
	fn update(&mut self, graph_resources: &mut GraphLocals, model_resources: &mut ModelsQueueResource, render_resources: &mut RenderResources);
	// Mutate context and encode rendering
	fn run(&self, graph_resources: &mut GraphLocals, model_resources: &ModelsQueueResource, render_resources: &mut RenderResources, encoder: &mut wgpu::CommandEncoder);
}
impl std::fmt::Debug for dyn RunnableNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RunnableNode {}", self.name())
    }
}



#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum GraphResourceType {
	Resources(BindGroupFormat),
	Meshes(VertexProperties, InstanceProperties),
	Materials(BindGroupFormat),
	Models(ShaderInput), // Don't use this it's bad
	Light(Vec<LightType>),
	Buffer,
	Texture,
	TextureArray,
	ArrayTexture,
	Float,
	Float3,
	FloatVec,
}



/// A structure which can be configured to hold the transient resources for a graph.
/// These are textures, buffers, and globals but not models. Definitely not models.
#[derive(Debug)]
pub struct GraphLocals {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	
	resources_bgs: Vec<wgpu::BindGroup>,
	resource_bg_index_of_format: HashMap<BindGroupFormat, usize>,

	textures: Vec<BoundTexture>,
	textures_index_of_id: HashMap<String, usize>,
	
	buffers: Vec<wgpu::Buffer>,
	buffers_index_of_id: HashMap<String, usize>,

	pub default_resolution: [u32; 2],
}
impl GraphLocals {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(), 
			resources_bgs: Vec::new(),
			resource_bg_index_of_format: HashMap::new(),
			textures: Vec::new(),
			textures_index_of_id: HashMap::new(),
			buffers: Vec::new(),
			buffers_index_of_id: HashMap::new(),
			default_resolution: [0, 0],
		}
	}

	pub fn get_index_of_id(&self, id: &String, resource_type: GraphResourceType) -> Option<usize> {
		let holder = match resource_type {
			GraphResourceType::Texture => &self.textures_index_of_id,
			GraphResourceType::Buffer => &self.buffers_index_of_id,
			_ => panic!(),
		};
		if holder.contains_key(id) {
			Some(holder[id])
		} else {
			None
		}
	}

	pub fn get_texture(&self, i: usize) -> &BoundTexture {
		&self.textures[i]
	}

	pub fn insert_texture(&mut self, t: BoundTexture, id: &String) -> usize {
		debug!("Inserting texture '{}' as '{}'", &t.name, id);
		let idx = self.textures.len();
		self.textures_index_of_id.insert(id.clone(), idx);
		self.textures.push(t);
		idx
	}

	pub fn get_buffer(&self, i: usize) -> &wgpu::Buffer {
		&self.buffers[i]
	}

	pub fn insert_buffer(&mut self, b: wgpu::Buffer, id: &String) -> usize {
		let idx = self.buffers.len();
		self.buffers_index_of_id.insert(id.clone(), idx);
		self.buffers.push(b);
		idx
	}

	/// Creates a bind group containing all of the specified resources.
	/// May fail if a requested resource does not exist.
	pub fn create_resources_group(
		&mut self, 
		format: &BindGroupFormat, 
		resources: &mut RenderResources,
	) -> Result<usize> {
		debug!("Creating resources bg for '{}'", format);

		// The default sampler, used for every requested sampler
		// Todo: make this customizable
		let sampler_thing = self.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::Repeat,
			address_mode_v: wgpu::AddressMode::Repeat,
			address_mode_w: wgpu::AddressMode::Repeat,
			..Default::default()
		});

		let mut bindings = Vec::new();
		for (&i, entry_format) in &format.entry_formats {
			let resource_id = &entry_format.resource_usage;
			match entry_format.binding_type {
				BindingType::Buffer => {
					if self.buffers_index_of_id.contains_key(resource_id) {
						let idx = self.buffers_index_of_id[resource_id];
						let buffer = &self.buffers[idx];
						bindings.push(wgpu::BindGroupEntry {
							binding: i,
							resource: buffer.as_entire_binding(),
						});
					} else {
						error!("No buffer found for resource id '{}'", resource_id);
						panic!("Tried to retreive nonexistent resource buffer")
					}
				},
				BindingType::Texture => {
					if self.textures_index_of_id.contains_key(resource_id) {
						let idx = self.textures_index_of_id[resource_id];
						let texture = &self.textures[idx];
						bindings.push(wgpu::BindGroupEntry {
							binding: i,
							resource: wgpu::BindingResource::TextureView(&texture.view),
						});
					} else {
						error!("No texture found for resource id '{}'", resource_id);
						panic!("Tried to retreive nonexistent resource buffer")
					}
				}
				BindingType::Sampler => {
					bindings.push(wgpu::BindGroupEntry {
						binding: i,
						resource: wgpu::BindingResource::Sampler(&sampler_thing),
					});
				},
				_ => todo!("Resource group binding type is not yet implemented"),
			}
		}

		let layout_idx = match resources.shaders.bind_group_layout_index_from_bind_group_format(format) {
			Some(idx) => idx,
			None => resources.shaders.bind_group_layout_create(format),
		};
		let layout = resources.shaders.bind_group_layout_index(layout_idx);

		let bind_group = resources.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &bindings[..],
			layout,
			label: Some(&*format!("resources group with format '{}'", format)),
		});

		let idx = self.resources_bgs.len();
		self.resource_bg_index_of_format.insert(format.clone(), idx);
		self.resources_bgs.push(bind_group);
		Ok(idx)
	}

	pub fn resource_bg(&self, i: usize) -> &wgpu::BindGroup {
		&self.resources_bgs[i]
	}

	/// Find index of resource bind group if it exists
	pub fn resource_bg_of_format(&self, bgf: &BindGroupFormat) -> Option<usize> {
		if self.resource_bg_index_of_format.contains_key(bgf) {
			Some(self.resource_bg_index_of_format[bgf])
		} else {
			None
		}
	}

	/// Find index of resource bind group, create it if it doesn't exist
	pub fn resource_bg_of_format_create(&mut self, bgf: &BindGroupFormat, resources: &mut RenderResources) -> Result<usize> {
		if self.resource_bg_index_of_format.contains_key(bgf) {
			Ok(self.resource_bg_index_of_format[bgf])
		} else {
			Ok(self.create_resources_group(bgf, resources)?)
		}
	}
}



/// A node which contains more nodes
#[derive(Debug)]
pub struct GraphNode {
	name: String,
	nodes: Vec<Box<dyn RunnableNode>>,
	order: Vec<usize>,
	
	inputs: HashSet<(String, GraphResourceType)>,
	outputs: HashSet<(String, GraphResourceType)>,

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
	pub fn calculate_io(&self) -> [HashSet<(String, GraphResourceType)>; 2] {

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
	
	fn inputs(&self) -> &HashSet<(String, GraphResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, GraphResourceType)> {
		&self.outputs
	}
	
	fn update(
		&mut self, 
		graph_resources: &mut GraphLocals, 
		model_resources: &mut ModelsQueueResource, 
		render_resources: &mut RenderResources,
	) {
		debug!("Updating graph {}", &self.name);
		for node in &mut self.nodes {
			node.update(graph_resources, model_resources, render_resources);
		}
	}
	
	fn run(
		&self, 
		context: &mut GraphLocals, 
		model_resources: &ModelsQueueResource,
		render_resources: &mut RenderResources, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running graph {}", &self.name);
		encoder.push_debug_group(&*format!("Graph node '{}'", &self.name));
		for i in &self.order {
			self.nodes[*i].run(context, model_resources, render_resources, encoder);
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
	shader_idx: usize,

	// The input formts for this node, if it needs them
	mesh_input_format: Option<MeshInputFormat>,
	material_input_bgf: Option<BindGroupFormat>,
	resources_idx: Option<usize>,

	// Index of texture, should store bool
	colour_attachments: Vec<(usize, bool)>,
	depth_attachment: Option<(usize, bool)>,
	// The queue index (if initialized) (if necessary)
	render_queue: Option<QueueType>,

	inputs: HashSet<(String, GraphResourceType)>,
	depth: Option<String>,
	aliases: HashMap<String, String>,
	outputs: HashSet<(String, GraphResourceType)>,

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
			inputs.insert((depth_id.clone(), GraphResourceType::Texture));
		}

		let shader_path = folder_context.join(&spec.shader).canonicalize()
			.with_context(|| format!("Failed to canonicalize shader path ('{:?}' + '{:?}')", &folder_context, &spec.shader))?;
			
		let shader_idx = match shaders.index_from_path(&shader_path) {
			Some(idx) => idx,
			None => shaders.register_path(&shader_path),
		};
		let shader = shaders.index(shader_idx);

		// Add globals input if needed
		if let Some(idx) = shader.resources_bg_index {
			let globals_bgf = ShaderNode::resources_alias_filter(&spec.aliases, shader.bind_groups[&idx].format().clone());
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

		Ok(Self {
			name: spec.name.clone(),
			shader_path: spec.shader.clone(),
			shader_idx,
			mesh_input_format,
			material_input_bgf,
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

	/// Filters a bind group to use this node's aliases
	fn resources_alias_filter(
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

	fn inputs(&self) -> &HashSet<(String, GraphResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, GraphResourceType)> {
		&self.outputs
	}
	
	fn update(
		&mut self, 
		graph_resources: &mut GraphLocals, 
		model_resources: &mut ModelsQueueResource, 
		render_resources: &mut RenderResources,
	) {	
		debug!("Updating shader node '{}'", &self.name);

		let shader = render_resources.shaders.index(self.shader_idx);
		let instance_properties = shader.instance_properties.clone();
		let vertex_properties = shader.vertex_properties.clone();
		let globals_bgf = shader.bind_groups[&0].format();
		let materials_bgf = match shader.material_bg_index {
			Some(idx) => Some(shader.bind_groups[&idx].format().clone()),
			None => None,
		};
		drop(shader);

		let filtered_globals_bgf = ShaderNode::resources_alias_filter(&self.aliases, globals_bgf);
		self.resources_idx = Some(
			graph_resources.resource_bg_of_format_create(&filtered_globals_bgf, render_resources).unwrap()
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

		let shader = render_resources.shaders.index(self.shader_idx);
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
			self.fugg_buff = Some(render_resources.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("fugg Buffer"),
				contents: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
				usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
			}));
		}
	}
	
	fn run(
		&self, 
		context: &mut GraphLocals, 
		model_resources: &ModelsQueueResource,
		resources: &mut RenderResources, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running shader node {}", &self.name);
		encoder.push_debug_group(&*format!("Shader node '{}'", &self.name));
		
		let shader = resources.shaders.index(self.shader_idx);

		let colour_attachments = self.colour_attachments.iter().cloned().map(|(idx, store)| {
			wgpu::RenderPassColorAttachment {
				view: &context.get_texture(idx).view,
				resolve_target: None, // Same as view unless using multisampling
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store,
				},
			}
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
					let material = resources.materials.index(*material_idx);
					render_pass.set_bind_group(1, &material.bind_group, &[]);
					
					// Mesh
					let mesh = resources.meshes.index(*mesh_idx);
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
					let mesh = resources.meshes.index(*mesh_idx);
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
	pub render_inputs: HashSet<(String, GraphResourceType)>,	// ("resource name", resource type)
	pub depth: Option<String>,
	// When assigning resources to the shader inputs we look to see if the input name exists as an alias
	// If no match is found we look through inputs as a fallback, avoiding manadtory overspecification
	pub aliases: HashMap<String, String>,	// ("resource name", "shader input usage")
	pub outputs: HashSet<(String, GraphResourceType)>,
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

	inputs: HashSet<(String, GraphResourceType)>,
	outputs: HashSet<(String, GraphResourceType)>,
}
impl TextureNode {
	pub fn from_spec(spec: &TextureNodeSpecification) -> Self {
		Self {
			name: spec.name.clone(),
			resource_id: spec.resource_name.clone(),
			texture_format: spec.texture_format,
			resolution: spec.resolution,
			fill_with: spec.fill_with.clone(),
			inputs: HashSet::new(),
			outputs: [(spec.resource_name.clone(), GraphResourceType::Texture)].iter().cloned().collect::<HashSet<_>>(),
			texture_idx: 0,
		}
	}

	fn create_texture(
		&self,
		context: &mut GraphLocals, 
		render_resources: &mut RenderResources, 
	) -> usize {
		let [width, height] = match self.resolution {
			Some(r) => r,
			None => context.default_resolution,
		};
		let t = BoundTexture::new_with_format(
			&render_resources.device,
			&self.resource_id,
			self.texture_format.translate(),
			width,
			height,
		);
		context.insert_texture(t, &self.resource_id)
	}
}
impl RunnableNode for TextureNode {
	fn name(&self) -> &String {
		&self.name
	}
	
	fn inputs(&self) -> &HashSet<(String, GraphResourceType)> {
		&self.inputs
	}
	
	fn outputs(&self) -> &HashSet<(String, GraphResourceType)> {
		&self.outputs
	}

	fn update(&mut self, context: &mut GraphLocals, _: &mut ModelsQueueResource, render_resources: &mut RenderResources) {
		// Should create if not exists but I'm lazy
		self.texture_idx = self.create_texture(context, render_resources);
	}

	fn run(
		&self, 
		context: &mut GraphLocals, 
		_: &ModelsQueueResource,
		_: &mut RenderResources, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		debug!("Running texture node {}", &self.name);
		encoder.push_debug_group(&*format!("Texture node '{}'", &self.name));

		// Fill with stuff if needed
		if let Some(fill_with) = &self.fill_with {
			let texxy = context.get_texture(self.texture_idx);
			let colour_attachments = match self.texture_format.is_depth() {
				true => vec![],
				false => {vec![
					wgpu::RenderPassColorAttachment {
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
					},
				]},
			};
	
			let depth_stencil_attachment = match self.texture_format.is_depth() {
				true => {
					Some(wgpu::RenderPassDepthStencilAttachment {
						view: &texxy.view,
						depth_ops: Some(wgpu::Operations {
							load: wgpu::LoadOp::Clear(fill_with[0]),
							store: true,
						}),
						stencil_ops: None,
					})
				},
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
