use std::{num::NonZeroU32, ops::Range, sync::Arc};

use arrayvec::ArrayVec;
use eks::{entity::Entity, World};
use hashbrown::HashMap;
use prelude::{AbstractRenderTarget, RenderContext, InstanceAttributeSource};
use rendertarget::RRID;
use slotmap::{SlotMap, SecondaryMap};
use wgpu::util::RenderEncoder;
use crate::{*, bundle::{RenderBundleStage, MaybeOwned, RenderBundle}};


type StageKey = u8;
type TargetKey = u8;


enum RenderClear {
	Colour([f32; 4]),
	Depth(f32),
}


#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RenderMesh {
	// Draw range, account for it when grouping! 
	// Would be so much easier with range 
	Mesh(MeshKey, u32, u32),
	// Uses a static draw range specified by the shader  
	Static, 
}


enum RenderInputItem {
	// Clears are immediately mapped to an abstract target 
	// This is clever becuase it allows us to group clears during sorting 
	// This is bad becuase we don't combine clears 
	// If you clear albedo, for example, it will not be grouped in a pass 
	// with both albedo and depth 
	// Maybe we can have compunding targets? 
	Clear(RenderClear), 
	Compute(MaterialKey, [u32; 3]), 
	Draw(MaterialKey, RenderMesh, Entity),
	// Instance buffer? We need that! Unless we do manual stuff? 
	Indirect(MaterialKey, Option<MeshKey>, BufferKey), 
	// Must be last because they reset the render pass' state
	// How do we know that buffers will not have been re-bound since this bundle was created? 
	Bundle(Arc<wgpu::RenderBundle>),
}

enum InstanceBuffer {
	// Instance buffer start, for rebinding when shader changes 
	// This data is duplicated but does not affect total size becuase Self::Indirect is larger 
	Main(u32), 
	Indirect(BufferKey),
}

// The part that is sorted 
enum RenderBufferItem {
	// Do not deduplicate multiples, but have a warning 
	Clear(RenderClear), 
	Compute(ShaderKey, [Option<BindGroupKey>; 4], [u32; 3]),
	// Draw and Indirect both map to this
	Polygon(ShaderKey, [Option<BindGroupKey>; 4], RenderMesh, InstanceBuffer),
	// Must be last because they reset the render pass' state
	Bundle(Arc<wgpu::RenderBundle>),
}
impl RenderBufferItem {
	#[inline]
	fn disc(s: &Self) -> u32 {
		match s {
			&Self::Clear(_) => 0,
			&Self::Compute(_, _, _) => 1,
			&Self::Polygon(_, _, _, _) => 2,
			&Self::Bundle(_) => 3,
		}
	}
}
impl std::cmp::PartialEq for RenderBufferItem {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}
impl std::cmp::Eq for RenderBufferItem {}
impl std::cmp::PartialOrd for RenderBufferItem {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}
impl std::cmp::Ord for RenderBufferItem {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		match (self, other) {
			(Self::Compute(s0, bgc0, _), Self::Compute(s1, bgc1, _)) => (s0, bgc0).cmp(&(s1, bgc1)),
			(
				Self::Polygon(s0, bgc0, m0, _), 
				Self::Polygon(s1, bgc1, m1, _),
			) => (s0, bgc0, m0).cmp(&(s1, bgc1, m1)),
			_ => Self::disc(self).cmp(&Self::disc(other)),
		}
	}
}

enum StageItem {
	// Clears appear at the beginning of a stage 
	Clear(NonZeroU32),
	Target(TargetKey),
}


pub struct RenderInput2 {
	// Stage name, depends_on, order in queue (max for not in queue) 
	stages: Vec<(String, Vec<StageKey>, u8)>,
	// Targets[0] is a dummy target for compute shaders 
	// Indices into this should start at 1 
	targets: Vec<AbstractRenderTarget>,

	items: Vec<(StageKey, TargetKey, MaterialKey, Option<MeshKey>, Entity)>,
	// A count of how many items have been inserted since the last sort
	// If it is big, then we do some heap allocation and a faster sort 
	// n_inserted: usize,
	
	// Could strip generation from keys? Map keys with secondary map 
	// Can we reduce the size somehow? Is that needed? 
	items_buffer: Vec<(StageKey, TargetKey, RenderBufferItem)>,
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

