

#[derive(Debug)]
struct RenderInputStage<T: EntityID> {
	pub name: String,
	pub polygons: Vec<(MaterialKey, Option<MeshKey>, T)>,
	pub computes: Vec<(MaterialKey, [u32; 3])>,
	pub clears: HashMap<TextureKey, wgpu::ImageSubresourceRange>, 
}
impl<T: EntityID> RenderInputStage<T> {
	pub fn new(name: String) -> Self {
		Self {
			name,
			polygons: Vec::new(),
			computes: Vec::new(),
			clears: HashMap::new(),
		}
	}
}



/// A render input consists of a sequence of stages.
/// Each stage contains a set of texture and compute targets.
/// 
/// When executing the render input, we run through each stage in order.
/// For each target in the stage, we do a render pass.
#[derive(Debug)]
pub struct RenderInput<T: EntityID> {
	// Replace this with a vec please
	stages: BTreeMap<String, RenderInputStage<T>>,
	dependencies: Vec<(String, String)>,
}
impl<T: EntityID> RenderInput<T> {
	pub fn new() -> Self {
		Self {
			stages: BTreeMap::new(),
			dependencies: Vec::new(),
		}
	}

	pub fn insert_items(
		&mut self, 
		stage: impl Into<String>, 
		items: impl Iterator<Item = (MaterialKey, Option<MeshKey>, T)>,
	) {
		let stage = stage.into();

		let stage = self.stages.entry(stage.clone()).or_insert(RenderInputStage::new(stage.clone()));
		stage.polygons.extend(items);
	}

	pub fn add_dependency(&mut self, dependent: impl Into<String>, dependency: impl Into<String>) {
        self.dependencies.push((dependent.into(), dependency.into()));
    }

	// Flat execution order.
	// Could easily provide groups but we don't need that and it just means more heap allocation.
	fn stage_order<'a>(&'a self) -> Vec<&'a String> {
		let mut queue = self.stages.keys()
			.map(|s| (s, self.dependencies.iter().filter_map(|(dependent, dependency)| dependent.eq(s).then(|| dependency)).collect::<Vec<_>>()))
			.collect::<Vec<_>>();

		let mut order: Vec<&String> = Vec::with_capacity(queue.len());
		while !queue.is_empty() {
			let items = queue.iter()
				.filter_map(|(stage, dependencies)| 
					dependencies.iter().all(|&d| order.contains(&d)).then(|| *stage)
				)
				.collect::<Vec<_>>();

			queue.retain(|(name, _)| !items.contains(name));
			order.extend(items.iter());
		}

		if !queue.is_empty() {
			panic!("Stages order error!");
		}

		order
	}

	pub fn finish(
		&mut self,
		device: &wgpu::Device,
		materials: &MaterialManager,
		shaders: &ShaderManager,
		// bind_groups: &BindGroupManager,
		// meshes: &MeshManager,
		// textures: &TextureManager,
		// buffers: &BufferManager,
		storage_provider: &impl InstanceDataProvider<T>,
		context: RenderContextKey,
	) -> RenderBundle {
		let mut bundle = RenderBundle::default();

		for stage in self.stage_order() {
			let input = self.stages.get(stage).unwrap();

			let mut bundle_stage = RenderBundleStage::default();

			// Collect by target
			// Collect by shader
			let mut targets = HashMap::new();
			for &(material_key, mesh, entity_id) in input.polygons.iter() {
				let material = materials.key(material_key).unwrap();
				let shader_key = material.shader_key.unwrap();

				let (target, binding_config) = material.bindings.get(context).unwrap().polygon_stuff();

				let shader_groups  = targets.entry(target).or_insert_with(|| HashMap::new());
				let data = shader_groups.entry(shader_key).or_insert_with(|| Vec::new());

				data.push((binding_config, mesh, entity_id));
			}

			// Sort
			// Fetch instance data
			for (&target, shader_groups) in targets.iter_mut() {

				let mut bundle_target_entries = HashMap::new();

				for (&shader_key, items) in shader_groups.iter_mut() {
					items.sort_unstable_by_key(|e| (e.0, e.1));

					let shader = shaders.get(shader_key).unwrap();
					let instance_attributes = &shader.specification.base.polygonal().instance_attributes;
					let storages = instance_attributes.iter().map(|a| (a, storage_provider.fetch_source(&a.source))).collect::<Vec<_>>();

					let mut instance_items = Vec::new();
					let mut instance_data = Vec::new();
					let (mut current_binding_config, mut current_mesh, _) = items[0];
					let mut count: u32 = 0;
					for &(binding_config, mesh, entity_id) in items.iter() {
						if current_binding_config != binding_config || current_mesh != mesh {
							instance_items.push((current_binding_config, current_mesh, count));
							current_binding_config = binding_config;
							current_mesh = mesh;
							count = 0;
						}

						for (attribute, fetched) in storages.iter() {
							if fetched.is_some() && let Some(s) = match fetched.as_ref().unwrap() {
								FetchedInstanceAttributeSource::Component(storage) => storage.get_component(entity_id),
								FetchedInstanceAttributeSource::Resource(r) => Some(r.as_slice())
							} {
								instance_data.extend_from_slice(s);
							} else if let Some(d) = attribute.default.as_ref() {
								instance_data.extend_from_slice(d.as_slice());
							} else {
								panic!("No entity data and no default!");
							}
						}

						count += 1;
					}
					instance_items.push((current_binding_config, current_mesh, count));

					let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
						label: Some(&*format!("Stage {} shader {} instance buffer", stage, &shader.specification.name)),
						contents: instance_data.as_slice(),
						usage: wgpu::BufferUsages::VERTEX,
					});

					bundle_target_entries.insert(shader_key, vec![(instance_buffer, instance_items)]);
				}

				bundle_stage.targets.insert(target.clone(), bundle_target_entries);
			}

			// Computes
			bundle_stage.computes = input.computes.iter().map(|&(m, w)| {
				let material = materials.key(m).unwrap();
				let (_, bgc) = material.bindings.get(context).unwrap().polygon_stuff();
				(material.shader_key.unwrap(), bgc, w)
			}).collect::<Vec<_>>();

			// Clears
			bundle_stage.clears = input.clears.clone();

			bundle.stages.push(bundle_stage);
		}

		bundle
	}
}

