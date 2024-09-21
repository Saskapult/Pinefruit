use std::{num::NonZeroU32, sync::Arc};
use arrayvec::ArrayVec;
use eks::{entity::Entity, World};
use crate::buffer::BufferManager;
use crate::mesh::MeshManager;
use crate::prelude::{AbstractRenderTarget, BindGroupManager, InstanceAttributeSource, MaterialManager, RenderContext};
use crate::rendertarget::RRID;
use crate::shader::ShaderManager;
use crate::texture::TextureManager;
use crate::vertex::FetchedInstanceAttributeSource;
use crate::{BindGroupKey, BufferKey, MaterialKey, MeshKey, ShaderKey, TextureKey};
use wgpu::util::DeviceExt;


type StageKey = u8;
type TargetKey = u8;

type RenderInputItem = (StageKey, StageItem);

#[derive(variantly::Variantly)]
enum StageItem {
	Clear(RRID, ClearValue),
	Compute(MaterialKey, [u32; 3]),
	Draw(TargetKey, DrawItem),
}
#[derive(variantly::Variantly)]

enum DrawItem {
	Draw(MaterialKey, VertexSource, Entity), 
	// Mesh draw range is controlled by indirect buffer, so full mesh is bound 
	// Mesh decides if indexed or not, and if no mesh then not indexed
	// Indirect buffer, instance buffer 
	Indirect(MaterialKey, Option<MeshKey>, BufferKey, BufferKey), 
	// Must be last because they reset the render pass' state
	// How do we know that buffers will not have been re-bound since this bundle was created? 
	Bundle(Arc<wgpu::RenderBundle>),
}

type RenderBufferItem = (StageKey, StageOp);

#[derive(Debug, Clone)]
enum StageOp {
	// Bool is for tacking if this has fulfilled during execution (init to false)
	Clear(TextureKey, ClearValue, bool),
	Compute(ShaderKey, [Option<BindGroupKey>; 4], [u32; 3]),
	Draw(TargetKey, DrawOp),
}
impl StageOp {
	#[inline]
	fn disc(s: &Self) -> u32 {
		match s {
			&Self::Clear(_, _, _) => 0,
			&Self::Compute(_, _, _) => 1,
			&Self::Draw(_, _) => 2,
		}
	}
}
impl std::cmp::PartialEq for StageOp {
	fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}
impl std::cmp::Eq for StageOp {}
impl std::cmp::PartialOrd for StageOp {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl std::cmp::Ord for StageOp {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		match (self, other) {
			(Self::Compute(s0, bgc0, _), Self::Compute(s1, bgc1, _)) => (s0, bgc0).cmp(&(s1, bgc1)),
			(
				Self::Draw(t0, o0), 
				Self::Draw(t1, o1),
			) => (t0, o0).cmp(&(t1, o1)),
			_ => Self::disc(self).cmp(&Self::disc(other)),
		}
	}
}
#[derive(Debug, Clone, Copy)]
enum ClearValue {
	Colour([f32; 4]),
	Depth(f32),
}
#[derive(Debug, Clone)]
enum DrawOp {
	Draw(ShaderKey, [Option<BindGroupKey>; 4], VertexSource, DrawInstance),
	Bundle(Arc<wgpu::RenderBundle>),
}
impl DrawOp {
	#[inline]
	fn disc(s: &Self) -> u32 {
		match s {
			&Self::Draw(_, _, _, _) => 0,
			&Self::Bundle(_) => 1,
		}
	}
}
impl std::cmp::PartialEq for DrawOp {
	fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}
impl std::cmp::Eq for DrawOp {}
impl std::cmp::PartialOrd for DrawOp {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl std::cmp::Ord for DrawOp {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		match (self, other) {
			(
				Self::Draw(s0, bgc0, vs0, dt0), 
				Self::Draw(s1, bgc1, vs1, dt1),
			) => (s0, bgc0, vs0, dt0).cmp(&(s1, bgc1, vs1, dt1)),
			_ => Self::disc(self).cmp(&Self::disc(other)),
		}
	}
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum VertexSource {
	// A mesh and a draw range 
	Mesh(MeshKey, DrawRange),
	// Uses a static draw range specified by the shader  
	Static, 
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum DrawRange {
	All, // Full draw range of a mesh
	Some(u32, NonZeroU32),
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, variantly::Variantly)]
enum DrawInstance {
	// Instance buffer start, for rebinding when shader changes 
	// This data is duplicated but does not affect total size becuase Self::Indirect is larger 
	Main(u32), 
	// Indirect draw instance buffers do not include offset :(
	// But if you're doing indirect draw calls you are likely using bindless
	// stuff and don't need offsets anyway 'cause that's the whole point 
	// Instance buffer, indirect buffer
	Indirect(BufferKey, BufferKey),
}


pub struct RenderInput2 {
	// Stage name, depends_on, order in queue (max for not in queue) 
	stages: Vec<(String, Vec<StageKey>, u8)>,
	// Targets[0] is a dummy target for compute shaders 
	// Indices into this should start at 1 
	targets: Vec<AbstractRenderTarget>,

