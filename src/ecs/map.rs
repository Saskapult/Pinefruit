use std::collections::{HashMap, HashSet};
use std::time::Instant;
use crossbeam_channel::{Receiver, Sender, unbounded};
use nalgebra::Vector3;
use shipyard::*;
use crate::octree::{Octree, chunk_to_octree};
use crate::world::*;
use crate::ecs::*;
use rapier3d::prelude::*;
use crate::mesh::Mesh;
use crate::util::KGeneration;
use generational_arena::Index;





#[derive(Debug)]
pub enum ChunkModelState {
	RequestingModel,
	Remodeling([KGeneration; 4]), // Need this to keep from resubmitting work
	Complete,
}
#[derive(Debug)]
pub struct ChunkModelEntry {
	pub content: Option<(
		Option<Vec<(Index, Index)>>, // Material, mesh, might be empty
		[KGeneration; 4], // self, xp, yp, zp
	)>,
	pub state: ChunkModelState, // Decides what to do with contents
}



#[derive(Unique)]
pub struct MapResource {
	pub map: crate::world::Map,

	pub chunk_octrees: HashMap<[i32; 3], (Octree<usize>, KGeneration)>,

	pub chunk_meshes: HashMap<[i32; 3], ChunkModelEntry>,
	pub chunk_mesh_sender: Sender<([i32;3], [KGeneration; 4], Vec<(Index, Mesh)>)>,
	pub chunk_mesh_receiver: Receiver<([i32;3], [KGeneration; 4], Vec<(Index, Mesh)>)>,
	pub max_meshing_jobs: u8,
	pub cur_meshing_jobs: u8,

	pub rigid_body: Option<RigidBodyHandle>,
	pub chunk_collider_handles: HashMap<[i32; 3], (Option<ColliderHandle>, KGeneration)>,
}
impl MapResource {
	pub fn new(chunk_size: [u32; 3], seed: u32) -> Self {
		let map = crate::world::Map::new(chunk_size, seed);
		let (chunk_mesh_sender, chunk_mesh_receiver) = unbounded();
		Self {
			map,

			chunk_octrees: HashMap::new(),

			chunk_meshes: HashMap::new(),
			chunk_mesh_sender,
			chunk_mesh_receiver,
			max_meshing_jobs: 4,
			cur_meshing_jobs: 0,

			rigid_body: None,
			chunk_collider_handles: HashMap::new(),
		}		
	}

	/// Sets a voxel in the map, regenerating chunks as necessary.
	/// Shouldnot be needed anymore with the new generation system.
	/// I'm keeping it here because I could be wrong about that.
	pub fn set_voxel(&mut self, pos: [i32; 3], voxel: Voxel) {
		// If chunk exists mark its mesh as out of date
		fn remodel_helper(chunk_models: &mut HashMap<[i32; 3], ChunkModelEntry>, cpos: [i32; 3]) {
			if let Some(entry) = chunk_models.get_mut(&cpos) {
				entry.state = ChunkModelState::RequestingModel;
			}
		}

		self.map.set_voxel_world(pos, voxel);
		let (c, v) = self.map.world_chunk_voxel(pos);
		let [cdx, cdy, cdz] = self.map.chunk_size;
		// X cases
		if v[0] as u32 == cdx-1 {
			let cxp = [c[0]+1, c[1], c[2]];
			remodel_helper(&mut self.chunk_meshes, cxp);
		} else if v[0] == 0 {
			let cxn = [c[0]-1, c[1], c[2]];
			remodel_helper(&mut self.chunk_meshes, cxn);
		}
		// Y cases
		if v[1] as u32 == cdy-1 {
			let cyp = [c[0], c[1]+1, c[2]];
			remodel_helper(&mut self.chunk_meshes, cyp);
		} else if v[1] == 0 {
			let cyn = [c[0], c[1]-1, c[2]];
			remodel_helper(&mut self.chunk_meshes, cyn);
		}
		// Z cases
		if v[2] as u32 == cdz-1 {
			let czp = [c[0], c[1], c[2]+1];
			remodel_helper(&mut self.chunk_meshes, czp);
		} else if v[2] == 0 {
			let czn = [c[0], c[1], c[2]-1];
			remodel_helper(&mut self.chunk_meshes, czn);
		}
		// The main chunk
		remodel_helper(&mut self.chunk_meshes, c);
	}
}



