use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::world::*;
use crate::ecs::*;
use rapier3d::prelude::*;
use crate::mesh::Mesh;
use crate::util::PTCT;




// An entry in the mesh storage for a map component
#[derive(Debug)]
pub enum ChunkModelEntry {
	Empty,
	Unavailable,
	UnModeled,
	Modeling(PTCT<Vec<(usize, Mesh)>>),
	RequestingReModel(Vec<(usize, usize)>),
	ReModeling(Vec<(usize, usize)>, PTCT<Vec<(usize, Mesh)>>),
	Complete(Vec<(usize, usize)>),
}



#[derive(Component)]
#[storage(VecStorage)]
pub struct MapComponent {
	pub map: crate::world::Map,
	pub chunk_models: HashMap<[i32; 3], ChunkModelEntry>,
	pub loading_chunks: HashMap<[i32; 3], PTCT<Result<(Chunk, ChunkBlockMods, GenerationStage), GenerationError>>>,
	pub chunk_collider_handles: HashMap<[i32; 3], ColliderHandle>,
}
impl MapComponent {
	pub fn new(blockmanager: &Arc<RwLock<crate::world::BlockManager>>) -> Self {
		let map = crate::world::Map::new([16; 3], blockmanager);
		// map.generate();
		Self {
			map,
			chunk_models: HashMap::new(),
			loading_chunks: HashMap::new(),
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

	// trait RenderableComponent?
	pub fn get_render_data(&self) -> Vec<(usize, usize)> {

		// self.chunk_models.iter().filter_map(|(_cpos, cme)| {
		// 	match cme {
		// 		_ => None,
		// 	}
		// }).collect::<Vec<_>>();

		todo!()
	}
}



/// The map system is responsible for loading and meshing chunks of maps near the cameras 
pub struct MapSystem;
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
			render_resource,
			mut physics_resource,
			mut maps,
			mut static_objects,
			cameras,
			transforms,
		): Self::SystemData,
	) { 
		// I love closures! I love closures!
		let generate_chunk_collider = |entry: &ChunkModelEntry| -> Option<Collider> {
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
		};

		let load_radius = 4;
		for map_c in (&mut maps).join() {
			let mut chunks_to_load = Vec::new();
			for (_, transform_c) in (&cameras, &transforms).join() {
				let camera_chunk = map_c.map.point_chunk(&transform_c.position);
				let mut cposs = map_c.map.chunks_sphere(camera_chunk, load_radius);
				chunks_to_load.append(&mut cposs);				
			}

			let mut chunks_to_unload = Vec::new();
			for chunk_position in map_c.chunk_models.keys() {
				let should_remove = (&cameras, &transforms).join().any(|(_, transform)| {
					let camera_chunk = map_c.map.point_chunk(&transform.position);
					!Map::within_chunks_sphere(*chunk_position, camera_chunk, load_radius+1)
				});
				if should_remove {
					chunks_to_unload.push(*chunk_position)
				}
			}

			for _chunk_position in chunks_to_unload {
				// Todo: This
			}

			for chunk_position in chunks_to_load {
				if !map_c.chunk_models.contains_key(&chunk_position) {
					debug!("Generating chunk {:?}", chunk_position);
					map_c.map.begin_chunk_generation(chunk_position);
					map_c.chunk_models.insert(chunk_position, ChunkModelEntry::UnModeled);
				}
			}
		}
		

		// Model loading
		let model_radius = 3;
		for map_c in (&mut maps).join() {
			
			// Find all chunks which should be displayed
			let mut chunks_to_display = Vec::new();
			for (_, transform_c) in (&cameras, &transforms).join() {
				let camera_chunk = map_c.map.point_chunk(&transform_c.position);
				let mut cposs = map_c.map.chunks_sphere(camera_chunk, model_radius);
				chunks_to_display.append(&mut cposs);				
			}

			// Unload some models
			let mut chunks_to_undisplay = Vec::new();
			for chunk_position in map_c.chunk_models.keys() {
				// If the chunk is not used for any camera
				let should_remove = (&cameras, &transforms).join().any(|(_, transform)| {
					let camera_chunk = map_c.map.point_chunk(&transform.position);
					!Map::within_chunks_sphere(*chunk_position, camera_chunk, model_radius+1)
				});
				if should_remove {
					chunks_to_undisplay.push(*chunk_position)
				}
			}

			for chunk_position in chunks_to_undisplay {
				if let Some(_cme) = map_c.chunk_models.remove(&chunk_position) {
					// Todo: unload mesh and all that
				}
			}

			// Load some models
			map_c.chunk_models.iter_mut().for_each(|(&chunk_position, cme)| {
				match cme {
					ChunkModelEntry::UnModeled => {
						// Poll generation
						if map_c.map.check_chunk_done(chunk_position) {
							// Queue for modeling
							if let Ok(entry) = map_c.map.mesh_chunk_rayon(chunk_position) {
								*cme = ChunkModelEntry::Modeling(entry);
							}
						}
					},
					ChunkModelEntry::RequestingReModel(d) => {
						if let Ok(entry) = map_c.map.mesh_chunk_rayon(chunk_position) {
							*cme = ChunkModelEntry::ReModeling(d.clone(), entry);
						}
					},
					ChunkModelEntry::ReModeling(_, result) => {
						match result.pollmebb() {
							Some(mut inner_content) => {
								info!("Got remodel for chunk {:?}", chunk_position);
								if inner_content.len() > 0 {
									let mesh_mats = {
										let mut mm = render_resource.meshes_manager.write().unwrap();
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
										let mut mm = render_resource.meshes_manager.write().unwrap();
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
					if let Some(collider) = generate_chunk_collider(entry) {
						let ch = spc.add_collider(&mut physics_resource, collider);
						map.chunk_collider_handles.insert(chunk_position, ch);
					}
				}
			}
		}
		
	}
}
