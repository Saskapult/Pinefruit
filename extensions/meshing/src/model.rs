use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use arrayvec::ArrayVec;
use chunks::{blocks::{BlockEntry, BlockKey, BlockManager, BlockRenderType, BlockResource}, chunk::Chunk, chunk_of_point, chunks::{ChunkKey, ChunksResource}, generation::KGeneration, VoxelCube, CHUNK_SIZE};
use pinecore::controls::ControlComponent;
use crossbeam_channel::{Receiver, Sender};
use eeks::prelude::*;
use glam::{IVec3, UVec3, Vec2, Vec3};
use krender::{prelude::{AbstractRenderTarget, Mesh, RRID}, MaterialKey, MeshKey};
use light::light::{LightRGBA, TorchLightChunksResource, TorchLightModifierComponent};
use parking_lot::RwLock;
use pinecore::render::{MaterialManager, MaterialResource, MeshManager, MeshResource, RenderInputResource};
use slotmap::SecondaryMap;
use smallvec::{smallvec, SmallVec};
use terrain::terrain::{TerrainChunk, TerrainEntry, TerrainResource};
use pinecore::transform::TransformComponent;



#[derive(Debug)]
pub struct MapModelEntry {
	// A model will (almost (if not full of model blocks)) always depend on itself and its negative neighbours
	// It can also depend on any other number of chunks, so a smallvec is necessary
	pub terrain_dependencies: SmallVec<[(IVec3, ChunkKey, KGeneration); 4]>, 
	// A model will always depend on light from itself
	// It might also depend on its negative neighbours
	pub light_dependencies: ArrayVec<(IVec3, ChunkKey, KGeneration), 4>,
	pub models: Vec<(MaterialKey, MeshKey)>, // renderable things
	pub entity: Entity,
	pub outdated: bool,
}


#[derive(Debug)]
pub enum MapModelState {
	Waiting,
	// We need to know this so we don't keep trying to generate chunks with failing deps
	// These are transformed into None by the map loading system
	Failed(MeshingError), 
	Complete(MapModelEntry),
}
impl MapModelState {
	pub fn ref_complete(&self) -> Option<&MapModelEntry> {
		match self {
			Self::Complete(c) => Some(c),
			_ => None,
		}
	}
}



#[derive(Debug, Resource)]
#[sda(commands = true)]
pub struct MapModelResource {
	// bool for if modelling job is active
	pub chunks: SecondaryMap<ChunkKey, (IVec3, bool, MapModelState)>,
	// It may be better to retun a result for this 
	// If a block must be read, but the chunk is not loaded, then give error. 
	pub sender: Sender<(
		ChunkKey, 
		IVec3, 
		Result<(
			Vec<(UVec3, u32, MaterialKey)>, 
			SmallVec<[(IVec3, ChunkKey, KGeneration); 4]>,
		), MeshingError>,
	)>, 
	pub receiver: Receiver<(
		ChunkKey, 
		IVec3, 
		Result<(
			Vec<(UVec3, u32, MaterialKey)>, 
			SmallVec<[(IVec3, ChunkKey, KGeneration); 4]>,
		), MeshingError>,
	)>,
	pub max_meshing_jobs: u8,
	pub cur_meshing_jobs: u8,
}
impl MapModelResource {
	pub fn new(max_meshing_jobs: u8) -> Self {
		assert_ne!(0, max_meshing_jobs);
		let (sender, receiver) = crossbeam_channel::unbounded();
		Self {
			chunks: SecondaryMap::new(),
			sender,
			receiver,
			max_meshing_jobs,
			cur_meshing_jobs: 0,
		}
	}

