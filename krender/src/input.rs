use prelude::{AbstractRenderTarget, RenderContext};
use rendertarget::RRID;
use slotmap::{SlotMap, SecondaryMap};
use crate::{*, bundle::{RenderBundleStage, MaybeOwned, RenderBundle}};



// Maybe use in future? For now I don't care about this enough
// pub struct TargetItems<T> {
// 	individuals: Vec<(MaterialKey, Option<MeshKey>, T)>,
// 	batches: SlotMap<RenderBatchKey, (MaterialKey, Option<MeshKey>, Buffer, u32, T)>,
// 	// indirects: SlotMap<RenderBatchKey, (MaterialKey, Option<MeshKey>, Buffer, u32, T)>,
// 	// indexed_indirects: SlotMap<RenderBatchKey, (MaterialKey, Option<MeshKey>, Buffer, u32, T)>,
// }


#[derive(Debug, Default)]
pub struct RenderInputStage<T> {
	// Target -> items
	target_items: HashMap<AbstractRenderTarget, Vec<(MaterialKey, Option<MeshKey>, T)>>,
	computes: SlotMap<RenderComputeKey, (MaterialKey, [u32; 3])>,

	// Move to vecs? Faster insertion
	colour_clears: HashMap<RRID, wgpu::Color>, 
	depth_clears: HashMap<RRID, f32>, 
}
impl<T: EntityIdentifier> RenderInputStage<T> {
	pub fn new() -> Self {
		Self {
			target_items: HashMap::new(),
			computes: SlotMap::with_key(),
			colour_clears: HashMap::new(),
			depth_clears: HashMap::new(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.target_items.values().all(|v| v.is_empty())
			&& self.computes.is_empty()
			&& self.colour_clears.is_empty()
			&& self.depth_clears.is_empty()
	}

	pub fn clear_depth(&mut self, id: RRID) -> &mut Self {
		if let Some(_) = self.depth_clears.insert(id.into(), 1.0) {
			warn!("Depth clear already set");
		}
		self
	}

	pub fn clear_colour(&mut self, id: RRID) -> &mut Self {
		if let Some(_) = self.colour_clears.insert(id.into(), wgpu::Color::RED) {
			warn!("Colour clear already set");
		}
		self
	}

	pub fn target(&mut self, target: AbstractRenderTarget) -> &mut Vec<(MaterialKey, Option<MeshKey>, T)> {
		self.target_items.entry(target.into()).or_default()
	}

	// This is good, but it considers each stage seperately. 
	// Good for parallelism, not good for more than that. 
	#[profiling::function]
	pub(crate) fn bundle<'a>(
		&self,
		device: &wgpu::Device,
		textures: &TextureManager,
		meshes: &mut MeshManager,
		materials: &MaterialManager,
		shaders: &ShaderManager,
		storage_provider: &'a impl InstanceDataProvider<'a, T>,
		context: &RenderContext<T>,
		sort: bool,
	) -> RenderBundleStage {

		// Map clears to keys
		let mut colour_clears = self.colour_clears.iter()
			.map(|(id, c)| (
				id.texture(context, textures).expect("clear (depth) texture not found"), 
				*c
			))
			.collect::<HashMap<_,_>>();
		let mut depth_clears = self.depth_clears.iter()
			.map(|(id, c)| (
				id.texture(context, textures).expect("clear (colour) texture not found"), 
				*c
			))
			.collect::<HashMap<_,_>>();

		// Targeted rendering
		let mut targets = Vec::with_capacity(self.target_items.len());
		for (abstract_target, items) in self.target_items.iter() {
			profiling::scope!("target");
			trace!("Bundle target {:?}", abstract_target);
			// Look up target id in context
			// If multiple abstract things map to the same texture then we don't care to merge them
			let mut target = abstract_target.specify(context, textures);

			// Look for any texture clears that could just be load operations
			for (t, _, ops) in target.colour_attachments.iter_mut() {
				if let Some(colour) = colour_clears.remove(&t) {
					trace!("Colour attachment {t:?} can be cleared using loadops");
					ops.load = wgpu::LoadOp::Clear(colour);
				}
			}
			if let Some((d, ops)) = target.depth_attachment.as_mut() {
				if let Some(f) = depth_clears.remove(&d) {
					trace!("Depth attachment {d:?} can be cleared using loadops");
					ops.load = wgpu::LoadOp::Clear(f);
				}
			}

			// Extract shader and bind groups from material
			let mut mapped_items = {
				trace!("Mapping render items");
				profiling::scope!("map");
				items.iter()
					.map(|&(mtl, mesh, e)| {
						// (shader, bgs, mesh, entity)
						let entry = materials.get(mtl)
							.expect(&*format!("Material key {mtl:?} is not valid!"));
						let shader = entry.shader_key
							.expect(&*format!("Material '{}' has no shader key!", entry.specification.name));
						let binding = context.material_bindings.get(mtl)
							.expect(&*format!("Material '{}' is not bound for this context!", entry.specification.name));
						(shader, binding.bind_groups, mesh, e)
					})
					.collect::<Vec<_>>()
			};

			// Sort by shader and then bind group and then mesh
			if sort {
				profiling::scope!("sort");
				mapped_items.sort_unstable_by_key(|&(shader, bgs, mesh, _)| (shader, bgs, mesh));
			}

			// Find shader partitions
			let shader_partitions = {
				profiling::scope!("partition");
				let mut partitions = Vec::new();
				// You could turn this into a fold
				if mapped_items.len() != 0 {
					let mut last_shader = mapped_items[0].0;
					let mut last_idx = 0;
					for (i, &(s, _, _, _)) in mapped_items.iter().enumerate() {
						if s != last_shader {
							partitions.push(last_idx..i);
							last_shader = s;
							last_idx = i;
						}
					}
					partitions.push(last_idx..mapped_items.len());
				}
				partitions
			};

			// Iterate over all shaders and then over partition contents
			let mut target_shaders = Vec::with_capacity(shader_partitions.len());
			for r in shader_partitions.iter() {
				profiling::scope!("shader");
				let items = &mapped_items[r.clone()];

				let shader_key = mapped_items[r.start].0;
				let shader = shaders.get(shader_key).unwrap();
				
				// Borrow ECS storages
				let attributes = &shader.specification.base.polygonal().instance_attributes;
				let storages = attributes.iter()
					.map(|a| (a, storage_provider.fetch_source(&a.source)))
					.collect::<Vec<_>>();

				// Collect storage data
				let instance_data = {
					profiling::scope!("fetch data");

					// This code is followed by a commented-out version of itself
					// For some reason, this approach is dramatically faster
					let attributes_len = attributes.iter().fold(0, |a, v| v.size() as usize + a);
					let mut buffer_data = Vec::with_capacity(r.clone().count() * attributes_len);
					for &(_, _, _, e) in &mapped_items[r.clone()] {
						for (attribute, storage) in storages.iter() {
							let d = match storage.as_ref().unwrap() {
								FetchedInstanceAttributeSource::Component(storage) => storage.get_component(e),
								FetchedInstanceAttributeSource::Resource(r) => Some(*r)
							};
							let data = if storage.is_some() && d.is_some() {
								d.unwrap()
							} else if let Some(d) = attribute.default.as_ref() {
								d.as_slice()
							} else {
								panic!("Error pulling data for {:?}, no entity data and no default!", attribute);
							};
							buffer_data.extend_from_slice(data);
						}
					}
					buffer_data

					// (&mapped_items[r.clone()]).iter()
					// 	.flat_map(|(_, _, _, e)| storages.iter().flat_map(move |(attribute, fetched)| {
					// 		if fetched.is_some() && let Some(s) = match fetched.as_ref().unwrap() {
					// 			FetchedInstanceAttributeSource::Component(storage) => storage.get_component(e),
					// 			FetchedInstanceAttributeSource::Resource(r) => Some(*r)
					// 		} {
					// 			s
					// 		} else if let Some(d) = attribute.default.as_ref() {
					// 			d.as_slice()
					// 		} else {
					// 			panic!("Error pulling data for {:?}, no entity data and no default!", attribute);
					// 		}
					// 	}).copied())
					// 	.collect::<Vec<_>>()
				};
				let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: None,
					contents: instance_data.as_slice(),
					usage: wgpu::BufferUsages::VERTEX,
				});

				// Collect draw calls
				let mut draws = Vec::with_capacity(items.len()); // Less than or equal
				let (_, mut current_binding_config, mut current_mesh, _) = items[0];
				let mut count: u32 = 0;
				{
					profiling::scope!("collect");
					for &(_, binding_config, mesh, _) in items {
						if current_binding_config != binding_config || current_mesh != mesh {
							draws.push((current_binding_config, current_mesh, count));
							current_binding_config = binding_config;
							current_mesh = mesh;
							count = 0;
						}
						count += 1;
					}
					draws.push((current_binding_config, current_mesh, count));
				}

				// Mark meshes for binding
				// Todo: Skip drawing meshes that are not bound
				{
					profiling::scope!("mesh");
					if let Some(format_key) = shader.mesh_format_key {
						// Will this exist?
						// Not it will not!
						if !meshes.vertex_bindings.contains_key(format_key) {
							meshes.vertex_bindings.insert(format_key, SecondaryMap::new());
						}
						let vertex_bindings = meshes.vertex_bindings.get_mut(format_key).unwrap();
						
						for mesh_key in draws.iter().filter_map(|&(_, m, _)| m) {
							if !meshes.index_bindings.contains_key(mesh_key) {
								trace!("Mark mesh {:?} for vertex binding ({:?})", mesh_key, format_key);
								meshes.index_bindings.insert(mesh_key, None);
							}
							if !vertex_bindings.contains_key(mesh_key) {
								trace!("Mark mesh {:?} for index binding", mesh_key);
								vertex_bindings.insert(mesh_key, None);
							}
						}
					}
				}

				// We use a vec in case we would want to include other items here
				// Examples: pre-batched items, indirect items
				target_shaders.push((shader_key, vec![(MaybeOwned::Owned(instance_buffer), draws)]));
			}

			targets.push((target, target_shaders));
		}