	items: Vec<RenderInputItem>,
	// A count of how many items have been inserted since the last sort
	// If it is big, then we do some heap allocation and a faster sort 
	// n_inserted: usize,
	
	// Could strip generation from keys? Map keys with secondary map 
	// Can we reduce the size somehow? Is that needed? 
	items_buffer: Vec<RenderBufferItem>,
	// Used to create the instance buffer, retained to avoid reallocation 
	instance_bytes: Vec<u8>,
}
impl RenderInput2 {
	pub fn stage(&mut self, id: impl AsRef<str>) -> StageBuilder {
		// let l = self.stages.len();
		// self.stages.entry_ref(id.as_ref()).or_insert_with(|| (l, vec![])).0
		let stage = if let Some(k) = self.stages.iter().position(|e| e.0 == id.as_ref()) {
			k
		} else {
			let k = self.stages.len();
			self.stages.push((id.as_ref().to_string(), vec![], u8::MAX));
			k
		} as StageKey;
		StageBuilder { stage, input: self }
	}
	
	fn make_or_get_target(&mut self, art: AbstractRenderTarget) -> TargetKey {
		if let Some(k) = self.targets.iter().position(|e| e == &art) {
			k as TargetKey
		} else {
			let k = self.stages.len();
			self.targets.push(art);
			k as TargetKey
		} 
	}

	pub fn add_stage_dep(&mut self, stage: StageKey, depends_on: StageKey) {
		self.stages.get_mut(stage as usize).unwrap().1.push(depends_on);
	}