	pub fn receive_jobs(
		&mut self, 
		meshes: &mut MeshResource,
		entities: &mut EntitiesMut,
		transforms: &mut CompMut<TransformComponent>, 
		torchlight: &TorchLightChunksResource,
	) {
		let torchlight_chunks = torchlight.chunks.read();

		for (key, position, r) in self.receiver.try_iter() {
			match r {
				Ok((quads, terrain_dependencies)) => {
					debug!("Received chunk model for {}", position);

					trace!("Contains {} quads", quads.len());

					// Group quads by material
					let quads_by_key = quads.iter().fold(HashMap::new(), |mut a: HashMap<MaterialKey, Vec<(UVec3, u32, MaterialKey)>>, &v| {
						if let Some(c) = a.get_mut(&v.2) {
							c.push(v);
						} else {
							a.insert(v.2, vec![v]);
						}
						a
					});
					trace!("Contains {} materials", quads_by_key.len());

					let torchlight_c = torchlight_chunks.get(key)
						.expect("Torchlight entry not exists!");
					
					// let mut torchlight_cxn = None;

					let mut light_dependencies = ArrayVec::new();
					light_dependencies.push((position, key, torchlight_c.generation));
					
					// Construct meshes
					let mut models = Vec::with_capacity(quads_by_key.len());
					for g in quads_by_key.values() {
						let mut positions = Vec::with_capacity(g.len() * 4);
						let mut lights = Vec::with_capacity(g.len() * 4);
						let mut uvs = Vec::with_capacity(g.len() * 4);
						let mut indices = Vec::with_capacity(g.len() * 6);
						for &(position, direction, _) in g {
							indices.extend_from_slice(quad_indices(direction).map(|i| i + positions.len() as u32).as_slice());
							uvs.extend_from_slice(quad_uvs().as_slice());
							positions.extend_from_slice(quad_positions(position, direction).as_slice());

							// Find the voxel this face is facing, then get light data from it
							// If on edge and facing negative (to neighbouring chunk)
							let light: f32 = if position.to_array().iter().any(|&p| p == 0) && ((direction & 0b001) != 0) {
								// if position.x == 0 {
								// 	// get cxn 16-1, y, z
								// 	// How do we know the key??
								//  // Maybe deps[2]?
								// 	let cxn = torchlight_cxn.get_or_insert_with(|| torchlight_chunks.get(key))
								// } else if position.y == 0 {

								// } else {

								// }
								1.0
							} else {
								let offs = match direction {
									// Every positive face is just zero
									0b000 => UVec3::ZERO,
									0b001 => UVec3::X,
									0b010 => UVec3::ZERO,
									0b011 => UVec3::Y,
									0b100 => UVec3::ZERO,
									0b101 => UVec3::Z,
									_ => unreachable!(),
								};
								let p = position - offs;
								let l: LightRGBA = torchlight_c.get(p).copied().into();

								if l.r != 0 {
									error!("{p}: {}", l.r);
								}

								l.into_vec4().x
							};
							lights.extend_from_slice(&[light; 4]);
						}

						let mesh = Mesh::new(format!("Chunk {position} material {:?}", g[0].2))
							.with_data("positions", positions.as_slice())
							.with_data("lights", lights.as_slice())
							.with_data("uvs", uvs.as_slice())
							.with_vertex_count(positions.len() as u32)
							.with_indices(indices);
						let key = meshes.insert(mesh);

						models.push((g[0].2, key));
					}
					trace!("Made {} models", models.len());

					// Make entity
					let world_position = (position * CHUNK_SIZE as i32).as_vec3();
					trace!("Spawning chunk entity with position {world_position}");
					let entity = entities.spawn();
					transforms.insert(entity, TransformComponent::new().with_position(world_position));

					let entry = MapModelEntry {
						terrain_dependencies, light_dependencies, models, entity, outdated: false,
					};

					trace!("Insert with key {:?}", key);

					// Todo: make sure it has higher generations than existing

					// Insert and (maybe) unload
					if let Some((_, _, MapModelState::Complete(e))) = self.chunks.insert(key, (position, false, MapModelState::Complete(entry))) {
						let entity = e.entity;
						warn!("Remove entity {entity:?}");
						transforms.remove(entity);
						entities.remove(entity);
						
						for (_, key) in e.models {
							warn!("Remove mesh {key:?}");
							meshes.remove(key);
						}
					}
				},
				Err(e) => {
					warn!("Modelling failed for {position} - {e}");
					self.chunks.insert(key, (position, false, MapModelState::Failed(e)));
				},
			}
			self.cur_meshing_jobs -= 1;
		}
	}
}
impl StorageCommandExpose for MapModelResource {
	// resource MapModelResource set max_jobs 32
	fn command(&mut self, command: &[&str]) -> anyhow::Result<String> {
		match command[0] {
			"set" => match command[1] {
				"max_jobs" => if let Some(v) = command.get(2) {
						let v = v.parse::<u8>()?;
						self.max_meshing_jobs = v;
						Ok(format!("MapModelResource max_jobs {}", v))
					} else {
						Err(anyhow::anyhow!("Give a set value"))
					},
				_ => Err(anyhow::anyhow!("Unknown field")),
			}
			_ => Err(anyhow::anyhow!("Unknown command")),
		}
	}
}