		warn!("Todo: Compute shaders");

		RenderBundleStage {
			targets, 
			computes: Vec::new(),
			attachment_clears: HashMap::new(),
			depth_clears: HashMap::new(),
		}
	}

	// pub fn computes(&mut self) -> &mut SlotMap<RenderComputeKey, (MaterialKey, [u32; 3])>
}


#[derive(Debug)]
pub struct RenderInput<T> {
	stages: BTreeMap<String, RenderInputStage<T>>,
	dependencies: Vec<(String, String)>,
}
impl<E: EntityIdentifier> RenderInput<E> {
	pub fn new() -> Self {
		Self {
			stages: BTreeMap::new(),
			dependencies: Vec::new(),
		}
	}

	pub fn stage(&mut self, stage: impl Into<String>) -> &mut RenderInputStage<E> {
		self.stages.entry(stage.into())
			.or_insert_with(|| RenderInputStage::new())
	}

	pub fn add_dependency(&mut self, dependent: impl Into<String>, dependency: impl Into<String>) {
        self.dependencies.push((dependent.into(), dependency.into()));
    }

	// Flat execution order.
	// Could easily provide groups but we don't need that and it just means more heap allocation.
	fn stage_order<'a>(&'a self) -> Vec<&'a String> {
		let mut queue = self.stages.iter()
			.filter(|(_, i)| !i.is_empty())
			.map(|(s, _)| (s, self.dependencies.iter().filter_map(|(dependent, dependency)| dependent.eq(s).then(|| dependency)).collect::<Vec<_>>()))
			.collect::<Vec<_>>();

		let mut order: Vec<&String> = Vec::with_capacity(queue.len());
		while !queue.is_empty() {
			let items = queue.iter()
				.filter_map(|(stage, dependencies)| 
					dependencies.iter().all(|&d| order.contains(&d)).then(|| *stage)
				)
				.collect::<Vec<_>>();
			if items.len() == 0 {
				error!("order: {order:#?}");
				error!("queue: {queue:#?}");
				panic!("Failed to create render order");
			}

			queue.retain(|(name, _)| !items.contains(name));
			order.extend(items.iter());
		}

		if !queue.is_empty() {
			panic!("Stages order error!");
		}

		order
	}

	#[profiling::function]
	pub fn bundle<'a>(
		&'a self,
		device: &wgpu::Device,
		textures: &TextureManager,
		meshes: &mut MeshManager,
		materials: &MaterialManager,
		shaders: &ShaderManager,
		storage_provider: &'a impl InstanceDataProvider<'a, E>,
		context: &RenderContext<E>,
		sort: bool,
	) -> RenderBundle<'a> {
		let mut bundle = RenderBundle::default();

		let stages = self.stage_order();
		trace!("Stages: {:?}", stages);
		
		for stage in stages {
			profiling::scope!("stage");
			trace!("Bundle stage {}", stage);
			let input = self.stages.get(stage).unwrap();

			let bundle_stage = input.bundle(
				device,
				textures,
				meshes,
				materials,
				shaders,
				storage_provider,
				context,
				sort
			);
			
			bundle.stages.push(bundle_stage);
		}

		bundle
	}
}
