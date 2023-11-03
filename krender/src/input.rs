use prelude::RenderTargetOperations;
use slotmap::SlotMap;
use wgpu::Buffer;
use wgpu_profiler::GpuProfiler;
use crate::*;



#[derive(Debug)]
struct InputStage<T> {
	pub items: SlotMap<RenderItemKey, (MaterialKey, Option<MeshKey>, T)>,
	pub static_batches: SlotMap<RenderBatchKey, (MaterialKey, Option<MeshKey>, Buffer, u32, T)>,
	pub computes: SlotMap<RenderComputeKey, (MaterialKey, [u32; 3])>,

	pub texture_clears: HashMap<TextureKey, wgpu::Color>, 
	pub depth_clears: HashMap<TextureKey, f32>, 
}
impl<T: EntityIdentifier> InputStage<T> {
	pub fn new() -> Self {
		Self {
			items: SlotMap::with_key(),
			static_batches: SlotMap::with_key(),
			computes: SlotMap::with_key(),
			texture_clears: HashMap::new(),
			depth_clears: HashMap::new(),
		}
	}
}


#[derive(Debug)]
pub struct RenderInput<T> {
	stages: BTreeMap<String, InputStage<T>>,
	dependencies: Vec<(String, String)>,
}
impl<E: EntityIdentifier> RenderInput<E> {
	pub fn new() -> Self {
		Self {
			stages: BTreeMap::new(),
			dependencies: Vec::new(),
		}
	}

	// This needs to return a texture clear key or soemthing 
	// I just want to render things so I don't care now
	pub fn clear_depth(
		&mut self,
		stage: impl Into<String>,
		texture: TextureKey,
	) {
		let stage = self.stages.entry(stage.into())
			.or_insert(InputStage::new());
		stage.depth_clears.insert(texture, 1.0);
	}

	pub fn insert_item(
		&mut self, 
		stage: impl Into<String>,
		material: MaterialKey,
		mesh: Option<MeshKey>,
		entity_id: E,
	) -> RenderItemKey {
		let stage = self.stages.entry(stage.into())
			.or_insert(InputStage::new());
		stage.items.insert((material, mesh, entity_id))
	}

	pub fn remove_item(
		&mut self, 
		stage: impl Into<String>,
		key: RenderItemKey,
	) {
		self.stages.get_mut(&stage.into())
			.and_then(|s| s.items.remove(key));
	}