#[derive(Debug, Component)]
pub struct MapMeshingComponent {
	pub radius: i32,
	pub tolerence: i32,
}
impl MapMeshingComponent {
	pub fn new(radius: i32, tolerence: i32) -> Self {
		assert!(radius >= 0);
		assert!(tolerence >= 0);
		Self { radius, tolerence, }
	}

	pub fn loading_volume(&self, transform: TransformComponent) -> VoxelCube {
		VoxelCube::new(chunk_of_point(transform.translation), UVec3::splat(self.radius as u32))
	}

	// Volume but expanded by tolerence
	pub fn un_loading_volume(&self, transform: TransformComponent) -> VoxelCube {
		VoxelCube::new(chunk_of_point(transform.translation), UVec3::splat((self.radius + self.tolerence) as u32))
	}
}



pub fn model_wipe_system(
	controls: Comp<ControlComponent>,
	mut modifiers: CompMut<TorchLightModifierComponent>,
	mut models: ResMut<MapModelResource>,
) {
	for (control, modifier) in (&controls, &mut modifiers).iter() {
		// Only for testing, doesn't wipe jobs in progress
		if control.last_tick_pressed(modifier.wipe) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			models.chunks.clear();
		}
	}
}


pub fn map_modelling_system(
	mut entities: EntitiesMut,
	chunks: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
	torchlight: Res<TorchLightChunksResource>,
	mut models: ResMut<MapModelResource>,
	loaders: Comp<MapMeshingComponent>,
	mut transforms: CompMut<TransformComponent>,
	mut meshes: ResMut<MeshResource>,
	blocks: Res<BlockResource>,
) {
	let loading_volumes = (&loaders, &transforms).iter()
		.map(|(l, t)| l.loading_volume(*t))
		.collect::<Vec<_>>();

	let un_loading_volumes = (&loaders, &transforms).iter()
		.map(|(l, t)| l.un_loading_volume(*t))
		.collect::<Vec<_>>();

	{
		// profiling::scope!("Mark");
		let chunks_chunks = chunks.read();
		for (k, &p) in chunks_chunks.chunks.iter() {
			if loading_volumes.iter().any(|lv| lv.contains(p)) {
				if !models.chunks.contains_key(k) {
					// trace!("Chunk {p} must be modeled");
					models.chunks.insert(k, (p, false, MapModelState::Waiting));
				}
			}
		}
	}

	{
		// profiling::scope!("Prune");
		let g = models.chunks.iter()
			.map(|(key, (pos, _, _))| (key, *pos))
			.collect::<Vec<_>>();
		for (key, pos) in g {
			if !un_loading_volumes.iter().any(|lv| lv.contains(pos)) {
				if let Some((_, _, MapModelState::Complete(_))) = models.chunks.remove(key) {
					// trace!("Unloading model for chunk {}", pos);
					// Remove meshes
					// Remove entity
				}
			}
		}
	}

	models.receive_jobs(&mut meshes, &mut entities, &mut transforms, &torchlight);
	// let n = chunks.read().chunks.len();
	// let n_loaded = models.chunks.values().filter(|(_, w, s)| {
	// 	(!w) && s.ref_complete().is_some()
	// }).count();
	// trace!("World is now {:.2}% meshed",  n_loaded as f32 / n as f32 * 100.0);

	{
		// profiling::scope!("Check for remesh viability");
		let chunks_chunks = chunks.read();
		let terrain_chunks = terrain.chunks.read();
		let light_chunks = torchlight.chunks.read();
		// Check for model validity
		// let mut n_outdated = 0;
		// let mut n_failed = 0;
		// let mut n_waiting = 0;
		for (_, modelling, state) in models.chunks.values_mut() {
			// Don't check if we're already trying to fix the issue
			if !*modelling { 
				match state {
					MapModelState::Complete(entry) => {
						if entry.outdated {
							// Don't test if we already know it's bad
							continue;
						}

						entry.outdated = entry.terrain_dependencies.iter().any(|(_, key, gen)| {
							match terrain_chunks.get(*key) {
								Some(TerrainEntry::Complete(g)) => g.generation != *gen,
								_ => false,
							}
						});
						entry.outdated |= entry.light_dependencies.iter().any(|(p, key, gen)| {
							match light_chunks.get(*key) {
								Some(c) => {
									let g = c.generation != *gen;
									if g {
										warn!("{p}: {:?} != {:?}", c.generation, gen);
									}
									g
								},
								_ => false,
							}
						});
						
						// if entry.outdated {
						// 	n_outdated += 1;
						// }
					},
					MapModelState::Failed(dep) => {
						// n_failed += 1;
						match *dep {
							MeshingError::ChunkUnloaded(pos) => {
								if let Some(TerrainEntry::Complete(_)) = chunks_chunks.get_position(pos).and_then(|k| terrain_chunks.get(k)) {
									*state = MapModelState::Waiting;
									// n_waiting += 1;
								}
							},
						}
					},
					MapModelState::Waiting => {
						// n_waiting += 1;
					},
				}
			}		
		}
		// debug!("{} outdated, {} failed, {} retry", n_outdated, n_failed, n_waiting);
	}

	if models.cur_meshing_jobs < models.max_meshing_jobs {
		let terrain_chunks = terrain.chunks.read();
		for &(key, d) in chunks.read().chunks_by_distance.iter() {
			// All chunks require their own contents to be loaded prior to meshing 
			// We don't want to start a job that will immediately fail 
			if !terrain_chunks.contains_key(key) {
				continue
			}

			// let position = chunks.chunks[key];
			// if loading_volumes.iter().any(|lv| lv.contains(p)) && !models.chunks.contains_key(k) 
			if let Some((position, working, entry)) = models.chunks.get_mut(key) {
				if match &entry {
					MapModelState::Complete(e) => e.outdated,
					MapModelState::Failed(_) => false,
					MapModelState::Waiting => true,
				} {
					let position = *position;
					*working = true;
					trace!("Begin modeling chunk {position} (distance {d})");
					let sender = models.sender.clone();
					let terrain_chunks = terrain.chunks.clone();
					let blocks = blocks.blocks.clone();
					let chunks = chunks.clone();
					rayon::spawn(move || {
						let blocks = blocks.read();
						let mesh_res = chunk_quads_simple(&blocks, &chunks, &terrain_chunks, position);
						sender.send((key, position, mesh_res)).unwrap();
					});
					models.cur_meshing_jobs += 1;
				}
			}

			if models.cur_meshing_jobs >= models.max_meshing_jobs {
				trace!("Reached maxium chunk meshing jobs");
				break;
			}
		}
	}
}


