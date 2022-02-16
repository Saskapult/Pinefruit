use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::world::*;
use crate::ecs::*;




// An entry in the mesh storage for a map component
#[derive(Debug)]
pub enum ChunkModelEntry {
	Empty,
	Unloaded,
	UnModeled,
	Complete(Vec<(usize, usize)>),
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct MapComponent {
	pub map: crate::world::Map,
	// A field for storing generated mesh index collections (or a lack thereof)
	pub chunk_models: HashMap<[i32; 3], ChunkModelEntry>,
}
impl MapComponent {
	pub fn new(blockmanager: &Arc<RwLock<crate::world::BlockManager>>) -> Self {
		let mut map = crate::world::Map::new([16; 3], blockmanager);
		map.generate();
		Self {
			map,
			chunk_models: HashMap::new(),
		}		
	}

	/// Sets a voxel in the map, regenerating chunks as necessary
	fn set_voxel(&mut self, pos: [i32; 3], voxel: Voxel) {
		self.map.set_voxel_world(pos, voxel);
		let (c, v) = self.map.world_chunk_voxel(pos);
		let [cdx, cdy, cdz] = self.map.chunk_dimensions;
		// X cases
		if v[0] as u32 == cdx-1 {
			let cx = [c[0]+1, c[1], c[2]];
			if self.chunk_models.contains_key(&cx) {
				self.chunk_models.insert(cx, ChunkModelEntry::UnModeled);
			}
		} else if v[0] == 0 {
			let cx = [c[0]-1, c[1], c[2]];
			if self.chunk_models.contains_key(&cx) {
				self.chunk_models.insert(cx, ChunkModelEntry::UnModeled);
			}
		}
		// Y cases
		if v[1] as u32 == cdy-1 {
			let cy = [c[0], c[1]+1, c[2]];
			if self.chunk_models.contains_key(&cy) {
				self.chunk_models.insert(cy, ChunkModelEntry::UnModeled);
			}
		} else if v[1] == 0 {
			let cy = [c[0], c[1]-1, c[2]];
			if self.chunk_models.contains_key(&cy) {
				self.chunk_models.insert(cy, ChunkModelEntry::UnModeled);
			}
		}
		// Z cases
		if v[2] as u32 == cdz-1 {
			let cz = [c[0], c[1], c[2]+1];
			if self.chunk_models.contains_key(&cz) {
				self.chunk_models.insert(cz, ChunkModelEntry::UnModeled);
			}
		} else if v[2] == 0 {
			let cz = [c[0], c[1], c[2]-1];
			if self.chunk_models.contains_key(&cz) {
				self.chunk_models.insert(cz, ChunkModelEntry::UnModeled);
			}
		}
		// The main chunk
		if self.chunk_models.contains_key(&c) {
			self.chunk_models.insert(c, ChunkModelEntry::UnModeled);
		}
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
}
impl<'a> System<'a> for MapSystem {
	type SystemData = (
		WriteExpect<'a, RenderResource>,
		WriteStorage<'a, MapComponent>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);
	fn run(
		&mut self, 
		(
			mut render_resource,
			mut map,
			camera,
			transform,
		): Self::SystemData,
	) { 
		for map_c in (&mut map).join() {
			
			// Find all chunks which should be displayed
			let mut chunks_to_show = Vec::new();
			for (_, transform_c) in (&camera, &transform).join() {
				let camera_chunk = map_c.map.point_chunk(transform_c.position);
				let mut cposs = map_c.map.chunks_sphere(camera_chunk, 5);
				chunks_to_show.append(&mut cposs);				
			}
			info!("Need to show {} chunks!", chunks_to_show.len());

			// // Unload some chunks
			// let mut chunks_to_remove = Vec::new();
			// for chunk_position in map_c.chunk_models.keys() {
			// 	let mut should_remove = true;
			// 	for (_, transform_c) in (&camera, &transform).join() {
			// 		let camera_chunk = map_c.map.point_chunk(transform_c.position);
			// 		should_remove &= Map::within_chunks_sphere(*chunk_position, camera_chunk, 5+1);
			// 		if !should_remove {
			// 			break
			// 		}
			// 	}
			// 	if should_remove {
			// 		chunks_to_remove.push(*chunk_position)
			// 	}
			// }
			// for chunk_position in chunks_to_remove {
			// 	if let Some(_cme) = map_c.chunk_models.remove(&chunk_position) {
			// 		// Todo: unload mesh and all that
			// 	}
			// }

			// Load some chunks
			for chunk_position in chunks_to_show {
				if map_c.chunk_models.contains_key(&chunk_position) {
					match map_c.chunk_models[&chunk_position] {
						ChunkModelEntry::UnModeled => {
							// Model it
							let res = MapSystem::model_chunk(&mut render_resource, &map_c.map, chunk_position);
							map_c.chunk_models.insert(chunk_position, res);
						}
						_ => {},
					}
				} else { 
					let res = MapSystem::model_chunk(&mut render_resource, &map_c.map, chunk_position);
					map_c.chunk_models.insert(chunk_position, res);
				}
			}
		}
	}
}


// pub trait InitializeMap {
// 	fn init_map(&mut self);
// }
// impl InitializeMap for World {
// 	fn init_map(&mut self) {
// 		self.register::<MapComponent>();
// 	}
// }
