use std::collections::{HashMap, HashSet};
use bytemuck::{Pod, Zeroable};
use eks::prelude::*;
use glam::{IVec3, UVec3};
use krender::{BufferKey, prelude::{BufferManager, Buffer, RenderContext, RenderInput}, MaterialKey, allocator::{SlabBufferAllocator, SlabAllocationKey, BufferAllocator}};
use oktree::Octree;
use slotmap::Key;

use crate::{util::KGeneration, game::{BufferResource, QueueResource, MaterialResource}, ecs::{TransformComponent, ChunkEntry}, voxel::{VoxelSphere, chunk_of_point, Chunk, chunk::CHUNK_SIZE}};

use super::{MapResource, BlockResource};


#[derive(Debug, ResourceIdent)]
pub struct BigBufferResource {
	// The buffer need not be only for chunk data, so we could store
	// these in another shared resource
	pub data_buffer: BufferKey,
	pub data_allocator: SlabBufferAllocator,
}
impl BigBufferResource {
	// 1GiB is 1073741824B
	// 1MiB is 1048576B
	// const BUFFER_SIZE: u64 = 1048576 * 64; // 64MiB
	const BUFFER_SIZE: u64 = 1048576 * 16;
	const SLAB_SIZE: u32 = 256; // 64 bytes per slab

	pub fn new(
		buffers: &mut BufferManager,
	) -> Self {
		let data_buffer = buffers.insert(Buffer::new(
			"big buffer",  
			Self::BUFFER_SIZE, 
			false, 
			true,
			true,
		).with_usages(wgpu::BufferUsages::STORAGE));

		let slab_count = Self::BUFFER_SIZE / Self::SLAB_SIZE as u64;
		let data_allocator = SlabBufferAllocator::new(
			Self::SLAB_SIZE, 
			slab_count as u32, 
			true,
		);

		Self { data_buffer, data_allocator, }
	}

	// Doesn't need to be a method, it just makes the code more clean
	pub fn insert(
		&mut self, 
		queue: &wgpu::Queue,
		buffers: &mut BufferManager,
		data: &[u8],
	) -> SlabAllocationKey {
		let allocation = self.data_allocator.alloc(data.len() as u64)
			.expect("Big buffer is too full!");

		// Use queued write to avoid more cloning
		let b = buffers.get_mut(self.data_buffer).unwrap();
		b.write(queue, allocation.start, data);

		allocation
	}

	/// Writes into an allocated space, will panic if bounds are exceeded
	pub fn write(
		&mut self, 
		queue: &wgpu::Queue,
		buffers: &mut BufferManager,
		allocation: SlabAllocationKey,
		offset: u64,
		data: &[u8],
	) {
		if allocation.start + offset + data.len() as u64 >= allocation.end {
			panic!("Want to write to bad place!")
		}
		
		debug!("Big buffer writes {} bytes at index {}", data.len(), allocation.start + offset);

		// Use queued write to avoid more cloning
		let b = buffers.get_mut(self.data_buffer).unwrap();
		b.write(queue, allocation.start + offset, data);
	}
}
impl std::ops::Deref for BigBufferResource {
	type Target = SlabBufferAllocator;
	fn deref(&self) -> &Self::Target {
		&self.data_allocator
	}
}
impl std::ops::DerefMut for BigBufferResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data_allocator
	}
}


#[derive(Debug, ResourceIdent, Default)]
pub struct GPUChunksResource {
	pub chunks: HashMap<IVec3, Option<(KGeneration, SlabAllocationKey)>>,

	pub material: Option<MaterialKey>,
}
impl GPUChunksResource {
	/// How much space is this using? 
	pub fn used_bytes(&self) -> u64 {
		self.chunks.values()
			.copied()
			.filter_map(|o| o)
			.map(|(_, k)| k.size())
			.reduce(|a, v| a + v)
			.unwrap_or(0)
	}
}


/// The chunks around this entity will be loaded into GPU memory as octrees
#[derive(Debug, ComponentIdent)]
pub struct GPUChunkLoadingComponent {
	pub radius: i32,
	pub tolerence: i32,
}
impl GPUChunkLoadingComponent {
	pub fn new(radius: i32, tolerence: i32) -> Self {
		assert!(radius > 0);
		assert!(tolerence > 0);
		Self {
			radius, tolerence, 
		}
	}
}


