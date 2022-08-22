use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use shipyard::*;
use crate::world::*;
use crate::ecs::*;
use rapier3d::prelude::*;
use crate::mesh::Mesh;
use crate::util::PTCT;
use generational_arena::Index;




// An entry in the mesh storage for a map component
#[derive(Debug)]
pub enum ChunkModelEntry {
	Empty,
	Unavailable,
	UnModeled,
	Modeling(PTCT<Vec<(Index, Mesh)>>),
	RequestingReModel(Vec<(Index, Index)>),
	ReModeling(Vec<(Index, Index)>, PTCT<Vec<(Index, Mesh)>>),
	Complete(Vec<(Index, Index)>),
}



#[derive(Unique)]
pub struct MapResource {
	pub map: crate::world::Map,
	pub chunk_models: HashMap<[i32; 3], ChunkModelEntry>,
	pub loading_chunks: HashMap<[i32; 3], PTCT<Result<(Chunk, ChunkBlockMods, GenerationStage), GenerationError>>>,
	pub rigid_body: Option<RigidBodyHandle>,
	pub chunk_collider_handles: HashMap<[i32; 3], ColliderHandle>,
}
impl MapResource {
	pub fn new(blockmanager: &Arc<RwLock<crate::world::BlockManager>>) -> Self {
		let map = crate::world::Map::new([16; 3], blockmanager);
		Self {
			map,
			chunk_models: HashMap::new(),
			loading_chunks: HashMap::new(),
			rigid_body: None,
			chunk_collider_handles: HashMap::new(),
		}		
	}

	/// Sets a voxel in the map, regenerating chunks as necessary
	pub fn set_voxel(&mut self, pos: [i32; 3], voxel: Voxel) {
		fn remodel_helper(chunk_models: &mut HashMap<[i32; 3], ChunkModelEntry>, cpos: [i32; 3]) {
			// If it is meant to be displayed
			if chunk_models.contains_key(&cpos) {
				let entry = chunk_models.get_mut(&cpos).unwrap();
				match entry {
					// If complete request remodel
					ChunkModelEntry::Complete(d) => {
						*entry = ChunkModelEntry::RequestingReModel(d.clone());
					},
					// If old model processing then request a new one
					ChunkModelEntry::ReModeling(d, _) => {
						*entry = ChunkModelEntry::RequestingReModel(d.clone());
					},
					ChunkModelEntry::Modeling(_) => {
						*entry = ChunkModelEntry::UnModeled;
					},
					_ => {},
				}
			}
		}

		self.map.set_voxel_world(pos, voxel);
		let (c, v) = self.map.world_chunk_voxel(pos);
		let [cdx, cdy, cdz] = self.map.chunk_size;
		// X cases
		if v[0] as u32 == cdx-1 {
			let cxp = [c[0]+1, c[1], c[2]];
			remodel_helper(&mut self.chunk_models, cxp);
		} else if v[0] == 0 {
			let cxn = [c[0]-1, c[1], c[2]];
			remodel_helper(&mut self.chunk_models, cxn);
		}
		// Y cases
		if v[1] as u32 == cdy-1 {
			let cyp = [c[0], c[1]+1, c[2]];
			remodel_helper(&mut self.chunk_models, cyp);
		} else if v[1] == 0 {
			let cyn = [c[0], c[1]-1, c[2]];
			remodel_helper(&mut self.chunk_models, cyn);
		}
		// Z cases
		if v[2] as u32 == cdz-1 {
			let czp = [c[0], c[1], c[2]+1];
			remodel_helper(&mut self.chunk_models, czp);
		} else if v[2] == 0 {
			let czn = [c[0], c[1], c[2]-1];
			remodel_helper(&mut self.chunk_models, czn);
		}
		// The main chunk
		remodel_helper(&mut self.chunk_models, c);
	}
}



const LOAD_RADIUS: i32 = MODEL_RADIUS + 1;
pub fn map_loading_system(
	mut map: UniqueViewMut<MapResource>,
	cameras: View<CameraComponent>,
	transforms: View<TransformComponent>,
) { 
	let loading_st = std::time::Instant::now();

	let mut chunks_to_load = Vec::new();
	for (_, transform_c) in (&cameras, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform_c.position);
		let mut cposs = map.map.chunks_sphere(camera_chunk, LOAD_RADIUS);
		chunks_to_load.append(&mut cposs);				
	}

	let mut chunks_to_unload = Vec::new();
	for chunk_position in map.chunk_models.keys() {
		let should_remove = (&cameras, &transforms).iter().any(|(_, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!Map::within_chunks_sphere(*chunk_position, camera_chunk, LOAD_RADIUS+1)
		});
		if should_remove {
			chunks_to_unload.push(*chunk_position)
		}
	}

	for _chunk_position in chunks_to_unload {
		// Todo: This
	}

	for chunk_position in chunks_to_load {
		if !map.chunk_models.contains_key(&chunk_position) {
			debug!("Generating chunk {:?}", chunk_position);
			map.map.begin_chunk_generation(chunk_position);
			map.chunk_models.insert(chunk_position, ChunkModelEntry::UnModeled);
		}
	}

	let _loading_dur = loading_st.elapsed();
}