#[derive(Debug, Component)]
pub struct MapLoadingComponent {
	pub radius: i32,
}
impl MapLoadingComponent {
	pub fn new(radius: i32) -> Self {
		Self { radius, }
	}
}
const MAP_LOAD_RADIUS: i32 = MAP_MODEL_RADIUS + 2;
pub fn map_loading_system(
	blocks: UniqueView<BlockResource>,
	mut map: UniqueViewMut<MapResource>,
	
	map_loaders: View<MapLoadingComponent>,
	transforms: View<TransformComponent>,
) { 
	let loading_st = std::time::Instant::now();

	map.map.receive_generated_chunks();

	let mut chunks_to_load = HashSet::new();
	for (loader, transform) in (&map_loaders, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform.position);
		let cposs = voxel_sphere(camera_chunk, loader.radius);
		for cpos in cposs {
			chunks_to_load.insert(cpos);
		}			
	}

	let mut chunks_to_unload = Vec::new();
	for chunk_position in map.chunk_meshes.keys() {
		let should_remove = (&map_loaders, &transforms).iter().any(|(loader, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!within_voxel_sphere(camera_chunk, loader.radius+1, *chunk_position)
		});
		if should_remove {
			chunks_to_unload.push(*chunk_position)
		}
	}
	for _chunk_position in chunks_to_unload {
		// Todo: This
		// Save if exists, then remove
	}

	for chunk_position in chunks_to_load {
		map.map.mark_chunk_existence(chunk_position);
	}
	map.map.begin_chunks_generation(&blocks.blocks).unwrap();

	let _loading_dur = loading_st.elapsed();
}



const MAP_MODEL_RADIUS: i32 = 5;
pub fn map_modeling_system(
	blocks: UniqueView<BlockResource>,
	mut meshes: UniqueViewMut<MeshResource>,
	mut map: UniqueViewMut<MapResource>,
	cameras: View<CameraComponent>,
	transforms: View<TransformComponent>,
) {
	let model_st = Instant::now();

	// Unload old meshes
	let mut chunks_to_undisplay = Vec::new();
	for &chunk_position in map.chunk_meshes.keys() {
		// If the chunk is not used for any camera
		let should_remove = (&cameras, &transforms).iter().any(|(_, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!within_voxel_sphere(camera_chunk, MAP_MODEL_RADIUS+1, chunk_position)
		});
		if should_remove {
			chunks_to_undisplay.push(chunk_position)
		}
	}
	for chunk_position in chunks_to_undisplay {
		if let Some(_cme) = map.chunk_meshes.remove(&chunk_position) {
			// Todo: unload mesh and all that
		}
	}

	// Update requested meshes
	let mut chunks_to_display = HashSet::new();
	for (_, transform_c) in (&cameras, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform_c.position);
		let cposs = voxel_sphere(camera_chunk, MAP_MODEL_RADIUS);
		for cpos in cposs {
			chunks_to_display.insert(cpos);
		}
	}
	for chunk_position in chunks_to_display {
		if let Some(_e) = map.chunk_meshes.get(&chunk_position) {
			// Change state to request if state in (remodeling, complete)
			// and self or positive neighbours generation higher than current
			todo!()
		} else {
			map.chunk_meshes.insert(chunk_position, ChunkModelEntry { 
				content: None, 
				state: ChunkModelState::RequestingModel,
			});
		}
	}

	// Load newly created meshes
	for (p, new_gens, mut ms) in map.chunk_mesh_receiver.try_iter().collect::<Vec<_>>() {
		map.cur_meshing_jobs -= 1;
		// If there is no entry then no one asked, so it must be old
		if let Some(e) = map.chunk_meshes.get_mut(&p) {
			// Add if no generation or if generation is old
			if let Some((contents, cur_gens)) = e.content.as_ref() {
				// Must still add if generation is same, accounts of neighbour regeneration
				if cur_gens.iter().zip(new_gens.iter()).any(|(cg, ng)| cg > ng) {
					continue
				}
				// Remove old meshes
				if let Some(mm) = contents {
					for &(_material, mesh) in mm.iter() {
						meshes.meshes.remove(mesh);
					}
				}
				// Insert new meshes if not empty
				if ms.len() > 0 {
					let m = ms.drain(..).map(|(material, mesh)| {
						let mesh_idx = meshes.meshes.insert(mesh);
						(material, mesh_idx)
					}).collect::<Vec<_>>();
					e.content = Some((Some(m), new_gens));
				} else {
					e.content = Some((None, new_gens));
				}
				e.state = ChunkModelState::Complete;
			}
		}
	}

	// Look for things to start meshing iff we can mesh more things
	let max_things_to_start = map.max_meshing_jobs - map.cur_meshing_jobs;
	let mut things_to_start = Vec::new();
	for (&chunk_position, cme) in map.chunk_meshes.iter_mut() {
		// Terminate if no more work can be submitted
		if things_to_start.len() >= max_things_to_start as usize {
			break;
		}
		match cme.state {
			ChunkModelState::RequestingModel => {
				
				things_to_start.push(chunk_position);
			}
			_ => {},
		}
	}
	let n_started = things_to_start.len() as u8;	
	for chunk_position in things_to_start {
		let (f, gens) = map.map.chunk_meshing_function(chunk_position, &blocks.blocks).unwrap();

		let cme = map.chunk_meshes.get_mut(&chunk_position).unwrap();
		cme.state = ChunkModelState::Remodeling(gens);

		let s = map.chunk_mesh_sender.clone();
		rayon::spawn(move || {
			let (meshes,) = f();
			s.send((chunk_position, gens, meshes)).unwrap();
		});
	}
	map.cur_meshing_jobs += n_started;
	
	let _model_dur = model_st.elapsed();
}