#[derive(Debug, thiserror::Error)]
pub enum MeshingError {
	#[error("this depends on chunk {0}, which isn't loaded")]
	ChunkUnloaded(IVec3),
}


fn chunk_quads_simple(
	blocks: &BlockManager,
	chunks: &ChunksResource, 
	terrain_chunks: &Arc<RwLock<SecondaryMap<ChunkKey, TerrainEntry>>>,
	position: IVec3,
) -> Result<(
	// Direction - 000 xp 001 xn 010 yp 011 100 zp 101 zn
	Vec<(UVec3, u32, MaterialKey)>, 
	SmallVec<[(IVec3, ChunkKey, KGeneration); 4]>,
), MeshingError> {
	fn get_chunk(
		chunks: &ChunksResource, 
		terrain_chunks: &Arc<RwLock<SecondaryMap<ChunkKey, TerrainEntry>>>,
		pos: IVec3,
	) -> Result<(ChunkKey, Arc<TerrainChunk>), MeshingError> {
		let key = chunks.read().get_position(pos).ok_or(MeshingError::ChunkUnloaded(pos))?;
		let chunk = terrain_chunks.read().get(key).ok_or(MeshingError::ChunkUnloaded(pos))?
			.complete_ref().ok_or(MeshingError::ChunkUnloaded(pos))?
			.clone();
		Ok((key, chunk))
	}

	let (chunk_key, chunk) = get_chunk(chunks, terrain_chunks, position)?;
	
	let mut deps = smallvec![(position, chunk_key, chunk.generation)];
	let mut cxn = None;
	let mut cyn = None;
	let mut czn = None;

	let mut quads = Vec::new();
	for x in 0..CHUNK_SIZE {
		for y in 0..CHUNK_SIZE {
			for z in 0..CHUNK_SIZE {
				let b = chunk.get(UVec3::new(x, y, z));
				let pe = b.and_then(|key| blocks.get(key));

				// Returns (positive face?), (negative face?)
				let faces = |pe: Option<&BlockEntry>, ne: Option<&BlockEntry>| {
					// Has face iff is some and other is not covering
					// We might be able to reduce this to one (bool, bool) match but I don't trust myself
					let p_face: Option<bool> = pe.and_then(|e| Some(e.covering));
					let n_face = ne.and_then(|e| Some(e.covering));
					match (n_face, p_face) {
						// empty | empty
						(None, None) => (false, false),
						// empty | anything (negative face)
						(None, Some(_)) => (false, true),
						// anything | empty (positive face)
						(Some(_), None) => (true, false),
						(Some(nc), Some(pc)) => match (nc, pc) {
							// glass | water
							// or glass | glass
							// face generation depends on: 
							// if same type
							//   if faces self
							//     two faces
							//   else
							//     no faces
							// else 
							//   two faces
							(false, false) => todo!(), 
							// glass | stone (two faces)
							(false, true) => (true, true),
							// stone | glass (two faces)
							(true, false) => (true, true),
							// stone | stone (no faces)
							(true, true) => (false, false),
						},
					}
				};

				// Look at xn
				let xn = if x == 0 {
					// Access the adjacent chunk
					if cxn.is_none() {
						let pxn = position - IVec3::X;
						let (k, e) = get_chunk(chunks, terrain_chunks, pxn)?;
						deps.push((pxn, k, e.generation));
						cxn = Some(e);
					}
					let cxn = cxn.as_mut().unwrap();
					cxn.get(UVec3::new(CHUNK_SIZE-1, y, z))
				} else {
					chunk.get(UVec3::new(x-1, y, z))
				}; 
				// Get entries
				let xne = xn.and_then(|key| blocks.get(key));
				let (positive_face, negative_face) = faces(pe, xne);
				if positive_face {
					let m = match xne.as_ref().unwrap().render_type {
						BlockRenderType::Cube(faces) => Some(faces[0]),
						_ => None,
					};
					if let Some(m) = m {
						quads.push((UVec3::new(x, y, z), 0, m));
					}
				}
				if negative_face {
					let m = match pe.as_ref().unwrap().render_type {
						BlockRenderType::Cube(faces) => Some(faces[1]),
						_ => None,
					};
					if let Some(m) = m {
						quads.push((UVec3::new(x, y, z), 1, m));
					}
				}

				// Look at yn
				let yn = if y == 0 {
					if cyn.is_none() {
						let pyn = position - IVec3::Y;
						let (k, e) = get_chunk(chunks, terrain_chunks, pyn)?;
						deps.push((pyn, k, e.generation));
						cyn = Some(e);
					}
					let cyn = cyn.as_mut().unwrap();
					cyn.get(UVec3::new(x, CHUNK_SIZE-1, z))
				} else {
					chunk.get(UVec3::new(x, y-1, z))
				}; 
				// Get entries
				let yne = yn.and_then(|key| blocks.get(key));
				let (positive_face, negative_face) = faces(pe, yne);
				if positive_face {
					let m = match yne.as_ref().unwrap().render_type {
						BlockRenderType::Cube(faces) => Some(faces[2]),
						_ => None,
					};
					if let Some(m) = m {
						quads.push((UVec3::new(x, y, z), 2, m));
					}
				}
				if negative_face {
					let m = match pe.as_ref().unwrap().render_type {
						BlockRenderType::Cube(faces) => Some(faces[3]),
						_ => None,
					};
					if let Some(m) = m {
						quads.push((UVec3::new(x, y, z), 3, m));
					}
				}

				// Look at zn
				let zn = if z == 0 {
					if czn.is_none() {
						let pzn = position - IVec3::Z;
						let (k, e) = get_chunk(chunks, terrain_chunks, pzn)?;
						deps.push((pzn, k, e.generation));
						czn = Some(e);
					}
					let czn = czn.as_mut().unwrap();
					czn.get(UVec3::new(x, y, CHUNK_SIZE-1))
				} else {
					chunk.get(UVec3::new(x, y, z-1))
				}; 
				// Get entries
				let zne = zn.and_then(|key| blocks.get(key));
				let (positive_face, negative_face) = faces(pe, zne);
				if positive_face {
					let m = match zne.as_ref().unwrap().render_type {
						BlockRenderType::Cube(faces) => Some(faces[4]),
						_ => None,
					};
					if let Some(m) = m {
						quads.push((UVec3::new(x, y, z), 4, m));
					}
				}
				if negative_face {
					let m = match pe.as_ref().unwrap().render_type {
						BlockRenderType::Cube(faces) => Some(faces[5]),
						_ => None,
					};
					if let Some(m) = m {
						quads.push((UVec3::new(x, y, z), 5, m));
					}
				}
			}
		}
	}

	Ok((quads, deps))
}