	pub fn run(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		textures: &mut TextureManager,
		buffers: &mut BufferManager,
		materials: &mut MaterialManager,
		shaders: &mut ShaderManager,
		bind_groups: &mut BindGroupManager,
		meshes: &mut MeshManager,
		context: &mut RenderContext,
		world: &World,
	) -> Vec<wgpu::CommandBuffer> {
		{
			profiling::scope!("reource preparation");

			materials.read_shaders_and_fetch_keys(shaders);
			
			shaders.load_and_register(meshes, bind_groups);
			
			bind_groups.build_layouts(device);
			
			materials.read_specified_resources(shaders, textures, buffers).unwrap();

			context.bind_materials(
				materials, 
				shaders, 
				textures, 
				buffers, 
				bind_groups,
			).unwrap();

			textures.update_bindings(device, queue, bind_groups);
			buffers.update_bindings(device, bind_groups);
	
			bind_groups.update_bindings(device, textures, buffers);
			shaders.build_pipelines(device, bind_groups);
		}

		{
			profiling::scope!("find render stage order");
			// Find stage order if any changed
			if self.stages.iter().any(|(_, _, o)| *o == u8::MAX) {
				let mut stage_i = 0;
				self.stages.iter_mut().for_each(|(_, _, o)| *o = u8::MAX);
				while let Some(i) = self.stages.iter().position(|(_, d, o)| {
					*o == u8::MAX && d.iter().copied().all(|d| self.stages[d as usize].2 != u8::MAX)
				}) {
					let (stage, _, o) = &mut self.stages[i];
					trace!("{}: stage {:?}", stage_i, stage);
					*o = stage_i;
					stage_i += 1;
				}
			}
			// Helpful error 
			if self.stages.iter().any(|(_, _, s)| *s == u8::MAX) {
				error!("Unsatisfied stage dependencies:");
				for (stage, deps, _) in self.stages.iter().filter(|(_, _, s)| *s == u8::MAX) {
					let dep_stages = deps.iter().copied()
						.filter_map(|i| (self.stages[i as usize].2 == u8::MAX).then_some(&self.stages[i as usize].0)).collect::<Vec<_>>();
					error!("\t{:?} depends on unsatisfied {:?}", stage, dep_stages);
				}
				panic!("Unsatified stage dependencies!");
			}
			// Remap items 
			self.items.iter_mut().for_each(|t| t.0 = self.stages[t.0 as usize].2);
		}

		{
			profiling::scope!("map render items to render buffer");
			self.items_buffer.clear();
			let additional = self.items.len().saturating_sub(self.items_buffer.capacity());
			self.items_buffer.reserve(additional);

			for (stage, item) in self.items.iter() {
				self.items_buffer.push((*stage, match item {
					StageItem::Clear(rrid, v) => {
						let t = rrid.texture(context, textures).expect("Failed to locate target texture (todo: give more information)");
						StageOp::Clear(t, *v, false)
					},
					StageItem::Compute(mtl, s) => {
						let m = materials.get(*mtl).unwrap();
						let shader = m.shader_key.unwrap();
						// Make accessor function for this please 
						let bgc = context.material_bindings.get(*mtl).unwrap().bind_groups;
						StageOp::Compute(shader, bgc, *s)
					},
					StageItem::Draw(t, d) => StageOp::Draw(*t, match d {
						DrawItem::Bundle(b) => DrawOp::Bundle(b.clone()),
						DrawItem::Indirect(mtl, mesh, ind, ins) => {
							let m = materials.get(*mtl).unwrap();
							let shader = m.shader_key.unwrap();
							let bgc = context.material_bindings.get(*mtl).unwrap().bind_groups;
							let vs = match mesh {
								// TODO: let the user specifiy the draw range 
								Some(m) => VertexSource::Mesh(*m, DrawRange::All),
								None => VertexSource::Static,
							};
							// Confusing order swap here
							let di = DrawInstance::Indirect(*ins, *ind);
							DrawOp::Draw(shader, bgc, vs, di)
						},
						DrawItem::Draw(mtl, vs, _) => {
							let m = materials.get(*mtl).unwrap();
							let shader = m.shader_key.unwrap();
							let bgc = context.material_bindings.get(*mtl).unwrap().bind_groups;
							let di = DrawInstance::Main(0);
							DrawOp::Draw(shader, bgc, *vs, di)
						},
					}),
				}));
			}
		}

		if true {
			profiling::scope!("render sort (insertion)");
			// Double-action insertion sort!
			// Great for retained mode 
			for i in 1..self.items_buffer.len() {
				for j in (1..i+1).rev() {
					if self.items_buffer[j-1] <= self.items_buffer[j] {
						break
					}
					self.items_buffer.swap(j-1, j);
					self.items.swap(j-1, j);
				}
			}
		} else {
			profiling::scope!("render sort (std)");
			todo!("Std sort render sort");
			// Useful if we have barely-sorted data and don't mind paying the heap allocation cost 
			// Ooor if I turns out that insertion sort is generally slower 
			// Allocate additional vecs with indices, sort with std, re-order base
			// We are unable to not remap the base because instance data pulling 
			// relies on it following the same order as the buffer items 
		}
		
		{
			profiling::scope!("fetch instance data");
			// Pull instance data 
			// TODO: precalculate total instance data size
			// - create buffer ahead of time to parallelize fetch and encoding 
			self.instance_bytes.clear();
			let mut cur_st = 0;
			let mut cur_shader = None;
			let mut storages = ArrayVec::<_, 8>::new();
			let mut cur_size = 0; 
			let mut cur_count = 0;
			for (i, (_, op)) in self.items_buffer.iter_mut().enumerate() {
				match op {
					StageOp::Draw(_, DrawOp::Draw(shader, _, _, DrawInstance::Main(o))) => {
						if Some(*shader) != cur_shader {
							cur_shader = Some(*shader);
							let new_shader = shaders.get(*shader)
								.expect("Failed to locate shader!");
							let new_attributes = &new_shader.specification.base.polygonal().instance_attributes;
							storages.clear();
							for a in new_attributes.iter() {
								storages.push((a, match &a.source {
									InstanceAttributeSource::Component(component_id) => {
										Some(FetchedInstanceAttributeSource::<'_>::Component(world.component_raw_ref(component_id)))
									},
									InstanceAttributeSource::Resource(resource_id) => Some(FetchedInstanceAttributeSource::<'_>::Resource(world.resource_raw_ref(resource_id))),
								}));
							}
							let new_size = new_attributes.iter().fold(0, |a, v| v.size() as usize + a) as u32;

							// Begin new segment 
							let segment_size = cur_count * cur_size;
							if cur_count != 0 {
								let shader = shaders.get(cur_shader.unwrap()).unwrap();
								trace!("Created instance segment of {} bytes for shader {:?}", segment_size, shader.specification.name);
							}
							cur_count = 0;
							cur_st += segment_size;
							cur_size = new_size;
						}

						*o = cur_st;
						cur_count += 1;
						for (_, s) in storages.iter() {
							match s.as_ref().unwrap() {
								FetchedInstanceAttributeSource::Component(storage) => {
									let entity = self.items[i].1.draw_ref().unwrap().1.draw_ref().unwrap().2;
									storage.render_extend(*entity, &mut self.instance_bytes);
								},
								FetchedInstanceAttributeSource::Resource(r) => {
									r.render_extend(&mut self.instance_bytes);
								},
							}
						}
					}
					_ => {},
				}
			}
			if cur_count != 0 && cur_shader.is_some() {
				let shader = shaders.get(cur_shader.unwrap()).unwrap();
				trace!("Created instance segment of {} bytes for shader {:?}", cur_count * cur_size, shader.specification.name);
			}
		}
		let instance_buffer = {
			// If profiling reveals that this is slow, we can try retaining it until the contents do not fit
			profiling::scope!("create instance buffer");
			device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("main instance buffer"),
				contents: self.instance_bytes.as_slice(),
				usage: wgpu::BufferUsages::VERTEX,
			})
		};

		meshes.bind_unbound(device);

		if false {
			let n_cores = 8;
			let segment_size = self.items_buffer.len() / n_cores;
			if segment_size < 32 {
				warn!("Parallel encoding is likely doing more harm than good (low batch size)");
			}

			// Can find draw call count of sequence 
			// Can find target count of sequence 
			// Can combine into split heuristic 
			// We can split clears, it just incurs extra passes 

			// At least 8 draw calls? 
			// self.items_buffer.split_at_mut(mid)

			// fn par_stage_exec(slice: &[bool]) -> Vec<wgpu::CommandBuffer> {
			// 	// Find this partition 
			// 	// While below capacity
			// 	// Find size of next partition 
			// 	let (buff, mut buffs) = (excute(your_bit), par_exec(other_bit));
			// 	buffs.push_front(buff);
			// 	buffs
			// }

			todo!("Parallel encoding")
		} else {
			vec![execute_sequence(
				&mut self.items_buffer, 
				&self.targets, 
				device, 
				textures, 
				buffers, 
				bind_groups, 
				shaders, 
				meshes, 
				&instance_buffer, 
				context,
			)]
		}

	}
}


#[derive(Debug, variantly::Variantly)]
enum PassState<'a> {
	Compute {
		pass: wgpu::ComputePass<'a>,
		shader: Option<ShaderKey>,
		bgc: [Option<BindGroupKey>; 4],
	},
	Render {
		target: TargetKey,
		pass: wgpu::RenderPass<'a>,
		shader: Option<ShaderKey>,
		bgc: [Option<BindGroupKey>; 4],
		mesh: Option<MeshKey>,
		instance: Option<DrawInstance>,
		instance_st: u32, // Accumulated instance st
	},
}