	pub fn insert_batch(
		&mut self,
		device: &wgpu::Device,
		stage: impl Into<String>,
		material: MaterialKey,
		mesh: Option<MeshKey>,
		data: &[u8],
		count: u32,
		entity_id: E,
	) -> RenderBatchKey {
		let stage = self.stages.entry(stage.into())
			.or_insert(InputStage::new());
		let b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: data,
			usage: wgpu::BufferUsages::VERTEX,
		});
		stage.static_batches.insert((material, mesh, b, count, entity_id))
	}

	pub fn remove_batch(
		&mut self, 
		stage: impl Into<String>,
		key: RenderBatchKey,
	) {
		self.stages.get_mut(&stage.into())
			.and_then(|s| s.static_batches.remove(key));
	}

	pub fn insert_compute(
		&mut self, 
		stage: impl Into<String>,
		material: MaterialKey,
		x: u32,
		y: u32,
		z: u32,
	) -> RenderComputeKey {
		let stage = self.stages.entry(stage.into())
			.or_insert(InputStage::new());
		stage.computes.insert((material, [x, y, z]))
	}

	pub fn remove_compute(
		&mut self, 
		stage: impl Into<String>,
		key: RenderComputeKey,
	) {
		self.stages.get_mut(&stage.into())
			.and_then(|s| s.computes.remove(key));
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
		meshes: &mut MeshManager,
		materials: &MaterialManager,
		shaders: &'a ShaderManager,
		context: RenderContextKey,
		storage_provider: &'a impl InstanceDataProvider<'a, E>,
	) -> RetainedBundle<'a> {
		let mut bundle = RetainedBundle::default();

		for stage in self.stage_order() {
			profiling::scope!("Stage");
			let input = self.stages.get(stage).unwrap();

			let mut bundle_stage = RetainedBundleStage::default();

			// Collect by target
			// Collect by shader
			let mut targets = HashMap::new();
			{profiling::scope!("Collect by target then shader");
			for &(material_key, mesh, entity_id) in input.items.values() {
				let material = materials.get(material_key).unwrap();
				let shader_key = material.shader().unwrap();

				let (target, binding_config) = material.binding(context).unwrap().polygon_stuff();

				let shader_groups  = targets.entry(target).or_insert_with(|| HashMap::new());
				let data = shader_groups.entry(shader_key).or_insert_with(|| Vec::new());

				data.push((binding_config, mesh, entity_id));
			}}

			// Clears
			let mut texture_clears = input.texture_clears.clone();
			let mut depth_clears = input.depth_clears.clone();

			// Sort
			// Fetch instance data
			for (&target, shader_groups) in targets.iter_mut() {
				profiling::scope!("Target");

				// Todo: fold over the `store` field instead of assuming that we will write
				let mut target = RenderTargetOperations::from_rt(target);
				// See if any attachments must be cleared
				// If so, set them up to be cleared usign loadops
				for (t, _, ops) in target.colour_attachments.iter_mut() {
					if let Some(colour) = texture_clears.remove(&t) {
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

				let mut bundle_target_entries = HashMap::new();

				for (&shader_key, items) in shader_groups.iter_mut() {
					profiling::scope!("Shader");

					{profiling::scope!("Sort");
					items.sort_unstable_by_key(|e| (e.0, e.1));
					}

					let shader = shaders.get(shader_key).unwrap();
					let attributes = &shader.specification.base.polygonal().instance_attributes;

					// Fetch stuff outside of the loop for better efficiency 
					// But now returned data is bound by this lifetime! 
					let storages = attributes.iter()
						.map(|a| (a, storage_provider.fetch_source(&a.source)))
						.collect::<Vec<_>>();

					let mut instance_items = Vec::new();
					let mut instance_data = Vec::new();
					let (mut current_binding_config, mut current_mesh, _) = items[0];
					let mut count: u32 = 0;
					{profiling::scope!("Fetch");
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
								FetchedInstanceAttributeSource::Resource(r) => Some(*r)
							} {
								instance_data.extend_from_slice(s);
							} else if let Some(d) = attribute.default.as_ref() {
								instance_data.extend_from_slice(d.as_slice());
							} else {
								panic!("Error pulling data for {:?}, no entity data and no default!", attribute);
							}
						}

						count += 1;
					}
					instance_items.push((current_binding_config, current_mesh, count));
					}

					let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
						label: Some(&*format!("Stage {} shader {} instance buffer", stage, &shader.specification.name)),
						contents: instance_data.as_slice(),
						usage: wgpu::BufferUsages::VERTEX,
					});

					{profiling::scope!("Bind");
					// Also bind the meshes!
					if let Some(format_key) = shader.mesh_format_key {
						for (_, mesh_key, _) in instance_items.iter() {
							if let &Some(mesh_key) = mesh_key {
								meshes.get_mut(mesh_key).unwrap().bind(format_key);
							}
						}
					}
					}
					
					bundle_target_entries.insert(shader_key, vec![(MaybeOwned::Owned(instance_buffer), instance_items)]);
				}

				bundle_stage.targets.push((target, bundle_target_entries));
			}

			// Do static stuff!
			for (material_key, mesh, instances, count, _) in input.static_batches.values() {
				// let shader = shaders.index(shader_key).unwrap();
				let material = materials.get(*material_key).unwrap();
				let shader_key = material.shader().unwrap();

				let (target, binding_config) = material.binding(context).unwrap().polygon_stuff();
				
				// Help, how do push constants?
				// Alternatively, ANOTHER instance buffer!
				// Actually that might be a good idea
				// The last instance buffer can be the one that holds entity data?
				// We don't have to have n of them, we can set a limit of two

				let g = (MaybeOwned::Borrowed(instances), vec![(binding_config, *mesh, *count)]);

				// if let Some(target) = bundle_stage.targets.get_mut(&target) {
				// 	if let Some(shader) = target.get_mut(&shader_key) {
				// 		shader.push(g);
				// 	}
				// }
			}

			// Computes
			bundle_stage.computes = input.computes.values().map(|&(m, w)| {
				let material = materials.get(m).unwrap();
				let (_, bgc) = material.binding(context).unwrap().polygon_stuff();
				(material.shader().unwrap(), bgc, w)
			}).collect::<Vec<_>>();

			// Store the (now reduced) clears
			bundle_stage.texture_clears = texture_clears;
			bundle_stage.depth_clears = depth_clears;

			bundle.stages.push(bundle_stage);
		}

		debug!("Doing mesh binding");
		meshes.bind_unbound(device);

		bundle
	}
}