// fn quads_greedy(
// 	blocks: &BlockManager,
// 	chunk: &Chunk,
// ) -> Vec<(IVec3, IVec3, bool, MaterialKey)> {
// 	let mut quads = Vec::new();
// 	for pass in 0..3 {
// 		for mut x in 0..CHUNK_SIZE {
// 			for mut y in 0..CHUNK_SIZE {
// 				for mut z in 0..CHUNK_SIZE {
// 					// Re-order to fit the pass
// 					match pass {
// 						0 => {},
// 						1 => {
// 							let b = z;
// 							z = y;
// 							y = x;
// 							x = b;
// 						},
// 						2 => {
// 							let b = y;
// 							z = x;
// 							y = z;
// 							x = b;
// 						},
// 						_ => unreachable!(),
// 					}
// 					println!("get {} {} {}", x, y, z);
// 				}
// 			}
// 		}
// 	}
// 	quads
// }


// u=x
// v=y
// uv 00, 10, 11, 01
const X_QUAD_POSITIONS: [Vec3; 4] = [
	Vec3::new(0.0, 0.0, 0.0), 
	Vec3::new(0.0, 0.0, 1.0), 
	Vec3::new(0.0, 1.0, 1.0), 
	Vec3::new(0.0, 1.0, 0.0), 
];
const Y_QUAD_POSITIONS: [Vec3; 4] = [
	Vec3::new(0.0, 0.0, 0.0), 
	Vec3::new(1.0, 0.0, 0.0), 
	Vec3::new(1.0, 0.0, 1.0), 
	Vec3::new(0.0, 0.0, 1.0), 
];
// Why is this apparently backwards??
const Z_QUAD_POSITIONS: [Vec3; 4] = [
	Vec3::new(0.0, 0.0, 0.0), 
	Vec3::new(1.0, 0.0, 0.0), 
	Vec3::new(1.0, 1.0, 0.0), 
	Vec3::new(0.0, 1.0, 0.0), 
];