/// Executes a sequence of commands. 
/// We can split the sequence by some heuristic and parallelize encoding. 
fn execute_sequence(
	sequence: &mut [RenderBufferItem],
	targets: &Vec<AbstractRenderTarget>,
	device: &wgpu::Device,
	textures: &TextureManager,
	buffers: &BufferManager,
	bind_groups: &BindGroupManager,
	shaders: &ShaderManager,
	meshes: &MeshManager,
	instance_buffer: &wgpu::Buffer,
	context: &RenderContext,
) -> wgpu::CommandBuffer {
	profiling::scope!("sequence execution");

	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
		label: Some("sequence encoder"),
	});

	let mut cur_stage = None;
	let mut cur_clears;
	let mut state: Option<PassState> = None;

	let mut i = 0;
	while i < sequence.len() {
		let stage = sequence[i].0;
		if cur_stage != Some(stage) {
			profiling::scope!("stage swap (lookahead)");
			cur_stage = Some(stage);
			trace!("Begin stage {}", stage);
			state = None;

			// Find clears partition (it's from here until next non-clear)
			let clears_end = sequence[i..].iter().position(|i| match i.1 {
				StageOp::Clear(_, _, _) => false,
				_ => true,
			}).unwrap_or(sequence.len()-1);
			cur_clears = Some(i..clears_end);
			
			// Look ahead until end of stage to find passes 
			// Make passes for clears that are not in upcoming passes 
			// I, however, am stupid so I will just clear them all! 
			for (_, clear) in sequence[cur_clears.unwrap()].iter_mut() {
				match clear {
					StageOp::Clear(t, v, f) => {
						*f = true;
						let texture = textures.get(*t).unwrap();
						trace!("Clear {:?}", texture.label);
						let view = texture.view().unwrap();
						match v {
							ClearValue::Colour([r, g, b, a]) => {
								encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
									label: Some("clear pass"),
									color_attachments: &[Some(wgpu::RenderPassColorAttachment {
										view, 
										resolve_target: None,
										ops: wgpu::Operations {
											load: wgpu::LoadOp::Clear(wgpu::Color {
												r: *r as f64, g: *g as f64, b: *b as f64, a: *a as f64,
											}),
											store: wgpu::StoreOp::Store, // unclear what this does
										},
									})],
									depth_stencil_attachment: None,
									timestamp_writes: None,
									occlusion_query_set: None,
								});
							},
							ClearValue::Depth(d) => {
								encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
									label: Some("clear pass"),
									color_attachments: &[],
									depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
										view,
										depth_ops: Some(wgpu::Operations {
											load: wgpu::LoadOp::Clear(*d),
											store: wgpu::StoreOp::Store, // see above
										}),
										stencil_ops: None,
									}),
									timestamp_writes: None,
									occlusion_query_set: None,
								});
							},
						}
					}
					_ => unreachable!("Non-StageOp::Clear variant in clears segment!"),
				}
			}
			
			// Skip over those clears
			trace!("Skipped over {} clear operations", clears_end - i);
			i = clears_end; 
			continue
		}

		let op = &sequence[i].1;
		match op {
			StageOp::Clear(_, _, _) => unreachable!(),
			&StageOp::Compute(os, obgc, [x, y, z]) => {
				if state.is_none() || state.as_ref().unwrap().is_not_compute() {
					drop(state);
					let pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
						label: None,
						timestamp_writes: None,
					});
					state = Some(PassState::Compute { pass, shader: None, bgc: [None; 4], });
				}
				match state.as_mut().unwrap() {
					PassState::Compute { pass, shader, bgc } => {
						if *shader != Some(os) {
							*shader = Some(os);
							let shader = shaders.get(os).unwrap();
							trace!("Set shader to {:?}", shader.specification.name);
							let pipeline = shader.pipeline.as_ref().unwrap()
								.compute().unwrap();
							pass.set_pipeline(pipeline);
						}
						for i in 0..obgc.len() {
							if let Some(bg) = obgc[i] {
								if bgc[i].is_none() || bgc[i].unwrap() != bg {
									bgc[i] = Some(bg);
									let entry = bind_groups.get(bg).unwrap();
									trace!("Set bind group {}", i);
									let bind_group = entry.binding.as_ref().unwrap();
									pass.set_bind_group(i as u32, bind_group, &[]);
								}
							}
						}
						pass.dispatch_workgroups(x, y, z);
					},
					_ => unreachable!(),
				}
			},
			StageOp::Draw(t, ref d) => {
				// If state is not draw or target is not t, make new pass 
				// OR state.as_ref().unwrap().render().target is wrong
				let target_is_wrong = || {
					match state.as_ref().unwrap() {
						PassState::Render { target, .. } => target != t,
						_ => unreachable!(),
					}
				};
				if state.is_none() || state.as_ref().unwrap().is_not_render() || target_is_wrong() {
					let target = &targets[*t as usize];
					trace!("Begin render pass for target {}", t);
					let starget = target.specify(context, textures);

					let color_attachments = starget.colour_attachments.into_iter().map(|(t, r, _)| {
						trace!("Colour attachment");
						let view = {
							let t = textures.get(t).unwrap();
							t.view().unwrap()
						};
						let resolve_target = r.map(|r| {
							let r = textures.get(r).unwrap();
							r.view().unwrap()
						});
						Some(wgpu::RenderPassColorAttachment {
							view, resolve_target, ops: wgpu::Operations {
								load: wgpu::LoadOp::Load,
								store: wgpu::StoreOp::Store,
							},
						})
					}).collect::<ArrayVec<_, 8>>();

					let depth_stencil_attachment = starget.depth_attachment.map(|(d, _)| {
						trace!("Depth attachment");
						let view = {
							let d = textures.get(d).unwrap();
							d.view().unwrap()
						};
						wgpu::RenderPassDepthStencilAttachment {
							view, stencil_ops: None, depth_ops: Some(wgpu::Operations {
								load: wgpu::LoadOp::Load,
								store: wgpu::StoreOp::Store,
							}),
						}
					});

					// Find unsatisfied clears 
					// for (t, v, s) in sequence[cur_clears.unwrap()].iter_mut().filter_map(|(_, op)| match op {
					// 	StageOp::Clear(t, v, s) => (!*s).then_some((t, v, s)),
					// 	_ => unreachable!("Non-clear op in ops thing"),
					// }) {
					// 	todo!("Look to see if this texture is part of the target pass");
					// 	// If texture in this pass, clear and set as satisfied 
					// }
					// trace!("Clears: ");

					drop(state);
					let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
						label: None, 
						color_attachments: color_attachments.as_slice(),
						depth_stencil_attachment,
						timestamp_writes: None,
						occlusion_query_set: None,
					});
					state = Some(PassState::Render { 
						target: *t, 
						pass, 
						shader: None, 
						bgc: [None; 4], 
						mesh: None, 
						instance: None, 
						instance_st: 0,
					});
				}

				match &mut state.as_mut().unwrap() {
					PassState::Render { 
						target: _, pass, shader, bgc, mesh, instance, instance_st, 
					} => {
						match d {
							DrawOp::Bundle(b) => {
								trace!("Execute render bundle");
								pass.execute_bundles([b.as_ref()]);
								// Wipe the pass' state 
								*shader = None;
								*bgc = [None; 4];
								*mesh = None;
								*instance = None;
							},
							&DrawOp::Draw(ds, dbgc, dvs, dinstance) => {
								if *shader != Some(ds) {
									*shader = Some(ds);
									let shader = shaders.get(ds).unwrap();
									trace!("Set shader to {:?}", shader.specification.name);
									let pipeline = shader.pipeline.as_ref().unwrap()
										.polygon().unwrap();
									pass.set_pipeline(pipeline);
								}
								for i in 0..bgc.len() {
									if let Some(bg) = dbgc[i] {
										if bgc[i].is_none() || bgc[i].unwrap() != bg {
											bgc[i] = Some(bg);
											let entry = bind_groups.get(bg).unwrap();
											trace!("Set bind group {}", i);
											let bind_group = entry.binding.as_ref().unwrap();
											pass.set_bind_group(i as u32, bind_group, &[]);
										}
									}
								}

								let (indexed, dr) = match dvs {
									VertexSource::Static => {
										let n = shaders.get(ds).unwrap().specification.base.polygonal().polygon_input.generative_vertices();
										(false, 0..n)
									},
									VertexSource::Mesh(k, dr) => {
										let m = meshes.get(k).unwrap();
										if *mesh != Some(k) {
											trace!("Set mesh {:?}", m.name);
											*mesh = Some(k);
										}
										let dr = match dr {
											DrawRange::Some(st, en) => st..en.get(),
											DrawRange::All => 0..m.n_vertices,
										};
										(true, dr)
									},
								};

								if *instance != Some(dinstance) {
									*instance = Some(dinstance);
									match dinstance {
										DrawInstance::Main(st) => {
											trace!("Set instance buffer to main at offset {}", st);
											pass.set_vertex_buffer(1, instance_buffer.slice((st as u64)..));
										},
										DrawInstance::Indirect(k, _) => {
											let b = buffers.get(k).unwrap();
											trace!("Set instance buffer to {:?}", b.name);
											pass.set_vertex_buffer(1, b.binding.as_ref().unwrap().slice(..));
											*instance_st = 0;
										},
									};
								}

								if let DrawInstance::Indirect(_, ib) = instance.unwrap() {
									let indirect_buffer = buffers.get(ib).unwrap().binding.as_ref().unwrap();

									if !indexed {
										trace!("Draw indirect");
										pass.draw_indirect(indirect_buffer, 0);
									} else {
										trace!("Draw indexed indirect");
										pass.draw_indexed_indirect(indirect_buffer, 0)
									}
								} else {
									// Find batches to draw
									let instance_count = sequence[i..].iter()
										.position(|(_, oop)| op != oop)
										.unwrap_or(sequence[i..].len());
									let instance_range = (*instance_st)..(*instance_st + (instance_count as u32)); 
									*instance_st += instance_count as u32;
									i = instance_count; // Skip over! 
									trace!("Batch {} instances", instance_count);

									if !indexed {
										trace!("Draw vertices {:?} instances {:?}", dr, instance_range);
										// trace!("Draw vertices {}..{} instances {}..{}", instance_range.start, instance_range.end);
										pass.draw(dr, instance_range);
									} else {
										trace!("Draw (indexed) vertices {:?} instances {:?}", dr, instance_range);
										pass.draw_indexed(dr, 0, instance_range);
									}
								}
							},
						}
					},
					_ => unreachable!(),
				}
			}
		}
		i += 1;
	}
	drop(state);
	encoder.finish()
}