// Could not use this by wrapping all index stuff in Arc
#[derive(Debug)]
enum MaybeOwned<'a, T> {
	Owned(T),
	Borrowed(&'a T),
}

#[derive(Debug, Default)]
struct RetainedBundleStage<'a> {
	pub targets: Vec<(
		RenderTargetOperations, 
		HashMap<ShaderKey, Vec<(MaybeOwned<'a, wgpu::Buffer>, Vec<([Option<BindGroupKey>; 4], Option<MeshKey>, u32)>)>>
	)>,
	pub computes: Vec<(ShaderKey, [Option<BindGroupKey>; 4], [u32; 3])>, // (shader, bgs, workgroup)
	// On target change check if clear exits for target index, clear if true
	pub texture_clears: HashMap<TextureKey, wgpu::Color>, 
	pub depth_clears: HashMap<TextureKey, f32>, 
}

#[derive(Debug, Default)]
pub struct RetainedBundle<'a> {
	stages: Vec<RetainedBundleStage<'a>>,
}
impl<'a> RetainedBundle<'a> {
	// Only counts draw calls, excludes computes and clears
	pub fn draw_count(&self) -> u64 {
		let mut draw_count = 0;
		for stage in self.stages.iter() {
			for (_, shaders) in stage.targets.iter() {
				for batch in shaders.values() {
					draw_count += batch.len() as u64;
				}
			}
		}
		draw_count
	}

	pub fn execute(
		mut self,
		shaders: &ShaderManager,
		bind_groups: &BindGroupManager,
		meshes: &MeshManager,
		textures: &TextureManager,
		encoder: &mut wgpu::CommandEncoder,
		device: &wgpu::Device,
		profiler: &mut GpuProfiler,
	) {
		for (i, bundle) in self.stages.iter_mut().enumerate() {
			profiler.begin_scope(&*format!("Stage {i}"), encoder, device);

			// Could move this to after iterating bundles, which would save computation
			// But also it messes with my head
			// I want something cleared BEFORE the rendering please
			for (key, colour) in bundle.texture_clears.iter() {
				info!("Clearing texture {key:?}");
				// let texture = textures.get(key).unwrap();
				// encoder.clear_texture(&texture.binding().unwrap().texture, range);
				todo!("do another render pass to clear the texture?")
			}
			for (key, value) in bundle.depth_clears.iter() {
				info!("Clearing texture {key:?}");
				todo!("do another render pass to clear the texture?")
			}

			if !bundle.computes.is_empty() {
				let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: None,
				});
				for &(key, bgs, [x,y,z]) in bundle.computes.iter() {
					let shader = shaders.get(key).unwrap();
					
					profiler.begin_scope(&*format!("Compute shader '{}'", shader.specification.name), &mut pass, device);
					pass.set_pipeline(shader.pipeline.as_ref().unwrap().compute().unwrap());
					for (index, key) in bgs.iter().enumerate() {
						if let &Some(key) = key {
							let bind_group = bind_groups.get(key).unwrap();
							pass.set_bind_group(index as u32, bind_group, &[]);
						}
					}
					pass.dispatch_workgroups(x, y, z);
					profiler.end_scope(&mut pass);
				}
			}