	pub fn build(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		shaders: &ShaderManager,
		bind_groups: &BindGroupManager,
		world: &World,
	) {
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
			for (stage, target, material, mesh, entity) in self.items.iter().copied() {
				self.items_buffer.push(todo!());
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
			// Allocate additional vecs with indices, sort with std, re-order base
			// We are unable to not remap the base because instance data pulling 
			// relies on it following the same order as the buffer items 
		}
		
		{
			profiling::scope!("fetch instance data");
			// Pull instance data 
			// TODO: precalculate total instance data size
			// - create buffer ahead of time to parallelize fetch and commands 
			self.instance_bytes.clear();
			let mut cur_st = 0;
			let mut cur_shader = None;
			let mut storages = ArrayVec::<_, 8>::new();
			let mut cur_size = 0; 
			let mut cur_count = 0;
			for (i, (_, _, item)) in self.items_buffer.iter_mut().enumerate() {
				match item {
					RenderBufferItem::Polygon(shader, _, _, InstanceBuffer::Main(o)) => {
						if Some(*shader) != cur_shader {
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
									let entity = self.items[i].4;
									storage.render_extend(entity, &mut self.instance_bytes);
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

		// Bind all meshes here

		

		// Parallel 

		// Can find draw call count of sequence 
		// Can find target count of sequence 
		// Can combine into split heuristic 

		// We can only split once without a lot of reallocation 
		// Cannot split stages becuase of clears? No we can, it just means we need extra passes 
		// Split with heuristic 
		// I want to not do this yet so just have an execution function that makes a command buffer 
		// fn par_stage_exec(slice: &[bool]) -> Vec<wgpu::CommandBuffer> {
		// 	// Find this partition 
		// 	// While below capacity
		// 	// Find size of next partition 
		// 	let (buff, mut buffs) = (excute(your_bit), par_exec(other_bit));
		// 	buffs.push_front(buff);
		// 	buffs
		// }
		

	}
}

enum InstanceBufferState {
	None,
	Main(u32),
	Indirect(BufferKey),
}
enum PassState<'a> {
	None,
	Compute {
		shader: Option<ShaderKey>,
		bgc: [Option<BindGroupKey>; 4],
	},
	Render {
		target: Option<TargetKey>,
		pass: Option<wgpu::RenderPass<'a>>,
		shader: Option<ShaderKey>,
		bgc: [Option<BindGroupKey>; 4],
		mesh: Option<(MeshKey, u32, u32)>,
		instance: InstanceBufferState,
	},
}
struct StageStateMachine {
	// Pass state 
	// Clears 
	// encoder 
}


/// Executes a sequence of commands. 
/// This is as it is so that we can execute arbitrary sequences of commands. 
/// We can split by some heuristic and parallelize encoding. 
fn execute_sequence(
	sequence: &[bool],
	device: &wgpu::Device,
) -> wgpu::CommandBuffer {
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
		label: Some("sequence encoder"),
	});

	let mut stage = None;
	let mut pass_state = PassState::None;

	// Stage, op[
		// clear(t, c),
		// targeted[
			// draw(s, bgc, ?mesh, [main(range), indirect(buffer)]), 
			// bundle(bundle), 
			// ], 
		// compute(s, bgc),
		// ]

	// If stage change  
		// Swap clears partition 
		// Look ahead to find passes 
		// Do passes for any that are not there 
	// If target change 
		// Look through clears partition for this 
			// What if there are multiple passes with that attachment?? Fuck!! 
		// Begin render pass 
	

	let mut i = 0;
	while i < self.items_buffer.len() {
		let (stage, target, item) = self.items_buffer[i];

		if Some(target) != cur_target {
			cur_target = Some(target);

			// Skip ahead to find clears 
			// Extract from target and then do this please
			let mut color_attachments = ArrayVec::<_, 8>::new();
			let mut depth_stencil_attachment = None;
			// while i < self.items_buffer.len() {
			// 	let (_, t, item) = self.items_buffer[i];
			// 	if t != target { break }
			// 	match item {
			// 		RenderBufferItem::Clear(c) => match c {
			// 			RenderClear::Colour([r, g, b, a]) => 
			// 		}
			// 		_ => break,
			// 	}
			// 	i += 1;
			// }

			trace!("Begin render pass for target {}", target);
			// ^^ info about contents and clears? 
			pass = Some(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None, 
				color_attachments: color_attachments.as_slice(),
				depth_stencil_attachment,
				timestamp_writes: None,
				occlusion_query_set: None,
			}));
		}

		let (stage, target, item) = self.items_buffer[i];
		match item {
			RenderBufferItem::Clear(_) => {},
			RenderBufferItem::Bundle(b) => {
				let g = pass.as_ref().expect("pass has not begun?");
				g.execute_bundles([b].iter().map(|b| b.as_ref()));
				cur_bgc = [None; 4];
			},
			RenderBufferItem::Compute(shader, bgc, [x, y, z]) => {
				todo!("Compute passes");
			},
			RenderBufferItem::Polygon(shader, bgc, mesh, buffer) => {
				let pass = pass.as_ref().unwrap();
				if cur_shader.is_none() || cur_shader.unwrap() != shader {
					let shader = shaders.get(shader).unwrap();
					trace!("Set shader to {:?}", shader.specification.name);
					pass.set_pipeline(shader.pipeline.as_ref().unwrap().polygon().unwrap());
				}
				for i in 0..cur_bgc.len() {
					if let Some(bg) = bgc[i] {
						if cur_bgc[i].is_none() || cur_bgc[i].unwrap() != bg {
							cur_bgc[i] = Some(bg);
							let entry = bind_groups.get(bg).unwrap();
							let bind_group = entry.binding.as_ref().unwrap();
							pass.set_bind_group(i as u32, bind_group, &[]);
						}
					}
				}
				let (indexed, n) = match mesh {
					RenderMesh::Static => todo!(),
					RenderMesh::Mesh(k, st, en) => {
						if cur_mesh.is_none() || cur_mesh.unwrap() != k {
							cur_mesh = Some(k)
						}
						(true, 32)
					},
				};

				let mut instance_count = 0;
				// Skip ahead to find those with same params (indluding mesh range!)

				let indirect = match buffer {
					InstanceBuffer::Main(g) => {
						false
					},
					InstanceBuffer::Indirect(k) => {
						true
					},
				};

				// match (indexed, indirect) {
				// 	(false, false) => 
				// }
				if !indirect {
					// Draw
					if !indexed {
						pass.draw(vertices, instances);
					} else {
						pass.draw_indexed(indices, base_vertex, instances);
					}
					// Add to count  
				} else {
					if !indexed {
						pass.draw_indirect(indirect_buffer, indirect_offset);
					} else {
						pass.draw_indexed_indirect(indirect_buffer, indirect_offset)
					}
				}

				if indexed {
					if indirect {
						pass.draw_indexed_indirect(indirect_buffer, indirect_offset);
					} else {
						pass.draw_indexed(indices, base_vertex, instances);
					}
				} else {
					if indirect
				}
				
				
			}
		}


		
		i += 1;
	}