fn quad_positions(position: UVec3, direction: u32) -> [Vec3; 4] {
	match direction & 0b110 {
		0b000 => X_QUAD_POSITIONS,
		0b010 => Y_QUAD_POSITIONS,
		0b100 => Z_QUAD_POSITIONS,
		_ => unreachable!(),
	}.map(|v| v + position.as_vec3())
}


fn quad_uvs() -> [Vec2; 4] {
	[
		Vec2::new(0.0, 1.0), // 00 -> 01
		Vec2::new(1.0, 1.0), // 10 -> 11
		Vec2::new(1.0, 0.0), // 11 -> 10
		Vec2::new(0.0, 0.0), // 01 -> 00
	]
}


fn quad_indices(direction: u32) -> [u32; 6] {
	if direction & 0b1 == 0 {
		[0, 1, 2, 2, 3, 0] // positive
	} else {
		[0, 3, 2, 2, 1, 0] // negative
	}
}


pub fn map_rendering_system(
	// context: Res<ActiveContextResource>,
	// mut contexts: ResMut<ContextResource>, 
	models: Res<MapModelResource>,
	mut input: ResMut<RenderInputResource>,
) {

	let target = AbstractRenderTarget::new()
		.with_colour(RRID::context("albedo"), None)
		.with_depth(RRID::context("depth"));
	let items = input
		.stage("models")
		.target(target);

	for entry in models.chunks.values().filter_map(|(_, _, g)| g.ref_complete()) {
		for &(material, mesh) in entry.models.iter() {
			items.push((material, Some(mesh), entry.entity));
		}
	}
}


