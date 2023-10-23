use std::{collections::{HashMap, HashSet}, sync::Arc, time::{Instant, Duration}};
use crossbeam_channel::{Sender, Receiver, unbounded};
use eks::prelude::*;
use glam::IVec3;
use crate::{ecs::*, voxel::{VoxelModification, TerrainGenerator, chunk_of_point, VoxelSphere, Chunk, NewTerrainGenerator, chunk::CHUNK_SIZE}, util::RingDataHolder};
use super::ChunkEntry;



#[derive(Debug, ResourceIdent)]
pub struct ChunkLoadingResource {
	pub chunk_sender: Sender<(IVec3, Chunk, Vec<VoxelModification>)>,
	pub chunk_receiver: Receiver<(IVec3, Chunk, Vec<VoxelModification>)>,
	pub max_generation_jobs: u8,
	pub cur_generation_jobs: u8,
	pub vec_generation_jobs: Vec<(IVec3, Instant)>, // For profiling
	pub generation_durations: RingDataHolder<Duration>, // For profiling

	pub seed: u32,
	pub pending_blockmods: HashMap<IVec3, Vec<VoxelModification>>,
	pub generator: Arc<NewTerrainGenerator>,
}
impl ChunkLoadingResource {
	pub fn new(seed: u32) -> Self {
		let (chunk_sender, chunk_receiver) = unbounded();
		Self {
			chunk_sender, chunk_receiver, 
			max_generation_jobs: 16,
			cur_generation_jobs: 0,
			vec_generation_jobs: Vec::with_capacity(8),
			generation_durations: RingDataHolder::new(32),
			seed, 
			pending_blockmods: HashMap::new(),
			generator: Arc::new(NewTerrainGenerator::new(seed as i32)),
		}
	}
}


#[derive(Debug, ComponentIdent)]
pub struct ChunkLoadingComponent {
	pub radius: i32,
	pub tolerence: i32,
}
impl ChunkLoadingComponent {
	pub fn new(radius: i32) -> Self { 
		assert!(radius > 0);
		Self { radius, tolerence: 2, }
	}
}


pub fn map_loading_system(
	blocks: Res<BlockResource>,
	map: ResMut<MapResource>,
	mut loading: ResMut<ChunkLoadingResource>,
	map_loaders: Comp<ChunkLoadingComponent>,
	transforms: Comp<TransformComponent>,
) { 
	info!("Chunk loading system");

	// Collect every chunk that should be loaded
	let mut chunks_to_load = HashSet::new();
	for (loader, transform) in (&map_loaders, &transforms).iter() {
		let loader_chunk = chunk_of_point(transform.translation);
		for cpos in VoxelSphere::new(loader_chunk, loader.radius).iter() {
			chunks_to_load.insert(cpos);
		}			
	}

	// Collect every chunk that should be unloaded
	let mut chunks_to_unload = Vec::new();
	for chunk_position in map.chunks.read().keys().copied() {
		// Remove iff not in any of the loading spheres
		let should_keep = (&map_loaders, &transforms).iter().any(|(loader, transform)| {
			let loader_chunk = chunk_of_point(transform.translation);
			VoxelSphere::new(loader_chunk, loader.radius+loader.tolerence).is_within(chunk_position)
		});
		if !should_keep {
			chunks_to_unload.push(chunk_position)
		}
	}

	// Begin the part where we edit things
	let mut chunks = map.chunks.write();

	// Remove old chunks
	// Todo: save data
	for position in chunks_to_unload {
		if let Some(c) = chunks.remove(&position) {
			debug!("Unloading chunk {position}");
			match c {
				ChunkEntry::UnLoaded => {},
				ChunkEntry::Loading | ChunkEntry::Generating => warn!("Unloading a generating/loading chunk for {position}"),
				ChunkEntry::Complete(_) => warn!("Dropping chunk data for {position}"),
			}
		}
	}

	// Receive new chunks
	while let Ok((position, chunk, modifications)) = loading.chunk_receiver.try_recv() {
		debug!("Received generated chunk for {position}");
		let i = loading.vec_generation_jobs.iter().position(|&(v, _)| v == position)
			.expect("We don't seem to have queued this for generation");
		let (_, t_start) = loading.vec_generation_jobs.remove(i);
		loading.generation_durations.insert(t_start.elapsed());

		chunks.insert(position, ChunkEntry::Complete(Arc::new(chunk)));
		map.modify_voxels(modifications.as_slice());
		loading.cur_generation_jobs -= 1;
	}

	// Mark chunks for loading
	for position in chunks_to_load {
		if chunks.get(&position).is_none() {
			debug!("Chunk {position} must be generated");
			chunks.insert(position, ChunkEntry::UnLoaded);
		}
	}

	// Take some generation jobs
	// Find potential jobs, sort by closest distance to a viewer
	// A bit hackey but it works
	let mut potential_jobs = chunks.iter_mut()
		.filter(|(_, entry)| if let ChunkEntry::UnLoaded = entry {true} else {false})
		.map(|(&position, entry)|{
			let distance = (&map_loaders, &transforms).iter()
				.map(|(_, t)| (position * CHUNK_SIZE as i32).as_vec3().distance_squared(t.translation))
				.min_by(|a, b| a.total_cmp(b))
				.unwrap();
			(position, entry, distance)
		})
		.collect::<Vec<_>>();
	potential_jobs.sort_unstable_by(|a, b| a.2.total_cmp(&b.2));
	
	for (position, entry, p) in potential_jobs {
		if loading.cur_generation_jobs >= loading.max_generation_jobs {
			debug!("Reached maxium chunk generation jobs");
			break;
		}
		if let ChunkEntry::UnLoaded = entry {
			// todo!("Generate chunk")
			debug!("Begin generating chunk {position} (priority {p})");

			let blocks = blocks.read();
			let grass = blocks.key_by_name(&"grass".into()).unwrap();
			let dirt = blocks.key_by_name(&"dirt".into()).unwrap();
			let stone = blocks.key_by_name(&"stone".into()).unwrap();

			let generator = loading.generator.clone();
			let sender = loading.chunk_sender.clone();
			rayon::spawn(move || {
				let mut c = Chunk::new();

				// let tgen = TerrainGenerator::new(0);
				// tgen.chunk_base_3d(position, &mut c, stone);
				// tgen.cover_chunk(&mut c, position, grass, dirt, 3);

				generator.base(position, &mut c.storage, stone);
				generator.cover(position, &mut c.storage, grass, dirt, 3);

				sender.send((position, c, Vec::new())).unwrap();
			});
			loading.vec_generation_jobs.push((position, Instant::now()));
			loading.cur_generation_jobs += 1;
		}
	}
}
