use std::{sync::Arc, collections::HashMap, time::Instant};
use chunks::{array_volume::ArrayVolume, blocks::{BlockKey, BlockResource}, chunk::Chunk, chunk_of_voxel, chunks::{ChunkKey, ChunksResource}, generation::KGeneration, voxel_relative_to_chunk, CHUNK_SIZE};
use crossbeam_channel::{Sender, Receiver, unbounded};
use eeks::prelude::*;
use glam::{IVec3, UVec3};
use parking_lot::RwLock;
use slotmap::SecondaryMap;
use crate::{generator::NewTerrainGenerator, modification::VoxelModification};



#[derive(Clone, Debug)]
pub struct TerrainContents {
	// Option<BlockKey> uses 8 bytes, but we can avoid storing more of it
	// By using pallettes 
	// TODO: that 
	contents: Option<Box<[Option<BlockKey>]>>,
	contents_count: usize,
}
impl TerrainContents {
	pub fn new() -> Self {
		Self { 
			contents: None, 
			contents_count: 0,
		}
	}

	pub fn is_empty(&self) -> bool {
		self.contents.is_none()
	}

	pub fn in_bounds(&self, position: UVec3) -> bool {
		position.cmplt(UVec3::splat(CHUNK_SIZE)).all()
	}

	fn index_of(&self, position: UVec3) -> usize {
		let [x, y, z] = position.to_array();
		(x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z) as usize
	}
	
	pub fn get(&self, position: UVec3) -> Option<BlockKey> {
		let i = self.index_of(position);
		self.contents.as_ref().and_then(|c| c[i])
	}
	
	pub fn insert(&mut self, position: UVec3, data: BlockKey) {
		let i = self.index_of(position);
		let c = self.contents.get_or_insert_with(|| vec![None; CHUNK_SIZE.pow(3) as usize].into_boxed_slice());
		c[i] = Some(data);
		self.contents_count += 1;
	}

	pub fn remove(&mut self, position: UVec3) {
		let i = self.index_of(position);
		if let Some(c) = self.contents.as_mut() {
			if c[i].take().is_some() {
				self.contents_count -= 1;
				// If no contents remain, deallocate
				if self.contents_count == 0 {
					self.contents.take();
				}
			}
		}
	}

	pub fn size(&self) -> usize {
		let mut base = std::mem::size_of::<Self>();
		if let Some(c) = self.contents.as_ref() {
			base += c.len() * std::mem::size_of::<Option<BlockKey>>();
		}
		base
	}

	// Tip: you can compress the result with lz4
	pub fn run_length_encode(&self) -> Vec<(Option<BlockKey>, u32)> {
		let mut runs = Vec::new();
		if let Some(c) = self.contents.as_ref() {
			let mut last = c[0].clone();
			let mut len = 1;
			for curr in c[1..].iter() {
				if last.eq(curr) {
					len += 1;
				} else {
					runs.push((last, len));
					last = curr.clone();
					len = 1;
				}
			}
			runs.push((last, len));
		} else {
			runs.push((None, CHUNK_SIZE.pow(3)));
		}
		
		runs
	}
	
	pub fn run_length_decode(rle: &Vec<(Option<BlockKey>, u32)>) -> Self {
		let mut s = Self::new();

		let mut i = 0;
		for (id, length) in rle.iter() {
			for _ in 0..*length {
				if id.is_some() {
					let c = s.contents.get_or_insert_with(|| vec![None; CHUNK_SIZE.pow(3) as usize].into_boxed_slice());
					c[i] = id.clone();
					s.contents_count += 1;
				}
				i += 1;
			}
		}
		s
	}
}


#[derive(Debug, Clone)]
pub struct TerrainChunk {
	contents: TerrainContents,
	pub generation: KGeneration,
}
impl std::ops::Deref for TerrainChunk {
	type Target = TerrainContents;
	fn deref(&self) -> &Self::Target {
		&self.contents
	}
}
impl std::ops::DerefMut for TerrainChunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.contents
	}
}


// An entry in the mesh storage for a map component
#[derive(Debug, Clone, variantly::Variantly)]
pub enum TerrainEntry {
	Loading,	// Waiting for disk data done
	Generating,	// Waiting for generation done
	Complete(Arc<TerrainChunk>),
}


#[derive(Debug, Default, Resource)]
#[sda(commands = true)]
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

		terrain_chunk.and_then(|tc| tc.contents.get(voxel))
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
impl StorageCommandExpose for TerrainResource {
	fn command(&mut self, command: &[&str]) -> anyhow::Result<String> {
		match command[0] {
			"stats" => Ok([
				format!("approx_size: {}", self.approximate_size()),
			].join("\n")),
			_ => Err(anyhow::anyhow!("Unknown command")),
		}
	}
}