#[profiling::function]
pub fn gpu_chunk_loading_system(
	queue: Res<QueueResource>,
	map: Res<MapResource>,
	mut buffers: ResMut<BufferResource>,
	mut bigbuffer: ResMut<BigBufferResource>,
	mut octrees: ResMut<GPUChunksResource>,
	transforms: Comp<TransformComponent>,
	loaders: Comp<GPUChunkLoadingComponent>,
) {
	info!("Octree loading system");
	
	// Collect every chunk that should be loaded
	let mut chunks_to_load = HashSet::new();
	for (loader, transform) in (&loaders, &transforms).iter() {
		let loader_chunk = chunk_of_point(transform.translation);
		for cpos in VoxelSphere::new(loader_chunk, loader.radius).iter() {
			chunks_to_load.insert(cpos);
		}			
	}

	// Collect every chunk that should be unloaded
	let mut chunks_to_unload = Vec::new();
	for chunk_position in octrees.chunks.keys().copied() {
		// Remove iff not in any of the loading spheres
		let should_keep = (&loaders, &transforms).iter().any(|(loader, transform)| {
			let loader_chunk = chunk_of_point(transform.translation);
			VoxelSphere::new(loader_chunk, loader.radius+loader.tolerence).is_within(chunk_position)
		});
		if !should_keep {
			chunks_to_unload.push(chunk_position)
		}
	}

	// Remove old stuff
	for position in chunks_to_unload {
		if let Some(Some((_, allocation))) = octrees.chunks.remove(&position) {
			debug!("Unloading chunk {position}");
			bigbuffer.free(allocation);
		}
	}

	// Mark stuff
	for position in chunks_to_load {
		if octrees.chunks.get(&position).is_none() {
			debug!("Chunk {position} must be loaded");
			octrees.chunks.insert(position, None);
		}
	}

	// Load unloaded things
	// Also check if current stuff is outdated
	let mut counter = 0;
	for (position, data) in octrees.chunks.iter_mut() {
		let chunks = map.chunks.read();
		if let Some(ChunkEntry::Complete(c)) = chunks.key(position).and_then(|k| chunks.get(k)) {
			if let Some((generation, _)) = data {
				// test for generation outdated, if not then "continue"
				if c.generation == *generation {
					continue
				}
				trace!("Chunk {position} is loaded but out of date");
			}

			debug!("Treeing chunk {position}");
			let tree = chunk_to_octree(c, CHUNK_SIZE);
			let tree_data = tree.data();

			let key = bigbuffer.insert(&queue, &mut buffers, bytemuck::cast_slice(tree_data.as_slice()));
			*data = Some((c.generation, key));

			counter += 1;
			if counter >= 3 {
				// We've loaded enough things during this tick
				break;
			}
		} else {
			warn!("Chunk {position} should be tree'd but isn't available in the map, skipping");
		}
	}
}


// Makes an octree with contents that match the chunk
fn chunk_to_octree(chunk: &Chunk, chunk_extent: u32) -> Octree {
	let depth = chunk_extent.ilog2();
	let mut tree = Octree::new(depth, 1);
	
	for x in 0..chunk_extent {
		for y in 0..chunk_extent {
			for z in 0..chunk_extent {
				let c = chunk.get(UVec3::new(x, y, z))
					.and_then(|v| {
						Some((v.data().as_ffi() & 0x00000000FFFFFFFF) as u32 - 1)
					});
				if let Some(c) = c {
					tree.insert(x, y, z, &c);
				}
			}
		}
	}

	// Make sure they're equal in content
	for x in 0..chunk_extent {
		for y in 0..chunk_extent {
			for z in 0..chunk_extent {
				let c_g = chunk.get(UVec3::new(x, y, z))
					.and_then(|v| {
						Some((v.data().as_ffi() & 0x00000000FFFFFFFF) as u32 - 1)
					});
				let t_g = tree.get(x, y, z)
					.and_then(|s| Some(s[0]));
				assert_eq!(c_g, t_g, "Contents differ!");
			}
		}
	}
	
	tree
}


/// Stick this on a context entity, get chunk acceleration structure info buffer. 
#[derive(Debug, ComponentIdent)]
pub struct GPUChunkViewer {
	pub radius: u32, // This is assumed to be constant (determines buffer size)
	pub info_buffer: Option<BufferKey>,	
	pub chunks_array: Option<SlabAllocationKey>, // in big buffer
}
impl GPUChunkViewer {
	pub fn new(radius: u32) -> Self {
		assert_ne!(0, radius, "Radius 0 breaks extent formula!");
		Self {
			radius,
			info_buffer: None,
			chunks_array: None,
		}
	}
	pub fn extent(&self) -> u32 {
		2 * self.radius - 1
	}
}