#[derive(Debug, Default)]
struct RenderBundleStage {
	pub targets: HashMap<
		RenderTarget, 
		HashMap<ShaderKey, Vec<(wgpu::Buffer, Vec<([Option<BindGroupKey>; 4], Option<MeshKey>, u32)>)>>
	>,
	pub computes: Vec<(ShaderKey, [Option<BindGroupKey>; 4], [u32; 3])>, // (shader, bgs, workgroup)
	// On target change check if clear exits for target index, clear if true
	pub clears: HashMap<TextureKey, wgpu::ImageSubresourceRange>, 
}

#[derive(Debug, Default)]
pub struct RenderBundle {
	stages: Vec<RenderBundleStage>,
}
impl RenderBundle {
	pub fn execute(
		&self,
		shaders: &ShaderManager,
		bind_groups: &BindGroupManager,
		meshes: &MeshManager,
		textures: &TextureManager,
		encoder: &mut wgpu::CommandEncoder,
	) {
		for bundle in self.stages.iter() {
			for (&key, range) in bundle.clears.iter() {
				info!("Clearing texture {key:?}");
				let texture = textures.index(key).unwrap();
				encoder.clear_texture(&texture.binding().unwrap().texture, range);
			}

			for &(key, bgs, [x,y,z]) in bundle.computes.iter() {
				let shader = shaders.get(key).unwrap();
				let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: None,
				});

				pass.set_pipeline(shader.pipeline.as_ref().unwrap().compute().unwrap());
				for (index, key) in bgs.iter().enumerate() {
					if let &Some(key) = key {
						let bind_group = bind_groups.get(key).unwrap();
						pass.set_bind_group(index as u32, bind_group, &[]);
					}
				}

				pass.dispatch_workgroups(x, y, z);
			}

			for (target, groups) in bundle.targets.iter() {
				let color_attachments = target.colour_attachments.iter().map(|&(attachment, resolve)| {
					Some(wgpu::RenderPassColorAttachment {
						view: &textures.index(attachment).unwrap().binding().unwrap().view,
						resolve_target: resolve.and_then(|r| Some(&textures.index(r).unwrap().binding().unwrap().view)),
						ops: wgpu::Operations { 
							load: wgpu::LoadOp::Load, 
							store: true, // Assume that we will write to any colour attachement
						},
					})
				}).collect::<Vec<_>>();

				let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None, 
					color_attachments: color_attachments.as_slice(), 
					depth_stencil_attachment: target.depth_attachment.and_then(|d| {
						Some(wgpu::RenderPassDepthStencilAttachment {
							view: &textures.index(d).unwrap().binding().unwrap().view,
							depth_ops: Some(wgpu::Operations { 
								load: wgpu::LoadOp::Load, 
								store: true, // Another assumption that we will write
							}),
							stencil_ops: None,
						})
					}), 
				});

				for (&shader_key, batches) in groups.iter() {
					for (instance_buffer, items) in batches {
						pass.set_vertex_buffer(1, instance_buffer.slice(..));

						let shader = shaders.get(shader_key).unwrap();
						info!("Shader {}", shader.specification.name);
						info!("has layout {:?}", shader.pipeline_layout.as_ref().unwrap());
						pass.set_pipeline(shader.pipeline.as_ref().unwrap().polygon().unwrap());
	
						println!("{:#?}", bind_groups);
	
						let mut current_bgs = items[0].0;
						for (index, key) in current_bgs.iter().enumerate() {
							if let &Some(key) = key {
								let bind_group = bind_groups.get(key).unwrap();	
								trace!("Binding bind group {key:?} to index {index}");
								pass.set_bind_group(index as u32, bind_group, &[]);
							}
						}
						let mut current_count = 0;
						for &(bgs, mesh, count) in items {
							// Set bind groups if needed
							for (index, &key) in bgs.iter().enumerate() {
								if current_bgs[index] != key {
									if let Some(key) = key {
										let bind_group = bind_groups.get(key).unwrap();	
										trace!("Binding bind group {key:?} to index {index}");
	
										pass.set_bind_group(index as u32, bind_group, &[]);
										current_bgs[index] = Some(key);
									}
									
								}
							}
	
							let n_vertices = match mesh {
								Some(key) => {
									let mesh = meshes.index(key).unwrap();
									trace!("Binding mesh {:?}", mesh.name);
									let mesh_format_key = shader.mesh_format_key.unwrap();
									let mesh_buffer = mesh.bindings.as_ref().unwrap().vertex_buffers.get(mesh_format_key).unwrap();
									pass.set_vertex_buffer(0, mesh_buffer.slice(..));
									mesh.n_vertices
								},
								// If we haven't been given a mesh, then the shader is generative
								// Fullquad, for example, has 3 vertices
								None => shader.specification.base.polygonal().polygon_input.generative_vertices(),
							};
							
							trace!("Drawing {} thing(s)!", n_vertices);
							pass.draw(0..n_vertices, current_count..current_count+count);
							current_count += count;
						}
					}
					
				}

				trace!("End render target");
			}
		}
		trace!("End execution");
	}
}