	if self.items_buffer.len() > 0 {
		let (
			_, 
			mut cur_target, 
			mut cur_shader, 
		) = self.items_buffer[0];

		// Target, pass with that target 

		let mut instance_count = 0;
		for (_, target, item) in self.items_buffer {
			// When to do remaining clears? How do we know what to clear? 
			if target != cur_target {
				cur_target = target;
				// Swap target
				// Apply clears if any in stage 
				// This makes a render pass, will lifetimes be an issue? 
			}
			if shader == cur_shader && bgc == cur_bgc {
				// Add to count 
				instance_count += 1;
			} else {
				// Do a draw of counted bits 
				
				instance_count = 1;
				if shader != cur_shader {
					// swap shader 
				}
				for i in 0..bgc.len() {
					if bgc[i] != cur_bgc[i] {
						// Rebing group for i
					}
				}
			}

			
		}
	}

	encoder.finish()
}





/// Finds partitions of different keys and returns their slices. 
struct PartitionIterator<'a, T, K> {
	f: Box<dyn Fn(&T) -> K>,
	slice: &'a [T],
	idx: usize,
}
impl<'a, T, K> PartitionIterator<'a, T, K> {
	pub fn new<F: Fn(&T) -> K + 'static>(slice: &'a [T], f: F) -> Self {
		Self { f: Box::new(f), slice, idx: 0, }
	}
}
impl<'a, T, K: Eq> Iterator for PartitionIterator<'a, T, K> {
	type Item = (K, &'a [T]);
	fn next(&mut self) -> Option<Self::Item> {
		if self.idx == self.slice.len() {
			None
		} else {
			let st = self.idx;
			let k = (self.f)(&self.slice[st]);
			let en = self.slice[st..].iter().position(|v| (self.f)(v) != k)?;
			self.idx = en;

			Some((k, &self.slice[st..en]))
		}
	}
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

	pub fn key(self) -> StageKey {
		self.stage
	}
}

pub struct TargetQueue<'a> {
	stage_builder: StageBuilder<'a>,
	target: TargetKey,
}
impl<'a> TargetQueue<'a> {
	pub fn add_pass(self, material: MaterialKey, entity: Entity) -> Self {
		let stage = self.stage_builder.stage;
		let target = self.target;
		self.stage_builder.input.items.push((stage, target, material, None, entity));
		self
	}

	pub fn add_model(self, material: MaterialKey, mesh: MeshKey, entity: Entity) -> Self {
		self.stage_builder.input.items.push((self.stage_builder.stage, self.target, material, Some(mesh), entity));
		self
	}

	// pub fn add_batch(self, material, mesh, buffer, count)

	pub fn finish(self) -> StageBuilder<'a> {
		self.stage_builder
	}
}
