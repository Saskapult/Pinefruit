use std::{sync::Arc, collections::HashMap, time::{Instant, Duration}};
use crossbeam_channel::{Sender, Receiver, unbounded};
use eks::prelude::*;
use glam::IVec3;
use parking_lot::RwLock;
use slotmap::SecondaryMap;
use crate::{voxel::{ArrayVolume, BlockKey, VoxelModification, chunk_of_voxel, voxel_relative_to_chunk, CHUNK_SIZE, NewTerrainGenerator, Chunk}, util::RingDataHolder, ecs::TransformComponent};
use super::{chunks::{ChunkKey, ChunksResource, ChunkLoadingComponent}, BlockResource};



// An entry in the mesh storage for a map component
#[derive(Debug, Clone, variantly::Variantly)]
pub enum TerrainEntry {
	UnLoaded,	// Used if chunk does not exist yet
	Loading,	// Waiting for disk data done
	Generating,	// Waiting for generation done
	Complete(Arc<Chunk<BlockKey>>),
}


#[derive(Debug, Default, Resource)]
pub struct TerrainResource {
	pub chunks: Arc<RwLock<SecondaryMap<ChunkKey, TerrainEntry>>>,
	pub block_mods: RwLock<HashMap<IVec3, Vec<VoxelModification>>>,
}
impl TerrainResource {

	pub fn get_voxel(&self, cr: &ChunksResource, voxel: IVec3) -> Option<BlockKey> {
		let chunk = chunk_of_voxel(voxel);
		let voxel = voxel_relative_to_chunk(voxel, chunk).as_uvec3();

		let chunks = self.chunks.read();
		let terrain_chunk = cr.read().get_position(chunk)
			.and_then(|chunk_key| chunks.get(chunk_key))
			.and_then(|entry| entry.complete_ref())
			.cloned();

		terrain_chunk.and_then(|tc| tc.contents.get(voxel).copied())
	}

	pub fn modify_voxel(&self, modification: VoxelModification) {
		let (c, r) = modification.as_chunk_relative();
		self.block_mods.write().entry(c).or_default().push(r);
	}

	pub fn modify_voxels(&self, modifications: &[VoxelModification]) {
		let mut block_mods = self.block_mods.write();
		
		for modification in modifications {
			let (c, r) = modification.as_chunk_relative();
			block_mods.entry(c).or_default().push(r);
		}
	}

	/// Gets some estimate of the map's data usage. 
	/// Assumes that chunks are stored with array volumes with no optimization
	pub fn approximate_size(&self) -> u64 {
		// Currently chunks store Option<BlockKey>, which is 64 bits in size
		(self.chunks.read().len() as u64) * 8 * CHUNK_SIZE as u64
	}
}


#[derive(Debug, Resource)]
pub struct TerrainLoadingResource {
	pub chunk_sender: Sender<(IVec3, ArrayVolume<BlockKey>, Vec<VoxelModification>)>,
	pub chunk_receiver: Receiver<(IVec3, ArrayVolume<BlockKey>, Vec<VoxelModification>)>,
	pub max_generation_jobs: u8,
	pub cur_generation_jobs: u8,
	pub vec_generation_jobs: Vec<(IVec3, Instant)>, // For profiling
	pub generation_durations: RingDataHolder<Duration>, // For profiling

	pub seed: u32,
	pub pending_blockmods: HashMap<IVec3, Vec<VoxelModification>>,
	pub generator: Arc<NewTerrainGenerator>,
}
impl TerrainLoadingResource {
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


#[profiling::function]
pub fn terrain_loading_system(
	blocks: Res<BlockResource>,
	chunks: Res<ChunksResource>,
	terrain: ResMut<TerrainResource>,
	mut loading: ResMut<TerrainLoadingResource>,
	loaders: Comp<ChunkLoadingComponent>, 
	transforms: Comp<TransformComponent>, 
) { 
	let mut terrain_chunks = terrain.chunks.write();
	let chunks = chunks.read();
	
	// Prune chunks that should not be loaded
	// Todo: save data
	{ 
		profiling::scope!("Prune chunks");
		terrain_chunks.retain(|k, _| chunks.chunks.contains_key(k));
	}

	{ // Receive new chunks
		profiling::scope!("Receive new chunks");
		while let Ok((position, chunk, modifications)) = loading.chunk_receiver.try_recv() {
			trace!("Received generated chunk for {position}");
			let i = loading.vec_generation_jobs.iter().position(|&(v, _)| v == position)
				.expect("We don't seem to have queued this for generation");
			let (_, t_start) = loading.vec_generation_jobs.remove(i);
			loading.generation_durations.insert(t_start.elapsed());

			if let Some(k) = chunks.get_position(position) {
				terrain_chunks.insert(k, TerrainEntry::Complete(Arc::new(Chunk::new_with_contents(chunk))));
				terrain.modify_voxels(modifications.as_slice());
				loading.cur_generation_jobs -= 1;
			} else {
				warn!("Received chunk but not meant to be loaded");
			}
		}
	}

	{ // Insert entries for chunks that should be in the HM but are not
		profiling::scope!("Insert chunk entries");
		// debug!("Insert {} entries", chunks_to_load.len());
		for (key, &position) in chunks.chunks.iter() {
			if !terrain_chunks.contains_key(key) {
				trace!("Mark terrain chunk {position} existence");
				terrain_chunks.insert(key, TerrainEntry::UnLoaded);
			}
		}
	}

	// Take some generation jobs
	// Find potential jobs, sort by closest distance to a viewer
	// A bit hackey but it works
	let mut potential_jobs = {
		profiling::scope!("Find potential jobs");
		terrain_chunks.iter_mut()
		.filter(|(_, entry)| if let TerrainEntry::UnLoaded = entry {true} else {false})
		.map(|(k, entry)|{
			let position = chunks.chunks.get(k).expect("Chunk not meant to be loaded, please ignore in future");
			let distance = (&loaders, &transforms).iter()
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
			if let TerrainEntry::UnLoaded = entry {
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
	
					generator.base(position, &mut c.contents, stone);
					generator.cover(position, &mut c.contents, grass, dirt, 3);
	
					sender.send((position, c.contents, Vec::new())).unwrap();
				});

				*entry = TerrainEntry::Loading;
				loading.vec_generation_jobs.push((position, Instant::now()));
				loading.cur_generation_jobs += 1;
			}
		}
	}
}