const MAP_COLLIDER_RADIUS: i32 = 2;
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
	let meshes = &*meshes;
	if map.rigid_body.is_none() {
		warn!("Map unique rigid body is not intialized");
		return;
	}
	let rigid_body_handle = map.rigid_body.unwrap();

	// Find all chunks which should have colliders
	let mut chunks_to_collide = HashSet::new();
	for (_, transform_c) in (&cameras, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform_c.position);
		let cposs = voxel_sphere(camera_chunk, MAP_COLLIDER_RADIUS);
		for cpos in cposs {
			chunks_to_collide.insert(cpos);
		}				
	}

	// Unload some colliders
	let mut chunks_to_remove = Vec::new();
	for &chunk_position in map.chunk_meshes.keys() {
		let should_remove = (&cameras, &transforms).iter().any(|(_, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!within_voxel_sphere(camera_chunk, MAP_COLLIDER_RADIUS+1, chunk_position)
		});
		if should_remove {
			chunks_to_remove.push(chunk_position)
		}
	}
	for chunk_position in chunks_to_remove {
		if let Some((ch, _)) = map.chunk_collider_handles.remove(&chunk_position) {
			if let Some(ch) = ch {
				physics_resource.remove_collider(ch);
			}
		}
	}

	for chunk_position in chunks_to_collide {
		if let Some(cme) = map.chunk_meshes.get(&chunk_position) {
			// Load iff no exist or old generation
			if let Some(&(c, g)) = map.chunk_collider_handles.get(&chunk_position) {
				if let Some((_, gens)) = cme.content {
					if g < gens[0] {
						continue
					}
					// Unload handle
					if let Some(ch) = c {
						physics_resource.remove_collider(ch);
					}
				} else {
					continue
				}
			}
			// Insert new collider and generation if available
			if let Some((mm, gens)) = cme.content.as_ref() {
				let handle = if let Some(mm) = mm {
					let meshes = mm.iter().map(|&(_, mesh_idx)| meshes.meshes.index(mesh_idx).unwrap()).collect::<Vec<_>>();
					let chunk_shape = crate::mesh::meshes_trimesh(meshes).unwrap();
					let chunk_collider = ColliderBuilder::new(chunk_shape).build();
					let collider_handle = physics_resource.collider_set.insert_with_parent(
						chunk_collider, 
						rigid_body_handle, 
						&mut physics_resource.rigid_body_set,
					);
					Some(collider_handle)
				} else {
					None
				};
				map.chunk_collider_handles.insert(chunk_position, (handle, gens[0]));
			}
		}
	}
	
	let _collider_dur = collider_st.elapsed();
}



