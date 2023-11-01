use std::{collections::{HashMap, HashSet}, sync::Arc};
use crossbeam_channel::{Sender, Receiver, unbounded};
use eks::prelude::*;
use glam::{IVec3, UVec3, Vec3, Vec2};
use krender::{MeshKey, MaterialKey, prelude::{Mesh, RenderInput}};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use crate::{util::KGeneration, game::{MeshResource, ModelMatrixComponent}, ecs::{TransformComponent, ChunkEntry}, voxel::{chunk_of_point, VoxelSphere, Chunk, chunk::CHUNK_SIZE, BlockManager, BlockRenderType, BlockEntry}};
use super::{MapResource, BlockResource};



#[derive(Debug)]
pub struct MapModelEntry {
	pub dependencies: Vec<(IVec3, KGeneration)>, // includes self (hopefully!)
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


#[derive(Debug, ResourceIdent)]
pub struct MapModelResource {
	// bool for if modelling job is active
	pub chunks: HashMap<IVec3, (bool, MapModelState)>,
	// It may be better to retun a result for this 
	// If a block must be read, but the chunk is not loaded, then give error. 
	pub sender: Sender<(IVec3, Result<(Vec<(UVec3, u32, MaterialKey)>, Vec<(IVec3, KGeneration)>), MeshingError>)>, 
	pub receiver: Receiver<(IVec3, Result<(Vec<(UVec3, u32, MaterialKey)>, Vec<(IVec3, KGeneration)>), MeshingError>)>,
	pub max_meshing_jobs: u8,
	pub cur_meshing_jobs: u8,
}
impl MapModelResource {
	pub fn new(max_meshing_jobs: u8) -> Self {
		assert_ne!(0, max_meshing_jobs);
		let (sender, receiver) = unbounded();
		Self {
			chunks: HashMap::new(),
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
		modelmat: &mut CompMut<ModelMatrixComponent>,
	) {
		for (position, r) in self.receiver.try_iter() {
			match r {
				Ok((quads, dependencies)) => {
					debug!("Received chunk model for {}", position);

					debug!("Contains {} quads", quads.len());

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
					
					// Construct meshes
					let mut models = Vec::new();
					for g in quads_by_key.values() {
						let mut positions = Vec::with_capacity(g.len() * 4);
						let mut uvs = Vec::with_capacity(g.len() * 4);
						let mut indices = Vec::with_capacity(g.len() * 6);
						for &(position, direction, _) in g {
							positions.extend_from_slice(quad_positions(position, direction).as_slice());
							uvs.extend_from_slice(quad_uvs().as_slice());
							indices.extend_from_slice(quad_indices(direction).map(|i| i + positions.len() as u32).as_slice());
						}

						let mesh = Mesh::new(format!("Chunk {position} material {:?}", g[0].2))
							.with_data("positions", positions.as_slice())
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
					modelmat.insert(entity, ModelMatrixComponent::new());

					let entry = MapModelEntry {
						dependencies, models, entity, outdated: false,
					};

					// Todo: make sure it has higher generations than existing

					self.chunks.insert(position, (false, MapModelState::Complete(entry)));
				},
				Err(e) => {
					warn!("Modelling failed for {position} - {e}");

					self.chunks.insert(position, (false, MapModelState::Failed(e)));
					// if let Some((m, _)) = self.chunks.get_mut(&position) {
					// 	*m = false;
					// }
				},
			}
			self.cur_meshing_jobs -= 1;
		}
	}

	pub fn start_jobs(&mut self, map: &MapResource, blocks: &BlockResource) {
		for (&position, (modelling, entry)) in self.chunks.iter_mut() {
			if self.cur_meshing_jobs >= self.max_meshing_jobs {
				debug!("Reached maxium chunk generation jobs");
				break;
			}
			// If not modelling and entry is outdated or none
			if !*modelling {
				if match &entry {
					MapModelState::Complete(e) => e.outdated,
					MapModelState::Failed(_) => false,
					MapModelState::Waiting => true,
				} {
					*modelling = true;
					debug!("Begin modeling chunk {position}");
					let sender = self.sender.clone();
					let chunks = map.chunks.clone();
					let blocks = blocks.blocks.clone();
					rayon::spawn(move || {
						let blocks = blocks.read();
						let mesh_res = chunk_quads_simple(&blocks, &chunks, position);
						sender.send((position, mesh_res)).unwrap();
					});
					self.cur_meshing_jobs += 1;
				}
			}
		}
	}
}


#[derive(Debug, ComponentIdent)]
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
}


#[profiling::function]
pub fn map_modelling_system(
	mut entities: EntitiesMut,
	map: Res<MapResource>,
	mut models: ResMut<MapModelResource>,
	loaders: Comp<MapMeshingComponent>,
	mut transforms: CompMut<TransformComponent>,
	mut meshes: ResMut<MeshResource>,
	mut modelmat: CompMut<ModelMatrixComponent>,
	blocks: Res<BlockResource>,
) {
	info!("Map modeling system");

	let mut chunks_to_load = HashSet::new();
	for (loader, transform) in (&loaders, &transforms).iter() {
		let loader_chunk = chunk_of_point(transform.translation);
		for cpos in VoxelSphere::new(loader_chunk, loader.radius).iter() {
			chunks_to_load.insert(cpos);
		}			
	}

	let mut chunks_to_unload = Vec::new();
	for chunk_position in models.chunks.keys().copied() {
		let should_keep = (&loaders, &transforms).iter().any(|(loader, transform)| {
			let loader_chunk = chunk_of_point(transform.translation);
			VoxelSphere::new(loader_chunk, loader.radius+loader.tolerence).is_within(chunk_position)
		});
		if !should_keep {
			chunks_to_unload.push(chunk_position);
		}
	}

	for position in chunks_to_unload {
		if let Some((_, MapModelState::Complete(_))) = models.chunks.remove(&position) {
			debug!("Unloading model for chunk {position}");
			// Remove meshes
			// Remove entity
		}
	}

	models.receive_jobs(&mut meshes, &mut entities, &mut transforms, &mut modelmat);

	for position in chunks_to_load {
		if models.chunks.get(&position).is_none() {
			debug!("Chunk {position} must be modeled");
			models.chunks.insert(position, (false, MapModelState::Waiting));
		}
	}

	let chunks = map.chunks.read();
	// Check for model validity
	for (modelling, state) in models.chunks.values_mut() {
		// Don't check if we're already trying to fix the issue
		if !*modelling { 
			match state {
				MapModelState::Complete(entry) => {
					entry.outdated = entry.dependencies.iter().any(|(pos, gen)| {
						match chunks.get(pos) {
							Some(ChunkEntry::Complete(g)) => g.generation != *gen,
							_ => false,
						}
					});
				},
				MapModelState::Failed(dep) => {
					match dep {
						MeshingError::ChunkUnloaded(pos) => {
							if let Some(ChunkEntry::Complete(_)) = chunks.get(pos) {
								*state = MapModelState::Waiting;
							}
						},
					}
				},
				MapModelState::Waiting => {},
			}
		}		
	}

	models.start_jobs(&map, &blocks);
}


#[derive(Debug, thiserror::Error)]
pub enum MeshingError {
	#[error("this depends on chunk {0}, which isn't loaded")]
	ChunkUnloaded(IVec3),
}


fn chunk_quads_simple(
	blocks: &BlockManager,
	chunks: &Arc<RwLock<FxHashMap<IVec3, ChunkEntry>>>,
	position: IVec3,
) -> Result<(
	Vec<(UVec3, u32, MaterialKey)>, 
	Vec<(IVec3, KGeneration)>, 
), MeshingError> {
	let chunk = chunks.read().get(&position)
		.ok_or(MeshingError::ChunkUnloaded(position))?
		.complete()
		.ok_or(MeshingError::ChunkUnloaded(position))?
		.clone();
	
	let mut deps = vec![(position, chunk.generation)];
	let mut cxn = None;
	let mut cyn = None;
	let mut czn = None;

	let mut quads = Vec::new();
	for x in 0..CHUNK_SIZE {
		for y in 0..CHUNK_SIZE {
			for z in 0..CHUNK_SIZE {
				let b = chunk.get(UVec3::new(x, y, z));
				let pe = b.and_then(|&key| blocks.get(key));

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
						let e = chunks.read().get(&pxn)
							.ok_or(MeshingError::ChunkUnloaded(pxn))?
							.complete()
							.ok_or(MeshingError::ChunkUnloaded(pxn))?
							.clone();
						deps.push((pxn, e.generation));
						cxn = Some(e);
					}
					let cxn = cxn.as_mut().unwrap();
					cxn.get(UVec3::new(15, y, z))
				} else {
					chunk.get(UVec3::new(x-1, y, z))
				}; 
				// Get entries
				let xne = xn.and_then(|&key| blocks.get(key));
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
						let e = chunks.read().get(&pyn)
							.ok_or(MeshingError::ChunkUnloaded(pyn))?
							.complete()
							.ok_or(MeshingError::ChunkUnloaded(pyn))?
							.clone();
						deps.push((pyn, e.generation));
						cyn = Some(e);
					}
					let cyn = cyn.as_mut().unwrap();
					cyn.get(UVec3::new(x, 15, z))
				} else {
					chunk.get(UVec3::new(x, y-1, z))
				}; 
				// Get entries
				let yne = yn.and_then(|&key| blocks.get(key));
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
						let e = chunks.read().get(&pzn)
							.ok_or(MeshingError::ChunkUnloaded(pzn))?
							.complete()
							.ok_or(MeshingError::ChunkUnloaded(pzn))?
							.clone();
						deps.push((pzn, e.generation));
						czn = Some(e);
					}
					let czn = czn.as_mut().unwrap();
					czn.get(UVec3::new(x, y, 15))
				} else {
					chunk.get(UVec3::new(x, y, z-1))
				}; 
				// Get entries
				let zne = zn.and_then(|&key| blocks.get(key));
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


fn quads_greedy(
	blocks: &BlockManager,
	chunk: &Chunk,
) -> Vec<(IVec3, IVec3, bool, MaterialKey)> {
	let mut quads = Vec::new();
	for pass in 0..3 {
		for mut x in 0..CHUNK_SIZE {
			for mut y in 0..CHUNK_SIZE {
				for mut z in 0..CHUNK_SIZE {
					// Re-order to fit the pass
					match pass {
						0 => {},
						1 => {
							let b = z;
							z = y;
							y = x;
							x = b;
						},
						2 => {
							let b = y;
							z = x;
							y = z;
							x = b;
						},
						_ => unreachable!(),
					}
					println!("get {} {} {}", x, y, z);
				}
			}
		}
	}
	quads
}


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


#[profiling::function]
pub fn map_model_rendering_system(
	(input,): (&mut RenderInput<Entity>,), 
	models: Res<MapModelResource>,
) {
	for entry in models.chunks.values().filter_map(|(_, g)| g.ref_complete()) {
		for &(material, mesh) in entry.models.iter() {
			input.insert_item("models", material, Some(mesh), entry.entity);
		}
	}
}