const MODEL_RADIUS: i32 = 3;
pub fn map_modeling_system(
	meshes: UniqueView<MeshResource>,
	mut map: UniqueViewMut<MapResource>,
	cameras: View<CameraComponent>,
	transforms: View<TransformComponent>,
) {
	let map = &mut *map;
	let model_st = Instant::now();

	// Find all chunks which should be displayed
	let mut chunks_to_display = Vec::new();
	for (_, transform_c) in (&cameras, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform_c.position);
		let mut cposs = map.map.chunks_sphere(camera_chunk, MODEL_RADIUS);
		chunks_to_display.append(&mut cposs);				
	}

	// Unload some models
	let mut chunks_to_undisplay = Vec::new();
	for chunk_position in map.chunk_models.keys() {
		// If the chunk is not used for any camera
		let should_remove = (&cameras, &transforms).iter().any(|(_, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!Map::within_chunks_sphere(*chunk_position, camera_chunk, MODEL_RADIUS+1)
		});
		if should_remove {
			chunks_to_undisplay.push(*chunk_position)
		}
	}

	for chunk_position in chunks_to_undisplay {
		if let Some(_cme) = map.chunk_models.remove(&chunk_position) {
			// Todo: unload mesh and all that
		}
	}

	// Load some models
	map.chunk_models.iter_mut().for_each(|(&chunk_position, cme)| {
		match cme {
			ChunkModelEntry::UnModeled => {
				// Poll generation
				if map.map.check_chunk_available(chunk_position) {
					
					// Queue for modeling
					if let Ok(entry) = map.map.mesh_chunk_rayon(chunk_position) {
						debug!("Chunk {:?} has been generated, inserting into modeling queue", chunk_position);
						*cme = ChunkModelEntry::Modeling(entry);
					}
				}
			},
			ChunkModelEntry::RequestingReModel(d) => {
				if let Ok(entry) = map.map.mesh_chunk_rayon(chunk_position) {
					*cme = ChunkModelEntry::ReModeling(d.clone(), entry);
				}
			},
			ChunkModelEntry::ReModeling(_, result) => {
				match result.pollmebb() {
					Some(mut inner_content) => {
						info!("Got remodel for chunk {:?}", chunk_position);
						if inner_content.len() > 0 {
							let mesh_mats = {
								let mut mm = meshes.meshes.write().unwrap();
								inner_content.drain(..).map(|(material_idx, mesh)| {
									let mesh_idx = mm.insert(mesh);
									(mesh_idx, material_idx)
								}).collect::<Vec<_>>()
							};
							*cme = ChunkModelEntry::Complete(mesh_mats);
						} else {
							*cme = ChunkModelEntry::Empty;
						}
					},
					None => {},
				}
			},
			ChunkModelEntry::Modeling(result) => {
				match result.pollmebb() {
					Some(mut inner_content) => {
						info!("Got model for chunk {:?}", chunk_position);
						if inner_content.len() > 0 {
							let mesh_mats = {
								let mut mm = meshes.meshes.write().unwrap();
								inner_content.drain(..).map(|(material_idx, mesh)| {
									let mesh_idx = mm.insert(mesh);
									(mesh_idx, material_idx)
								}).collect::<Vec<_>>()
							};
							*cme = ChunkModelEntry::Complete(mesh_mats);
						} else {
							*cme = ChunkModelEntry::Empty;
						}
					},
					None => {},
				}
			},
			_ => {},
		}
	});
	
	
	let _model_dur = model_st.elapsed();
}



const COLLIDER_RADIUS: i32 = 2;
pub fn map_collider_system(
	meshes: UniqueView<MeshResource>,
	mut physics_resource: UniqueViewMut<PhysicsResource>,
	mut map: UniqueViewMut<MapResource>,
	cameras: View<CameraComponent>,
	transforms: View<TransformComponent>,
) { 
	let collider_st = std::time::Instant::now();

	let physics_resource = &mut *physics_resource;
	let map = &mut *map;
	if map.rigid_body.is_none() {
		warn!("Map unique rigid body is not intialized");
		return;
	}
	let rigid_body_handle = map.rigid_body.unwrap();

	// I love closures! I love closures!
	let generate_chunk_collider = |entry: &ChunkModelEntry| -> Option<Collider> {
		match entry {
			ChunkModelEntry::Complete(meshmats) => {
				let mm = meshes.meshes.read().unwrap();
				let meshes = meshmats.iter().map(|(mesh_idx, _)| mm.index(*mesh_idx).unwrap()).collect::<Vec<_>>();
				let chunk_shape = crate::mesh::meshes_trimesh(meshes).unwrap();
				let chunk_collider = ColliderBuilder::new(chunk_shape).build();
				Some(chunk_collider)
			},
			_ => None,
		}
	};

	// Find all chunks which should have colliders
	let mut chunks_to_collide = Vec::new();
	for (_, transform_c) in (&cameras, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform_c.position);
		let mut cposs = map.map.chunks_sphere(camera_chunk, COLLIDER_RADIUS);
		chunks_to_collide.append(&mut cposs);				
	}

	// Unload some colliders
	let mut chunks_to_remove = Vec::new();
	for chunk_position in map.chunk_models.keys() {
		// If the chunk is not used for any camera
		let should_remove = (&cameras, &transforms).iter().any(|(_, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!Map::within_chunks_sphere(*chunk_position, camera_chunk, COLLIDER_RADIUS+1)
		});
		if should_remove {
			chunks_to_remove.push(*chunk_position)
		}
	}
	for chunk_position in chunks_to_remove {
		if let Some(ch) = map.chunk_collider_handles.remove(&chunk_position) {
			physics_resource.remove_collider(ch);
		}
	}

	for chunk_position in chunks_to_collide {
		if map.chunk_models.contains_key(&chunk_position) && !map.chunk_collider_handles.contains_key(&chunk_position) {
			let entry = &map.chunk_models[&chunk_position];
			if let Some(collider) = generate_chunk_collider(entry) {
				let collider_handle = physics_resource.collider_set.insert_with_parent(
					collider, 
					rigid_body_handle, 
					&mut physics_resource.rigid_body_set,
				);
				map.chunk_collider_handles.insert(chunk_position, collider_handle);
			}
		}
	}
	
	let _collider_dur = collider_st.elapsed();
}