			for (target, groups) in bundle.targets.iter() {
				let color_attachments = target.colour_attachments.iter().map(|&(attachment, resolve, ops)| {
					Some(wgpu::RenderPassColorAttachment {
						view: &textures.get(attachment)
							.expect("texture not found")
							.binding()
							.expect("texture not bound").view,
						resolve_target: resolve.and_then(|r| Some(&textures.get(r).unwrap().binding().unwrap().view)),
						ops,
					})
				}).collect::<Vec<_>>();

				let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None, 
					color_attachments: color_attachments.as_slice(), 
					depth_stencil_attachment: target.depth_attachment.and_then(|(d, depth_ops)| {
						Some(wgpu::RenderPassDepthStencilAttachment {
							view: &textures.get(d).unwrap().binding().unwrap().view,
							depth_ops: Some(depth_ops),
							stencil_ops: None,
						})
					}), 
				});

				for (&shader_key, batches) in groups.iter() {
					let shader = shaders.get(shader_key).unwrap();
					profiler.begin_scope(&*format!("Shader '{}'", shader.specification.name), &mut pass, device);

					for (instance_buffer, items) in batches {
						let buffer_slice = match instance_buffer {
							MaybeOwned::Owned(b) => b.slice(..),
							MaybeOwned::Borrowed(b) => b.slice(..),
						};
						pass.set_vertex_buffer(1, buffer_slice);

						
						info!("Shader {}", shader.specification.name);
						// info!("has layout {:?}", shader.pipeline_layout.as_ref().unwrap());
						pass.set_pipeline(shader.pipeline.as_ref().unwrap().polygon().unwrap());
	
						let mut current_bgs = items[0].0;
						for (index, key) in current_bgs.iter().enumerate() {
							if let &Some(key) = key {
								let bind_group = bind_groups.get(key).unwrap();	
								// trace!("Binding bind group {key:?} to index {index}");
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
										// trace!("Binding bind group {key:?} to index {index}");
	
										pass.set_bind_group(index as u32, bind_group, &[]);
										current_bgs[index] = Some(key);
									}
									
								}
							}
	
							let (indexed, n) = match mesh {
								Some(key) => {
									let mesh = meshes.get(key).unwrap();
									// trace!("Binding mesh {:?}", mesh.name);
									let mesh_format_key = shader.mesh_format_key.unwrap();
									let mesh_buffer = mesh.bindings.as_ref()
										.expect(&*format!("mesh {} has no bindings! {:?}", mesh.name, mesh.pending_binds))
										.vertex_buffers.get(mesh_format_key)
										.expect("mesh bindings doesn't contain this format!");
									pass.set_vertex_buffer(0, mesh_buffer.slice(..));

									if let Some((index_buffer, index_format)) = mesh.bindings.as_ref().unwrap().index_buffer.as_ref() {
										pass.set_index_buffer(index_buffer.slice(..), *index_format);
										(true, mesh.indices.as_ref().unwrap().len() as u32)
									} else {
										(false, mesh.n_vertices)
									}
								},
								// If we haven't been given a mesh, then the shader is generative
								// Fullquad, for example, has 3 vertices
								None => (false, shader.specification.base.polygonal().polygon_input.generative_vertices()),
							};
							
							// trace!("Drawing {} vertices!", n);
							if indexed {
								pass.draw_indexed(0..n, 0, current_count..current_count+count)
							} else {
								pass.draw(0..n, current_count..current_count+count);
							}
							
							current_count += count;
						}
					}
					profiler.end_scope(&mut pass);
				}
				// trace!("End render target");
			}
			profiler.end_scope(encoder);
		}
		// trace!("End execution");
	}
}