pub struct StageBuilder<'a> {
	stage: StageKey,
	input: &'a mut RenderInput2,
}
impl<'a> StageBuilder<'a> {
	pub fn run_before(self, other: impl AsRef<str>) -> Self {
		let other = self.input.stage(other).key();
		self.input.add_stage_dep(self.stage, other);
		self
	}

	pub fn run_after(self, other: impl AsRef<str>) -> Self {
		let other = self.input.stage(other).key();
		self.input.add_stage_dep(other, self.stage);
		self
	}

	pub fn clear_texture(self, texture: RRID, value: [f32; 4]) -> Self {
		self.input.items.push((self.stage, StageItem::Clear(texture, ClearValue::Colour(value))));
		self
	}

	pub fn clear_depth(self, texture: RRID, value: f32) -> Self {
		self.input.items.push((self.stage, StageItem::Clear(texture, ClearValue::Depth(value))));
		self
	}

	pub fn target(self, target: AbstractRenderTarget) -> TargetQueue<'a> {
		let target = self.input.make_or_get_target(target);
		TargetQueue {
			stage_builder: self,
			target,
		}
	}

	pub fn compute(self, material: MaterialKey, size: [u32; 3]) -> Self {
		self.input.items.push((self.stage, StageItem::Compute(material, size)));
		todo!()
	}

	pub fn key(self) -> StageKey {
		self.stage
	}
}

