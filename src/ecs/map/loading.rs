use std::{collections::HashMap, sync::Arc, time::{Instant, Duration}};
use crossbeam_channel::{Sender, Receiver, unbounded};
use eks::prelude::*;
use glam::{IVec3, UVec3};
use crate::{ecs::*, voxel::{VoxelModification, chunk_of_point, Chunk, NewTerrainGenerator, chunk::CHUNK_SIZE, VoxelCube}, util::RingDataHolder};
use super::ChunkEntry;



#[derive(Debug, ResourceIdent)]
pub struct ChunkLoadingResource {
	pub chunk_sender: Sender<(IVec3, Chunk, Vec<VoxelModification>)>,
	pub chunk_receiver: Receiver<(IVec3, Chunk, Vec<VoxelModification>)>,
	pub max_generation_jobs: u8,
	pub cur_generation_jobs: u8,
	pub vec_generation_jobs: Vec<(IVec3, Instant)>, // For profiling
	pub generation_durations: RingDataHolder<Duration>, // For profiling

	// Potential optimization:
	// Loading volumes are stored here so that we can test whether or not we
	// need to recalculate the loaded areas
	// Would cause spikes in frame times every time a chunk boundary is crossed
	// pub previous_loading_volumes: HashMap<Entity, VoxelCube>,

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

	pub fn loading_volume(&self, transform: TransformComponent) -> VoxelCube {
		VoxelCube::new(chunk_of_point(transform.translation), UVec3::splat(self.radius as u32))
	}

	// Volume but expanded by tolerence
	pub fn un_loading_volume(&self, transform: TransformComponent) -> VoxelCube {
		VoxelCube::new(chunk_of_point(transform.translation), UVec3::splat((self.radius + self.tolerence) as u32))
	}
}


#[profiling::function]
pub fn map_loading_system(
	blocks: Res<BlockResource>,
	map: ResMut<MapResource>,
	mut loading: ResMut<ChunkLoadingResource>,
	map_loaders: Comp<ChunkLoadingComponent>,
	transforms: Comp<TransformComponent>,
) { 
	info!("Chunk loading system");

	let loading_volumes = (&map_loaders, &transforms).iter()
		.map(|(loader, transform)| loader.loading_volume(*transform))
		.collect::<Vec<_>>();

	let un_loading_volumes = (&map_loaders, &transforms).iter()
		.map(|(loader, transform)| loader.un_loading_volume(*transform))
		.collect::<Vec<_>>();

	let chunks = map.chunks.read();

	// Note: This is not inexpensive!
	// In a debug build, hashmap lookups take ~278ns, 19^3 lookups in ~1.6ms
	// In a release build, hashmap lookups take ~19ns, 19^3 lookups in ~0.1ms
	// FxHash is much faster
	// In a debug build, FxHashMap lookups take ~147ns, 19^3 lookups in ~1.0ms
	// In a release build, hashmap lookups take ~4ns, 19^3 lookups in ~0.03ms
	// This data was collected using benchmarks and is reflected by profiling
	let chunks_to_load = {
		profiling::scope!("Collect chunks to load");
		loading_volumes.iter().map(|lv| lv.iter())
			.flatten()
			.filter(|position| !chunks.positions.contains_key(position)).collect::<Vec<_>>()
	};

	let chunks_to_prune = {
		profiling::scope!("Collect chunks to prune");
		chunks.positions.keys().copied()
			.filter(|&p| !un_loading_volumes.iter()
				.any(|lv| lv.contains(p)))
			.collect::<Vec<_>>()
	};

	drop(chunks);
	let mut chunks = map.chunks.write();

	// Todo: save data
	{ // Prune chunks that should not be loaded
		profiling::scope!("Prune chunks");
		debug!("Prune {} chunks", chunks_to_prune.len());
		for position in chunks_to_prune {
			if let Some((_, c)) = chunks.remove(&position) {
				trace!("Unloading chunk {position}");
				match c {
					ChunkEntry::UnLoaded => {},
					ChunkEntry::Loading | ChunkEntry::Generating => warn!("Unloading a generating/loading chunk for {position}"),
					ChunkEntry::Complete(_) => warn!("Dropping chunk data for {position}"),
				}
			}
		}
	}

	{ // Receive new chunks
		profiling::scope!("Receive new chunks");
		while let Ok((position, chunk, modifications)) = loading.chunk_receiver.try_recv() {
			trace!("Received generated chunk for {position}");
			let i = loading.vec_generation_jobs.iter().position(|&(v, _)| v == position)
				.expect("We don't seem to have queued this for generation");
			let (_, t_start) = loading.vec_generation_jobs.remove(i);
			loading.generation_durations.insert(t_start.elapsed());
	
			chunks.insert(position, ChunkEntry::Complete(Arc::new(chunk)));
			map.modify_voxels(modifications.as_slice());
			loading.cur_generation_jobs -= 1;
		}
	}

	{ // Insert entries for chunks that should be in the HM but are not
		profiling::scope!("Insert chunk entries");
		debug!("Insert {} entries", chunks_to_load.len());
		for position in chunks_to_load {
			trace!("Mark chunk {position} existence");
			chunks.insert(position, ChunkEntry::UnLoaded);
		}
	}

	// Take some generation jobs
	// Find potential jobs, sort by closest distance to a viewer
	// A bit hackey but it works
	let mut potential_jobs = {
		profiling::scope!("Find potential jobs");
		chunks.entries.iter_mut()
		.filter(|(_, (_, entry))| if let ChunkEntry::UnLoaded = entry {true} else {false})
		.map(|(_, (position, entry))|{
			let distance = (&map_loaders, &transforms).iter()
				.map(|(_, t)| (*position * CHUNK_SIZE as i32).as_vec3().distance_squared(t.translation))
				.min_by(|a, b| a.total_cmp(b))
				.unwrap();
			(*position, entry, distance)
		})
		.collect::<Vec<_>>()
	};
	{
		profiling::scope!("Sort potential jobs");
		potential_jobs.sort_unstable_by(|a, b| a.2.total_cmp(&b.2));
	}
	
	{
		profiling::scope!("Start new jobs");
		for (position, entry, p) in potential_jobs {
			if loading.cur_generation_jobs >= loading.max_generation_jobs {
				trace!("Reached maxium chunk generation jobs");
				break;
			}
			if let ChunkEntry::UnLoaded = entry {
				// todo!("Generate chunk")
				trace!("Begin generating chunk {position} (priority {p})");
	
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

				*entry = ChunkEntry::Loading;
				loading.vec_generation_jobs.push((position, Instant::now()));
				loading.cur_generation_jobs += 1;
			}
		}
	}
}
