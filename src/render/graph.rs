use serde::{Serialize, Deserialize};
use std::{collections::{HashMap, HashSet}, time::Instant, sync::Arc};
use crate::render::*;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;


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
	// Pull requisite data for run()
	fn update(&mut self, graph_resources: &GraphLocals, model_resources: &ModelsResource, render_resources: &mut RenderResources);
	// Mutate context and encode rendering
	fn run(&self, graph_resources: &mut GraphLocals, model_resources: &ModelsResource, render_resources: &mut RenderResources, encoder: &mut wgpu::CommandEncoder);
}
impl std::fmt::Debug for dyn RunnableNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RunnableNode {}", self.name())
    }
}



#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum GraphResourceType {
	Resources(BindGroupFormat),
	Models(ShaderInput),
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
		let idx = self.textures.len();
		self.textures_index_of_id.insert(id.clone(), idx);
		self.textures.push(t);
		idx
	}

	/// Leaves the thing broken, please fix
	pub fn remove_texture(&mut self, id: &String) -> BoundTexture {
		let idx = self.textures_index_of_id.remove(id).unwrap();
		self.textures.remove(idx)
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

	pub fn create_resources_group(
		&mut self, 
		format: &BindGroupFormat, 
		resources: &mut RenderResources,
	) -> usize {
		info!("Creating resources bg for '{}'", format);

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
						panic!("Tried to reterive nonexistent resource buffer")
					}
				},
				_ => todo!(),
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
			label: Some(&*format!("resources group '{}'", format)),
		});

		let idx = self.resources_bgs.len();
		self.resource_bg_index_of_format.insert(format.clone(), idx);
		self.resources_bgs.push(bind_group);
		idx
	}

	pub fn resource_bg(&self, i: usize) -> &wgpu::BindGroup {
		&self.resources_bgs[i]
	}

	pub fn resource_bg_of_format(&self, bgf: &BindGroupFormat) -> Option<usize> {
		if self.resource_bg_index_of_format.contains_key(bgf) {
			Some(self.resource_bg_index_of_format[bgf])
		} else {
			None
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
		graph_resources: &GraphLocals, 
		model_resources: &ModelsResource, 
		render_resources: &mut RenderResources,
	) {
		for node in &mut self.nodes {
			node.update(graph_resources, model_resources, render_resources);
		}
	}
	
	fn run(
		&self, 
		context: &mut GraphLocals, 
		model_resources: &ModelsResource,
		render_resources: &mut RenderResources, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		info!("Running graph {}", &self.name);
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
}



#[derive(Debug, Serialize, Deserialize)]
enum NodeSpecificationType {
	// Graph(GraphNodeSpecification),
	Shader(ShaderNodeSpecification),
}



/// A node which runs a shader
#[derive(Debug)]
struct ShaderNode {
	name: String,
	shader_path: PathBuf,
	shader_idx: usize,
	globals_idx: Option<usize>,
	models_queue: Option<usize>,
	inputs: HashSet<(String, GraphResourceType)>,
	aliases: HashMap<String, String>,	// Flipped from spec
	outputs: HashSet<(String, GraphResourceType)>,
}
impl ShaderNode {
	pub fn from_spec(spec: &ShaderNodeSpecification, folder_context: &Path, shaders: &mut ShaderManager) -> Self {
		let aliases = spec.aliases.iter().map(|(a, b)| (b.clone(), a.clone())).collect::<HashMap<_,_>>();

		let mut inputs = spec.inputs.clone();
		// Add model inputs and globals format

		let shader_path = folder_context.join(&spec.shader).canonicalize().unwrap();
		let shader_idx = match shaders.index_from_path(&shader_path) {
			Some(idx) => idx,
			None => shaders.register_path(&shader_path),
		};
		let shader = shaders.index(shader_idx);

		// Add globals input
		let globals_bgf = ShaderNode::resources_alias_filter(&aliases, shader.bind_groups[&0].format().clone());
		inputs.insert(("_globals".to_string(), GraphResourceType::Resources(globals_bgf)));
		
		// Add shader input
		inputs.insert(("_materials".to_string(), GraphResourceType::Models((
			shader.instance_properties.clone(), 
			shader.vertex_properties.clone(), 
			shader.bind_groups[&1].format(),
		))));

		Self {
			name: spec.name.clone(),
			shader_path: spec.shader.clone(),
			shader_idx,
			globals_idx: None,
			models_queue: None,
			inputs,
			aliases,
			outputs: spec.outputs.clone(),
		}
	}

	fn alias_for(&self, s: &String) -> Option<&String> {
		if self.aliases.contains_key(s) {
			Some(&self.aliases[s])
		} else {
			None
		}
	}

	fn resources_alias_filter(
		aliases: &HashMap<String, String>, 
		mut bgf: BindGroupFormat,
	) -> BindGroupFormat {
		for (_, bgef) in &mut bgf.entry_formats {
			if aliases.contains_key(&bgef.resource_usage) {
				let alias = aliases[&bgef.resource_usage].clone();
				info!("Found alias '{}' -> '{}'", &bgef.resource_usage, &alias);
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
		graph_resources: &GraphLocals, 
		model_resources: &ModelsResource, 
		render_resources: &mut RenderResources,
	) {	
		let shader = render_resources.shaders.index(self.shader_idx);

		let g = ShaderNode::resources_alias_filter(&self.aliases, shader.bind_groups[&0].format().clone());
		self.globals_idx = Some(
			graph_resources.resource_bg_of_format(&g).unwrap()
		);
		info!("Shader node {} chose globals idx {}", &self.name, &self.globals_idx.unwrap());

		self.models_queue = Some(
			model_resources.queue_index_of_format(
				&(shader.instance_properties.clone(), shader.vertex_properties.clone(), shader.bind_groups[&1].format())
			).unwrap()
		);
		info!("Shader node {} chose model queue idx {}", &self.name, &self.models_queue.unwrap());
	}
	
	fn run(
		&self, 
		context: &mut GraphLocals, 
		model_resources: &ModelsResource,
		resources: &mut RenderResources, 
		encoder: &mut wgpu::CommandEncoder,
	) {
		info!("Running shader node {}", &self.name);
		encoder.push_debug_group(&*format!("Shader node '{}'", &self.name));
		
		let shader = resources.shaders.index(self.shader_idx);

		let colour_attachments = shader.attachments.iter().map(|attachment| {
			let resource_name = {
				match self.alias_for(&attachment.usage) {
					Some(alias) => alias.clone(),
					None => attachment.usage.clone(),
				}
			};
			info!("Attaching attachment {} ({})", &resource_name, &attachment.usage);
			let attatchment_key = (resource_name.clone(), GraphResourceType::Texture);
			if self.inputs.contains(&attatchment_key) {
				let store = self.outputs.contains(&attatchment_key);

				let attatchment_texture = context.get_texture(context.get_index_of_id(&attachment.usage, GraphResourceType::Texture).expect("Attatchment not found!"));

				wgpu::RenderPassColorAttachment {
					view: &attatchment_texture.view,
					resolve_target: None, // Same as view unless using multisampling
					ops: wgpu::Operations {
						load: {
							// Haha jonathan
							if true {
								wgpu::LoadOp::Clear(wgpu::Color {
									r: 0.1,
									g: 0.2,
									b: 0.3,
									a: 1.0,
								})
							} else {
								wgpu::LoadOp::Load
							}
						},
						store,
					},
				}
			} else {
				panic!("Shader requires attachment not found in node inputs!")
			}
		}).collect::<Vec<_>>();

		let depth_stencil_attachment = {
			let depth_key = ("_depth".to_string(), GraphResourceType::Texture);
			let store_depth = self.outputs.contains(&depth_key);
			let clear_depth = store_depth && !self.inputs.contains(&depth_key);
			if self.inputs.contains(&depth_key) {
				let dt = context.get_texture(context.get_index_of_id(&depth_key.0, GraphResourceType::Texture).expect("Depth attatchment not found!?"));
				Some(wgpu::RenderPassDepthStencilAttachment {
					view: &dt.view,
					depth_ops: Some(wgpu::Operations {
						load: {
							if true {
								wgpu::LoadOp::Clear(1.0)
							} else {
								wgpu::LoadOp::Load
							}
						},
						store: store_depth,
					}),
					stencil_ops: None,
				})
			} else {
				None
			}
		};

		let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some(&*self.name),
			color_attachments: &colour_attachments[..],
			depth_stencil_attachment,
		});

		render_pass.set_pipeline(&shader.pipeline);

		// Globals
		let globals_bind_group = context.resource_bg(self.globals_idx.unwrap());
		render_pass.set_bind_group(0, globals_bind_group, &[]);

		let (instances_idx, models) = model_resources.queue(self.models_queue.unwrap());

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

		drop(render_pass);
		encoder.pop_debug_group();
	}
}



#[derive(Debug, Serialize, Deserialize)]
enum KLoadOp {
	Load,
	Clear(Vec<f32>),
}



/// Serializable shader node info
#[derive(Debug, Serialize, Deserialize)]
struct ShaderNodeSpecification {
	pub name: String,
	pub shader: PathBuf,
	pub inputs: HashSet<(String, GraphResourceType)>,	// ("resource name", resource type)
	// When assigning resources to the shader inputs we look to see if the input name exists as an alias
	// If no match is found we look through inputs as a fallback, avoiding manadtory overspecification
	pub aliases: HashMap<String, String>,	// ("resource name", "shader input usage")
	pub outputs: HashSet<(String, GraphResourceType)>,
}




// /// A node for clearing textures!
// #[derive(Debug)]
// pub struct ClearTextureNode {
// 	name: String,
// 	values: Vec<f32>,
	
// 	inputs: HashSet<(String, GraphResourceType)>,
// 	outputs: HashSet<(String, GraphResourceType)>,
// 	to_create: HashSet<(String, GraphResourceType)>,
// }
// impl ClearTextureNode {
// 	pub fn from_spec(spec: ClearTextureNode) -> Self {
// 		let to_create = spec.outputs.difference(&spec.inputs).collect::<Vec<_>>();
// 		todo!()
// 	}
// }
// impl RunnableNode for ClearTextureNode {
// 	fn name(&self) -> &String {
// 		&self.name
// 	}
// 	fn inputs(&self) -> &HashSet<(String, GraphResourceType)> {
// 		&self.inputs
// 	}
// 	fn outputs(&self) -> &HashSet<(String, GraphResourceType)> {
// 		&self.outputs
// 	}
// 	fn run(&self, context: &mut ResourceContext, encoder: &mut wgpu::CommandEncoder) {
// 		println!("Running clear {}", &self.name);
// 		for (n, t) in &self.to_create {
// 			println!("Creating resource {} ({:?})", n, t);
// 			// Todo: this
// 		}
// 		for (n, t) in &self.outputs {
// 			println!("Clearing texture {} ({:?})", n, t);
// 			match t {
// 				GraphResourceType::Texture => {
// 					let tidx = context.get_index(n, GraphResourceType::Texture).unwrap();
// 					let tex = context.get_texture(tidx);
// 					let int_values = self.values.iter().map(|v| *v as u8).collect::<Vec<_>>();
// 					let n = 4 * tex.size.height * tex.size.width / (int_values.len() as u32);
// 					let data = int_values.repeat(n as usize);
// 					tex.fill(&data[..], &context.resources.queue);
// 				}
// 				_ => panic!("Hey that's not a texture!"),
// 			}
// 		}
// 	}
// }



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
						graph.add_node(Box::new(ShaderNode::from_spec(spec, context, shaders)));
					}
				},
				_ => panic!("fugg"),
			};
		}
	}

	graph
}



#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_thing() {
		//read_graph_file(&PathBuf::from("resources/graphs/kdefault.ron"));

		let g = GraphNodeSpecification {
			name: "fug".into(),
			order: vec![],
		};
		let ns = vec![
			NodeSpecificationType::Shader(ShaderNodeSpecification {
				name: "fug".into(),
				shader: "g".into(),
				inputs: HashSet::from([
					("g".into(), GraphResourceType::Texture),
				]),
				aliases: HashMap::new(),
				outputs: HashSet::new(),
			})
		];
		let gns = (g, ns);
		
		let pretty = ron::ser::PrettyConfig::new().depth_limit(3).separate_tuple_members(true).enumerate_arrays(false);
		let s = ron::ser::to_string_pretty(&gns, pretty).expect("Serialization failed");
		println!("{}", &s);

		let f: (GraphNodeSpecification, Vec<NodeSpecificationType>) = ron::de::from_str(&s)
		.expect("Failed to read graph ron file");
		println!("{:?}", &f);
		
		assert!(true);
	}
}
