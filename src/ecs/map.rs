use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::world::*;
use crate::ecs::*;
use rapier3d::prelude::*;
use crate::mesh::Mesh;




// An entry in the mesh storage for a map component
#[derive(Debug)]
pub enum ChunkModelEntry {
	Empty,
	Unloaded,
	UnModeled,
	Modeling(Arc<Mutex<Option<Vec<(usize, Mesh)>>>>),
	RequestingReModel(Vec<(usize, usize)>),
	ReModeling(Vec<(usize, usize)>, Arc<Mutex<Option<Vec<(usize, Mesh)>>>>),
	Complete(Vec<(usize, usize)>),
}



#[derive(Component)]
#[storage(VecStorage)]
pub struct MapComponent {
	pub map: crate::world::Map,
	pub chunk_models: HashMap<[i32; 3], ChunkModelEntry>,
	pub chunk_collider_handles: HashMap<[i32; 3], ColliderHandle>,
}
impl MapComponent {
	pub fn new(blockmanager: &Arc<RwLock<crate::world::BlockManager>>) -> Self {
		let mut map = crate::world::Map::new([16; 3], blockmanager);
		map.generate();
		Self {
			map,
			chunk_models: HashMap::new(),
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
		let [cdx, cdy, cdz] = self.map.chunk_dimensions;
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

	// trait RenderableComponent?
	pub fn get_render_data(&self) -> Vec<(usize, usize)> {

		self.chunk_models.iter().filter_map(|(_cpos, cme)| {
			match cme {
				_ => None,
			}
		}).collect::<Vec<_>>()
	}
}



/// The map system is responsible for loading and meshing chunks of maps near the cameras 
pub struct MapSystem;
impl MapSystem {
	fn model_chunk(
		renderr: &mut RenderResource,
		map: &crate::world::Map, 
		chunk_position: [i32; 3],
	) -> ChunkModelEntry {
		//info!("Evaluating chunk {:?} for modeling", chunk_position);
		if map.is_chunk_loaded(chunk_position) {
			info!("Modeling chunk {:?}", chunk_position);
			// Model it and register the segments
			let mesh_mats = {
				let mut mm = renderr.meshes_manager.write().unwrap();
				map.mesh_chunk(chunk_position).drain(..).map(|(material_idx, mesh)| {
					let mesh_idx = mm.insert(mesh);
					(mesh_idx, material_idx)
				}).collect::<Vec<_>>()
			};
			if mesh_mats.len() > 0 {
				//info!("Chunk {:?} modeled", chunk_position);
				ChunkModelEntry::Complete(mesh_mats)
			} else {
				info!("Chunk {:?} was empty", chunk_position);
				ChunkModelEntry::Empty
			}
		} else {
			//info!("Chunk {:?} was not available", chunk_position);
			ChunkModelEntry::Unloaded
		}
	}

	fn generate_chunk_collider(
		render_resource: &RenderResource,
		entry: &ChunkModelEntry,
	) -> Option<Collider> {
		match entry {
			ChunkModelEntry::Complete(meshmats) => {
				let mm = render_resource.meshes_manager.read().unwrap();
				let meshes = meshmats.iter().map(|(mesh_idx, _)| mm.index(*mesh_idx)).collect::<Vec<_>>();
				let chunk_shape = crate::mesh::meshes_trimesh(meshes).unwrap();
				let chunk_collider = ColliderBuilder::new(chunk_shape).build();
				Some(chunk_collider)
			},
			_ => None,
		}
	}
}
impl<'a> System<'a> for MapSystem {
	type SystemData = (
		WriteExpect<'a, RenderResource>,
		WriteExpect<'a, PhysicsResource>,
		WriteStorage<'a, MapComponent>,
		WriteStorage<'a, StaticPhysicsComponent>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);
	fn run(
		&mut self, 
		(
			mut render_resource,
			mut physics_resource,
			mut maps,
			mut static_objects,
			cameras,
			transforms,
		): Self::SystemData,
	) { 
		// Model loading
		let model_radius = 3;
		for map_c in (&mut maps).join() {
			
			// Find all chunks which should be displayed
			let mut chunks_to_show = Vec::new();
			for (_, transform_c) in (&cameras, &transforms).join() {
				let camera_chunk = map_c.map.point_chunk(&transform_c.position);
				let mut cposs = map_c.map.chunks_sphere(camera_chunk, model_radius);
				chunks_to_show.append(&mut cposs);				
			}

			// Unload some models
			let mut chunks_to_remove = Vec::new();
			for chunk_position in map_c.chunk_models.keys() {
				// If the chunk is not used for any camera
				let should_remove = (&cameras, &transforms).join().any(|(_, transform)| {
					let camera_chunk = map_c.map.point_chunk(&transform.position);
					!Map::within_chunks_sphere(*chunk_position, camera_chunk, model_radius+1)
				});
				if should_remove {
					chunks_to_remove.push(*chunk_position)
				}
			}
			for chunk_position in chunks_to_remove {
				if let Some(_cme) = map_c.chunk_models.remove(&chunk_position) {
					// Todo: unload mesh and all that
				}
			}

			// Enter new chunks for rendering
			for chunk_position in chunks_to_show {
				if !map_c.chunk_models.contains_key(&chunk_position) {
					if map_c.map.is_chunk_loaded(chunk_position) {
						map_c.chunk_models.insert(chunk_position, ChunkModelEntry::UnModeled);
					} else {
						map_c.chunk_models.insert(chunk_position, ChunkModelEntry::Unloaded);
					}
					
				}
			}

			// Load some models
			map_c.chunk_models.iter_mut().for_each(|(&chunk_position, cme)| {
				match cme {
					ChunkModelEntry::UnModeled => {
						// Queue for modeling
						let entry = map_c.map.mesh_chunk_rayon(chunk_position);
						*cme = ChunkModelEntry::Modeling(entry);
					},
					ChunkModelEntry::RequestingReModel(d) => {
						let entry = map_c.map.mesh_chunk_rayon(chunk_position);
						*cme = ChunkModelEntry::ReModeling(d.clone(), entry);
					},
					ChunkModelEntry::ReModeling(_, result) => {
						// Test if modeling is done
						let mut content = result.lock().unwrap();
						if content.is_some() {
							info!("Got remodel for chunk {:?}", chunk_position);
							let inner_content = content.as_mut().unwrap();

							if inner_content.len() > 0 {
								let mesh_mats = {
									let mut mm = render_resource.meshes_manager.write().unwrap();
									inner_content.drain(..).map(|(material_idx, mesh)| {
										let mesh_idx = mm.insert(mesh);
										(mesh_idx, material_idx)
									}).collect::<Vec<_>>()
								};
								// drop(result);
								drop(content);
								*cme = ChunkModelEntry::Complete(mesh_mats);
							} else {
								drop(content);
								*cme = ChunkModelEntry::Empty;
							}
						}
					},
					ChunkModelEntry::Modeling(result) => {
						// Test if modeling is done
						let mut content = result.lock().unwrap();
						if content.is_some() {
							info!("Got model for chunk {:?}", chunk_position);
							let inner_content = content.as_mut().unwrap();

							if inner_content.len() > 0 {
								let mesh_mats = {
									let mut mm = render_resource.meshes_manager.write().unwrap();
									inner_content.drain(..).map(|(material_idx, mesh)| {
										let mesh_idx = mm.insert(mesh);
										(mesh_idx, material_idx)
									}).collect::<Vec<_>>()
								};
								// drop(result);
								drop(content);
								*cme = ChunkModelEntry::Complete(mesh_mats);
							} else {
								drop(content);
								*cme = ChunkModelEntry::Empty;
							}
						}
					},
					_ => {},
				}
			})
		}

		// Collider loading
		let collider_radius = 3;
		for (map, spc) in (&mut maps, &mut static_objects).join() {
			// Find all chunks which should have colliders
			let mut chunks_to_collide = Vec::new();
			for (_, transform_c) in (&cameras, &transforms).join() {
				let camera_chunk = map.map.point_chunk(&transform_c.position);
				let mut cposs = map.map.chunks_sphere(camera_chunk, collider_radius);
				chunks_to_collide.append(&mut cposs);				
			}

			// Unload some colliders
			let mut chunks_to_remove = Vec::new();
			for chunk_position in map.chunk_models.keys() {
				// If the chunk is not used for any camera
				let should_remove = (&cameras, &transforms).join().any(|(_, transform)| {
					let camera_chunk = map.map.point_chunk(&transform.position);
					!Map::within_chunks_sphere(*chunk_position, camera_chunk, collider_radius+1)
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
					if let Some(collider) = MapSystem::generate_chunk_collider(&render_resource, entry) {
						let ch = spc.add_collider(&mut physics_resource, collider);
						map.chunk_collider_handles.insert(chunk_position, ch);
					}
				}
			}
		}
		
	}
}