#[profiling::function]
pub fn chunk_rays_system(
	(
		context,
		input,
	): (
		&mut RenderContext<Entity>,
		&mut RenderInput<Entity>,
	),
	queue: Res<QueueResource>,
	mut buffers: ResMut<BufferResource>,
	mut bigbuffer: ResMut<BigBufferResource>,
	mut octrees: ResMut<GPUChunksResource>,
	mut viewers: CompMut<GPUChunkViewer>,
	transforms: Comp<TransformComponent>,
	mut materials: ResMut<MaterialResource>,
) {
	#[repr(C)]
	#[derive(Debug, Pod, Zeroable, Clone, Copy)]
	struct ViewInfo {
		pub extent: u32, // always odd
		pub chunks_st: u32, // points to [u32; extent^3]
	}

	if let Some(entity) = context.entity {
		if let Some(viewer) = viewers.get_mut(entity) {
			let transform = transforms.get(entity).unwrap();
			let centre_chunk = chunk_of_point(transform.translation);

			// Fetch chunk octree locations
			let mut locations = Vec::with_capacity(viewer.radius.pow(3) as usize);
			let half_extent = viewer.extent() as i32 / 2;
			for x in -half_extent..=half_extent {
				for y in -half_extent..=half_extent {
					for z in -half_extent..=half_extent {
						let position = centre_chunk + IVec3::new(x, y, z);
						// 0 if empty, else idx+1 (in tetrabytes)
						if let Some(Some((_, a))) = octrees.chunks.get(&position) {
							locations.push((a.start / 4) as u32 + 1);
						} else {
							locations.push(0);
						}
					}
				}
			}

			// Upload locations data to da big buffer
			let locations_data = bytemuck::cast_slice::<_, u8>(locations.as_slice());
			let locations_allocation = viewer.chunks_array
				.get_or_insert_with(|| bigbuffer.data_allocator.alloc(locations_data.len() as u64).unwrap())
				.clone();
			bigbuffer.write(&queue, &mut buffers, locations_allocation, 0, locations_data);

			let view_data = ViewInfo { 
				extent: viewer.extent(), 
				chunks_st: (locations_allocation.start / 4) as u32, 
			};

			if let Some(key) = viewer.info_buffer {
				// Write data
				let b = buffers.get_mut(key).unwrap();
				b.write(&queue, 0, bytemuck::bytes_of(&view_data));
			} else {
				debug!("Init buffer");
				let b = Buffer::new_init(
					"idk", 
					bytemuck::bytes_of(&view_data), 
					false,
					true,
					false,
				);
				let key = buffers.insert(b);
				viewer.info_buffer = Some(key);
				context.insert_buffer("idk", key);
			}

			
			let material = *octrees.material.get_or_insert_with(|| 
				materials.key_by_path("resources/materials/octree_chunks.ron")
				.unwrap_or_else(|| materials.read("resources/materials/octree_chunks.ron"))
			);

			input.insert_item("voxels", material, None, entity);
		}
	}
}


#[derive(Debug, ResourceIdent)]
pub struct BlockColoursResource {
	pub colours: BufferKey,
}


pub fn block_colours_system(
	blocks: Res<BlockResource>,
	mut buffers: ResMut<BufferResource>,
	mut block_colours: ResOptMut<BlockColoursResource>,
) {
	if block_colours.is_none() {
		info!("Initialize block colours buffer");
		let encoded_colours = blocks.read().blocks.values()
			.map(|b| b.colour())
			.map(|[r, g, b, a]| {
				let r = ((r * u8::MAX as f32).round() as u32) << 24;
				let g = ((g * u8::MAX as f32).round() as u32) << 16;
				let b = ((b * u8::MAX as f32).round() as u32) << 8;
				let a = ((a * u8::MAX as f32).round() as u32) << 0;
				r | g | b | a
			})
			.collect::<Vec<_>>();
		// We need the explicit usages because I haven't finished presistent buffer bindings
		let colours = buffers.insert(Buffer::new_init(
			"block colours buffer", 
			bytemuck::cast_slice(encoded_colours.as_slice()), 
			false, 
			false, 
			true,
		).with_usages(wgpu::BufferUsages::STORAGE));
		*block_colours = Some(BlockColoursResource {
			colours,
		});

		// println!("Block colours is now {encoded_colours:?} stored at {colours:?}");
		// std::thread::sleep(std::time::Duration::from_secs(5));
	}
}