pub struct TargetQueue<'a> {
	stage_builder: StageBuilder<'a>,
	target: TargetKey,
}
impl<'a> TargetQueue<'a> {
	pub fn pass(self, material: MaterialKey, entity: Entity) -> Self {
		self.stage_builder.input.items.push((self.stage_builder.stage, StageItem::Draw(self.target, DrawItem::Draw(material, VertexSource::Static, entity))));
		self
	}

	pub fn mesh(self, material: MaterialKey, mesh: MeshKey, entity: Entity) -> Self {
		self.stage_builder.input.items.push((self.stage_builder.stage, StageItem::Draw(self.target, DrawItem::Draw(material, VertexSource::Mesh(mesh, DrawRange::All), entity))));
		self
	}

	pub fn mesh_range(self, material: MaterialKey, mesh: MeshKey, st: u32, en: u32, entity: Entity) -> Self {
		let en = NonZeroU32::new(en).unwrap();
		self.stage_builder.input.items.push((self.stage_builder.stage, StageItem::Draw(self.target, DrawItem::Draw(material, VertexSource::Mesh(mesh, DrawRange::Some(st, en)), entity))));
		self
	}

	pub fn bundle(self, bundle: Arc<wgpu::RenderBundle>) -> Self {
		self.stage_builder.input.items.push((self.stage_builder.stage, StageItem::Draw(self.target, DrawItem::Bundle(bundle))));
		todo!()
	}

	pub fn indirect(self, material: MaterialKey, indirect: BufferKey, instance: BufferKey) -> Self {
		self.stage_builder.input.items.push((self.stage_builder.stage, StageItem::Draw(self.target, DrawItem::Indirect(material, None, indirect, instance))));
		self
	}

	pub fn indirect_mesh(self, material: MaterialKey, mesh: MeshKey, indirect: BufferKey, instance: BufferKey) -> Self {
		self.stage_builder.input.items.push((self.stage_builder.stage, StageItem::Draw(self.target, DrawItem::Indirect(material, Some(mesh), indirect, instance))));
		self
	}

	pub fn finish(self) -> StageBuilder<'a> {
		self.stage_builder
	}
}