#[derive(Debug, Resource)]
#[sda(commands = true)]
pub struct TerrainLoadingResource {
	pub chunk_sender: Sender<(IVec3, TerrainContents, Vec<VoxelModification>)>,
	pub chunk_receiver: Receiver<(IVec3, TerrainContents, Vec<VoxelModification>)>,
	pub max_generation_jobs: u8,
	pub cur_generation_jobs: u8,
	pub vec_generation_jobs: Vec<(IVec3, Instant)>, // For profiling
	// pub generation_durations: RingDataHolder<Duration>, // For profiling

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
			vec_generation_jobs: Vec::with_capacity(16),
			// generation_durations: RingDataHolder::new(32),
			seed, 
			pending_blockmods: HashMap::new(),
			generator: Arc::new(NewTerrainGenerator::new(seed as i32)),
		}
	}
}
impl StorageCommandExpose for TerrainLoadingResource {
	// resource TerrainLoadingResource set max_jobs 32
	fn command(&mut self, command: &[&str]) -> anyhow::Result<String> {
		match command[0] {
			"set" => match command[1] {
				"max_jobs" => if let Some(v) = command.get(2) {
						let v = v.parse::<u8>()?;
						self.max_generation_jobs = v;
						Ok(format!("TerrainLoadingResource max_jobs {}", v))
					} else {
						Err(anyhow::anyhow!("Give a set value"))
					},
				_ => Err(anyhow::anyhow!("Unknown field")),
			},
			"stats" => {
				let s = if let Some((longest_p, longest_t)) = self.vec_generation_jobs.iter()
					.map(|(p, s)| (p, s.elapsed().as_secs_f32()))
					.reduce(|a, v| if a.1 > v.1 { a } else { v }) 
				{
					format!("longest_running: {:?}, {:.1}ms", longest_p, longest_t * 1000.0)
				} else {
					format!("longest_running: None")
				};

				Ok([
					format!("max_jobs: {}", self.max_generation_jobs),
					format!("current_jobs: {}", self.cur_generation_jobs),
					s,
					format!("seed: {}", self.seed),
				].join("\n"))
			},
			_ => Err(anyhow::anyhow!("Unknown command")),
		}
	}
}


pub fn terrain_loading_system(
	blocks: Res<BlockResource>,
	chunks: Res<ChunksResource>,
	terrain: ResMut<TerrainResource>,
	mut loading: ResMut<TerrainLoadingResource>,
	// loaders: Comp<ChunkLoadingComponent>, 
	// transforms: Comp<TransformComponent>, 
) { 
	let mut terrain_chunks = terrain.chunks.write();
	let chunks = chunks.read();
	
	// Prune chunks that should not be loaded
	// Todo: save data
	{ 
		// profiling::scope!("Prune chunks");
		terrain_chunks.retain(|k, _| chunks.chunks.contains_key(k));
	}

	{ // Receive new chunks
		// profiling::scope!("Receive new chunks");
		while let Ok((position, chunk, modifications)) = loading.chunk_receiver.try_recv() {
			trace!("Received generated chunk for {position}");
			let n = chunks.chunks.len();
			let n_loaded = terrain_chunks.values().filter(|e| e.is_complete()).count();
			trace!("Terrain is now {:.2}% loaded",  n_loaded as f32 / n as f32 * 100.0);

			let i = loading.vec_generation_jobs.iter().position(|&(v, _)| v == position)
				.expect("We don't seem to have queued this for generation");
			let (_, _t_start) = loading.vec_generation_jobs.remove(i);
			// loading.generation_durations.insert(t_start.elapsed());

			if let Some(k) = chunks.get_position(position) {
				terrain_chunks.insert(k, TerrainEntry::Complete(Arc::new(TerrainChunk { 
					contents: chunk, 
					generation: KGeneration::new(), 
				})));
				terrain.modify_voxels(modifications.as_slice());
				loading.cur_generation_jobs -= 1;
			} else {
				warn!("Received chunk but not meant to be loaded");
			}
		}
	}
	
	if loading.cur_generation_jobs < loading.max_generation_jobs {
		// profiling::scope!("Start new jobs");
		for &(key, d) in chunks.chunks_by_distance.iter() {
			let position = chunks.chunks[key];
			if !terrain_chunks.contains_key(key) {
				trace!("Begin generating chunk {position} (distance {d})");
				terrain_chunks.insert(key, TerrainEntry::Loading);
	
				let blocks = blocks.read();
				let grass = blocks.key_by_name(&"grass".into()).unwrap();
				let dirt = blocks.key_by_name(&"dirt".into()).unwrap();
				let stone = blocks.key_by_name(&"stone".into()).unwrap();
	
				let generator = loading.generator.clone();
				let sender = loading.chunk_sender.clone();
				rayon::spawn(move || {
					let mut c = TerrainContents::new();
	
					// let tgen = TerrainGenerator::new(0);
					// tgen.chunk_base_3d(position, &mut c, stone);
					// tgen.cover_chunk(&mut c, position, grass, dirt, 3);
	
					generator.base(position, &mut c, stone);
					generator.cover(position, &mut c, grass, dirt, 3);
	
					sender.send((position, c, Vec::new())).unwrap();
				});

				loading.vec_generation_jobs.push((position, Instant::now()));
				loading.cur_generation_jobs += 1;
			}
			if loading.cur_generation_jobs >= loading.max_generation_jobs {
				trace!("Reached maxium chunk generation jobs");
				break;
			}
		}
	}
}