#[derive(Debug, Component)]
pub struct MapOctreeLoadingComponent {
	pub radius: i32,
}
impl MapOctreeLoadingComponent {
	pub fn new(radius: i32) -> Self {
		Self { radius, }
	}
}
// Makes octrees, also puts them on the gpu
pub fn map_octree_system(
	mut map: UniqueViewMut<MapResource>,
	loaders: View<MapOctreeLoadingComponent>,
	transforms: View<TransformComponent>,
	mut voxel_data: UniqueViewMut<VoxelRenderingResource>,
	gpu: UniqueView<GraphicsHandleResource>,
) { 
	let map = &mut *map;

	let mut chunks_to_tree = HashSet::new();
	for (loader, transform_c) in (&loaders, &transforms).iter() {
		let camera_chunk = map.map.point_chunk(&transform_c.position);
		let cposs = voxel_sphere(camera_chunk, loader.radius);
		for cpos in cposs {
			chunks_to_tree.insert(cpos);
		}
	}

	let mut chunks_to_untree = Vec::new();
	for &chunk_position in map.chunk_meshes.keys() {
		let should_remove = (&loaders, &transforms).iter().any(|(loader, transform)| {
			let camera_chunk = map.map.point_chunk(&transform.position);
			!within_voxel_sphere(camera_chunk, loader.radius+1, chunk_position)
		});
		if should_remove {
			chunks_to_untree.push(chunk_position)
		}
	}
	// for chunk_position in chunks_to_untree {
	// 	map.chunk_octrees.remove(&chunk_position);
	// }

	for chunk_position in chunks_to_tree {
		// If map chunk exists
		if let Ok(mce) = map.map.chunk_map(chunk_position) {
			// Reload if no entry or map generation is newer
			if let Some((_, gen)) = map.chunk_octrees.get(&chunk_position) {
				if !(mce.generation > *gen) {
					continue
				}
			}
			println!("Creating octree for chunk {chunk_position:?}");
			let tree = chunk_to_octree(&mce.chunk).unwrap();
			voxel_data.insert_chunk(&gpu.queue, chunk_position, &tree);
			map.chunk_octrees.insert(chunk_position, (tree, mce.generation));
			info!("Buffer is now at {:.2}% capacity", voxel_data.buffer.capacity_frac() * 100.0);
		}
	}
}



#[derive(Debug, Component)]
pub struct MapLookAtComponent {
	pub max_distance: f32,
	pub hit: Option<String>,
	pub distance: Option<f32>,
	pub normal: Option<Vector3<f32>>,
}
impl MapLookAtComponent {
	pub fn new(max_distance: f32) -> Self {
		Self {
			max_distance,
			hit: None,
			distance: None,
			normal: None,
		}
	}
}
pub fn map_lookat_system(
	map: UniqueView<MapResource>,
	blocks: UniqueView<BlockResource>,
	mut lookers: ViewMut<MapLookAtComponent>,
	transforms: View<TransformComponent>,
) {
	for (looker, transform) in (&mut lookers, &transforms).iter() {

		let origin = transform.position;
		let direction = transform.rotation * Vector3::new(0.0, 0.0, 1.0);
		
		let res = map.map.ray(origin, direction, 100.0);

		if let Some((v, d, n)) = res {
			let s = v.id().and_then(|i| Some(blocks.blocks.index(i).name.clone()));
			looker.hit = s;
			looker.distance = Some(d);
			looker.normal = Some(n);
		} else {
			looker.hit = None;
			looker.distance = None;
			looker.normal = None;
		}
	}
}