pub fn chunk_bounds_rendering_system(
	mut materials: ResMut<MaterialResource>,
	mut meshes: ResMut<MeshResource>,
	models: Res<MapModelResource>,
	mut input: ResMut<RenderInputResource>,
) {
	let target = AbstractRenderTarget::new()
		.with_colour(RRID::context("albedo"), None)
		.with_depth(RRID::context("depth"));
	let items = input
		.stage("models")
		.target(target);

	let material = materials.read("resources/materials/chunk_bounds.ron");
	let mesh = meshes.key_from_label("chunk cube mesh").unwrap_or_else(|| {
		// let size = CHUNK_SIZE as f32;
		// let positions = [
		// 	-0.5, -0.5, 0.5,
		// 	0.5, -0.5, 0.5,
		// 	-0.5, 0.5, 0.5,
		// 	0.5, 0.5, 0.5,
		// 	-0.5, 0.5, -0.5,
		// 	0.5, 0.5, -0.5,
		// 	-0.5, -0.5, -0.5,
		// 	0.5, -0.5, -0.5,
		// ].map(|v| (v + 0.5) * size);
		// let indices = [
		// 	1, 2, 3,
		// 	3, 2, 4,
		// 	3, 4, 5,
		// 	5, 4, 6,
		// 	5, 6, 7,
		// 	7, 6, 8,
		// 	7, 8, 1,
		// 	1, 8, 2,
		// 	2, 8, 4,
		// 	4, 8, 6,
		// 	7, 1, 5,
		// 	5, 1, 3,
		// ].map(|v| v - 1).to_vec();
		// let mesh = Mesh::new("chunk cube mesh")
		// 	.with_data("positions", &positions)
		// 	.with_vertex_count(8)
		// 	.with_indices(indices);
		let mesh = Mesh::read_obj("resources/meshes/cube.obj");
		meshes.insert(mesh)
	});

	for entry in models.chunks.values().filter_map(|(_, _, g)| g.ref_complete()) {
		items.push((material, Some(mesh), entry.entity));
	}
}
