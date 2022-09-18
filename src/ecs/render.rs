use std::sync::Arc;
use std::{time::Instant, collections::HashMap};
use nalgebra::{Point3, Matrix4, Translation3};
use shipyard::*;
use crate::material::MaterialManager;
use crate::mesh::MeshManager;
use crate::render::{ShaderManager, RenderCamera};
use crate::texture::TextureManager;
use crate::{world::*, octree::Octree};
use crate::ecs::*;
use generational_arena::{Arena, Index};




#[derive(Debug, Unique)]
pub struct GraphicsHandleResource {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
}
#[derive(Debug, Unique, Default)]
pub struct TextureResource {
	pub textures: TextureManager,
}
#[derive(Debug, Unique, Default)]
pub struct MeshResource {
	pub meshes: MeshManager,
}
#[derive(Debug, Unique, Default)]
pub struct MaterialResource {
	pub materials: MaterialManager,
}
#[derive(Debug, Unique, Default)]
pub struct BlockResource {
	pub blocks: BlockManager,

}



#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct ArrayVolumeHeader {
	pub size: [u32; 4],
}
impl ArrayVolumeHeader {
	pub fn new(size: [u32; 3]) -> Self {
		Self { size: [size[0], size[1], size[2], 42], }
	}
}


#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct CubeColours {
	pub colours: [[u8; 3]; 6],
}



#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
struct VolumeOBB {
	pub aabb_max: [f32; 3],
	pub aabb_min: [f32; 3],
	pub matrix: [[f32; 4]; 4],
}


/// Octree node as seen by the shader.
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
struct AccelOctreeNode {
	octants: [u32; 8],
	content: u32,
	leaf_mask: u32, // Should be u8 but I'm paranoid
}


#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
struct VRCU {
	chunk_as_centre: [i32; 4],
	chunk_as_header: ArrayVolumeHeader,
	chunk_size: u32,
	chunk_as_idx: [u32; 2],
	colours_idx: [u32; 2],
	n_array_volumes: u32,
	array_volumes: [u32; 2],
}
#[derive(Debug, Component)]
/// Per-view information
pub struct VoxelRenderingComponent {
	// Start of chunk AS
	pub chunk_as_centre: [i32; 3],
	pub chunk_as_header: ArrayVolumeHeader,
	pub chunk_size: u32,
	pub chunk_as_idx: (usize, usize),
	// Start of block colours
	// Convert to float using uintBitsToFloat if usig full float
	pub colours_idx: (usize, usize),
	// Start of list of (obb, volume st)
	// This might be better for the GPU cache because oobs are so close together
	// I don't know enough about GPU architecture to know that though
	// This does mean that we will store obb info for each object for each view
	// It still shouldn't be much data, so I don't care
	pub n_array_volumes: u32,
	pub array_volumes: (usize, usize),
}
impl VoxelRenderingComponent {
	pub fn new(radius: u32) -> Self {
		Self {
			chunk_as_centre: [0; 3],
			chunk_as_header: ArrayVolumeHeader::new([radius*2+1; 3]),
			chunk_size: 0,
			chunk_as_idx: (0, 0),	
			colours_idx: (0, 0),
			n_array_volumes: 0,
			array_volumes: (0, 0),
		}
	}
	
	pub fn to_uniform_contents(&self) -> Vec<u8> {
		bytemuck::bytes_of(&VRCU {
			chunk_as_centre: [self.chunk_as_centre[0], self.chunk_as_centre[1], self.chunk_as_centre[2], 42],
			chunk_as_header: self.chunk_as_header,
			chunk_size: self.chunk_size,
			chunk_as_idx: [self.chunk_as_idx.0 as u32, self.chunk_as_idx.1 as u32],
			colours_idx: [self.colours_idx.0 as u32, self.colours_idx.1 as u32],
			n_array_volumes: self.n_array_volumes,
			array_volumes: [self.array_volumes.0 as u32, self.array_volumes.1 as u32],
		}).to_vec()		
	}

	pub fn vrcu_size() -> usize {
		// vec4 aligned?
		std::mem::size_of::<VRCU>().next_multiple_of(16)
	}
}



/// All the rendering voxel stuff for all views
#[derive(Debug, Unique)]
pub struct VoxelRenderingResource {
	// All data
	pub buffer: SlabBuffer,
	// Octree
	pub chunks_octrees: HashMap<[i32; 3], (usize, usize)>,
	// Header and contents
	pub entity_array_volumes: HashMap<EntityId, Index>,
	pub array_volumes: Arena<(usize, usize)>,
	
	// Limited to u8 for simplicity
	pub colours: Vec<[u8; 4]>,
	pub colours_idx: (usize, usize),

	pub scene_shader_index: Index,
	pub vrc_buffer: wgpu::Buffer,
	pub scene_bg: wgpu::BindGroup,
}
impl VoxelRenderingResource {
	// 1GB is 1073741824B
	// 1MB is 1048576B
	const BUFFER_SIZE: usize = 1048576 * 64;
	const SLAB_SIZE: usize = 256; // 64 uints

	pub fn new(device: &wgpu::Device, shaders: &mut ShaderManager) -> Self {
		let slab_count = Self::BUFFER_SIZE / Self::SLAB_SIZE;
		let buffer = SlabBuffer::new(device, Self::SLAB_SIZE, slab_count, wgpu::BufferUsages::STORAGE);

		let vrc_buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("Voxel rendering component uniform buffer"),
			size: VoxelRenderingComponent::vrcu_size() as u64,
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
			mapped_at_creation: false,
		});

		// This is messy and I slightly hate it
		let scene_shader_index = shaders.register_path("resources/shaders/voxel_scene.ron");
		let shader_pt = shaders.prototype(scene_shader_index).unwrap();
		let scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("Voxel scene bg"),
			layout: shaders.layout(&shader_pt.bind_group_entries(2).unwrap()).unwrap(),
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::Buffer(buffer.buffer.as_entire_buffer_binding()),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Buffer(vrc_buffer.as_entire_buffer_binding()),
				},
			],
		});

		Self {
			buffer,
			chunks_octrees: HashMap::new(),
			entity_array_volumes: HashMap::new(),
			array_volumes: Arena::new(),
			colours: Vec::new(),
			colours_idx: (0, 0),
			scene_shader_index,
			vrc_buffer,
			scene_bg,
		}
	}

	pub fn insert_chunk(&mut self, queue: &wgpu::Queue, chunk_position: [i32; 3], octree: &Octree<usize>) {
		if let Some(&(st, en)) = self.chunks_octrees.get(&chunk_position) {
			self.buffer.remove(st..en);
		}

		let nodes = octree.nodes.iter()
			.map(|n| AccelOctreeNode {
				// Extract data and store it directly
				octants: {
					let mut octants = n.octants;
					for i in 0..8 {
						let v = n.octants[i];

						if n.is_leaf(i) && v != 0 {
							octants[i] = octree.data[v as usize - 1] as u32 + 1
						}
					}
					octants
				},
				content: { 
					if n.content != 0 {
						octree.data[n.content as usize - 1] as u32 + 1
					} else {
						0
					}
				},
				leaf_mask: n.leaf_mask as u32,
			})
			.collect::<Vec<_>>();

		let data = bytemuck::cast_slice(&nodes[..]);
		let [st, en] = self.buffer.insert_direct(queue, data);
		self.chunks_octrees.insert(chunk_position, (st, en));
	}
	pub fn get_chunk(&self, chunk_position: [i32; 3]) -> Option<(usize, usize)> {
		self.chunks_octrees.get(&chunk_position).cloned()
	}

	pub fn insert_volume(&mut self, queue: &wgpu::Queue, entity: EntityId, volume: TypedArrayVoxelVolume<Voxel>) -> (usize, usize) {
		if let Some(i) = self.entity_array_volumes.get(&entity).cloned() {
			debug!("Replacing volume data for entity {entity:?}");
			let (st, en) = self.array_volumes[i];
			self.buffer.remove(st..en);
		}
		
		// Prepare data
		let header = ArrayVolumeHeader::new(volume.size);
		let mapped_conents = volume.contents.iter()
			.map(|v| v.block_encode().unwrap())
			.map(|e| e as u32)
			.collect::<Vec<_>>();
		let mut bytes = Vec::new();
		bytes.extend_from_slice(bytemuck::bytes_of(&header));
		bytes.extend_from_slice(bytemuck::cast_slice(&mapped_conents[..]));

		// Feed data
		let [st, en] = self.buffer.insert_direct(queue, &bytes[..]);
		let idx = self.array_volumes.insert((st, en));
		self.entity_array_volumes.insert(entity, idx);

		(st, en)
	}
	pub fn get_volume(&self, entity: EntityId) -> Option<(usize, usize)> {
		self.entity_array_volumes.get(&entity).and_then(|&i| self.array_volumes.get(i).cloned())
	}

	pub fn update_colours(&mut self, queue: &wgpu::Queue, blocks: &BlockManager, materials: &MaterialManager, textures: &TextureManager) {
		// Pull colours after end of self, reallocate colour buffer and change location
		let new_stuff = blocks.colours(materials, textures, self.colours.len());
		if new_stuff.len() == 0 {
			return;
		}

		let g = new_stuff.iter()
			.map(|f| {
				let g: [u8; 4] = (0..4).map(|i| (f[i] * u8::MAX as f32).floor() as u8).collect::<Vec<_>>().try_into().unwrap();
				g
			});
		self.colours.extend(g);

		let (st, en) = self.colours_idx;
		self.buffer.remove(st..en);

		let cs = self.colours.iter().cloned().flatten().collect::<Vec<_>>();
		let [ns, ne] = self.buffer.insert_direct(queue, &cs[..]);
		self.colours_idx = (ns, ne);

		// println!("{:?}", cs);
		// panic!();
		println!("Colours:");
		for (i, c) in self.colours.iter().enumerate() {
			println!("{i} = {c:?}")
		}
	}

	pub fn update_uniform(&mut self, queue: &wgpu::Queue, vrc: &VoxelRenderingComponent) {
		queue.write_buffer(&self.vrc_buffer, 0, &vrc.to_uniform_contents()[..]);
	}
}



pub fn voxel_render_system(
	gpu: UniqueView<GraphicsHandleResource>,
	mut voxel_data: UniqueViewMut<VoxelRenderingResource>,
	map: UniqueView<MapResource>,
	cameras: View<CameraComponent>,
	transforms: View<TransformComponent>,
	vvolumes: View<ArrayVolumeComponent>,
	mut voxel_infos: ViewMut<VoxelRenderingComponent>,
) {
	// Other systems load voxel data
	// Just find visible stuff for camera
	for (_camera, transform, voxel_info) in (&cameras, &transforms, &mut voxel_infos).iter() {
		
		// Fetch voxel volume data (obb, st of entry (header, data))
		let (ost, oen) = voxel_info.array_volumes;
		if ost != oen {
			voxel_data.buffer.remove(ost..oen); // Remove old volume data
		}
		let mut view_data_entries = 0;
		let mut view_data = Vec::new();
		for (eid, (volume, transform)) in (&vvolumes, &transforms).iter().with_id() {
			if let Some((st, _en)) = voxel_data.get_volume(eid) {
				// Todo: cull
				let [aabb_min, aabb_max] = volume.volume.aabb_extent();
				let matrix = transform.rotation * Translation3::from(transform.position);
				let obb = VolumeOBB {
					aabb_max,
					aabb_min,
					matrix: matrix.to_homogeneous().into(),
				};
				view_data.extend_from_slice(bytemuck::bytes_of(&obb));
				view_data.extend_from_slice(bytemuck::bytes_of(&st));
				view_data_entries += 1;
			} else {
				warn!("Entity {eid:?} has unload vvolume");
			}
		}
		let [st, en] = voxel_data.buffer.insert_direct(&gpu.queue, &view_data[..]);
		voxel_info.n_array_volumes = view_data_entries;
		voxel_info.array_volumes = (st, en);


		// Colours are shared
		voxel_info.colours_idx = voxel_data.colours_idx;

		
		// Fetch map data
		// We will do this every frame for convinience (it's not much data)
		let (ost, oen) = voxel_info.chunk_as_idx;
		if ost != oen {
			voxel_data.buffer.remove(ost..oen); // Remove old chunk as data
		}
		let centre = map.map.point_chunk(&transform.position);
		let llc = [
			centre[0] - voxel_info.chunk_as_header.size[0] as i32 / 2,
			centre[1] - voxel_info.chunk_as_header.size[1] as i32 / 2,
			centre[2] - voxel_info.chunk_as_header.size[2] as i32 / 2,
		];
		voxel_info.chunk_as_centre = centre;
		voxel_info.chunk_size = map.map.chunk_size[0]; // This should not ever change but I'm still paranoid
		let mut chunk_sts = Vec::new();
		let [sx, sy, sz, _] = voxel_info.chunk_as_header.size;
		for x in 0..sx {
			for y in 0..sy {
				for z in 0..sz {

					let cpos = [llc[0] + x as i32, llc[1] + y as i32, llc[2] + z as i32];

					if let Some(bounds) = voxel_data.get_chunk(cpos) {
						chunk_sts.push(bounds.0 as u32 + 1);
					} else {
						chunk_sts.push(0);
					}
				}
			}
		}
		let [st, en] = voxel_data.buffer.insert_direct(&gpu.queue, bytemuck::cast_slice(&chunk_sts[..]));
		voxel_info.chunk_as_idx = (st, en);
	}

	// info!("Buffer is at {:.2}% capacity", voxel_data.buffer.capacity_frac() * 100.0);
}



#[derive(Component, Debug, Clone)]
pub struct CameraComponent {
	pub fovy: f32, // In radians, don't forget
	pub near: f32,
	pub far: f32,
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			fovy: 45.0_f32.to_radians(),
			near: Self::near_from_fovy_degrees(45.0),
			far: 100.0,
		}
	}

	pub fn with_fovy_degrees(self, degrees: f32) -> Self {
		Self {
			fovy: degrees.to_radians(),
			near: Self::near_from_fovy_degrees(degrees),
			..self
		}
	}

	pub fn with_far(self, far: f32) -> Self {
		Self {
			far,
			..self
		}
	}

	fn near_from_fovy_degrees(fovy: f32) -> f32 {
		1.0 / (fovy.to_radians() / 2.0).tan()
	}

	pub fn set_fovy(&mut self, degrees: f32) {
		self.fovy = degrees.to_radians();
		self.near = Self::near_from_fovy_degrees(self.fovy);
	}

	pub fn rendercamera(&self, transform: &TransformComponent) -> RenderCamera {
		RenderCamera::new(transform.position, transform.rotation, self.fovy.to_degrees(), self.near, self.far)
	}
}


#[derive(Unique, Debug)]
pub struct PolygonRenderingResource {
	// graphs?
	// fugg buffer!
}
#[derive(Component, Debug)]
pub struct PolygonRenderingComponent {
	// Indices should be bound indices, but I don't have that accessible to the ecs right now
	// We could also include interpolation information
	pub models: Vec<(Index, bool, TransformComponent)>, // Mesh, material (unused for now), transform
}

pub fn polygon_render_system(
	gpu: UniqueView<GraphicsHandleResource>,
	cameras: View<CameraComponent>,
	transforms: View<TransformComponent>,
	models: View<ModelComponent>,
	mut infos: ViewMut<PolygonRenderingComponent>,
) {
	for (_camera, _transform, info) in (&cameras, &transforms, &mut infos).iter() {

		info.models.clear();
		for (model, transform) in (&models, &transforms).iter() {
			let (mesh, _) = model.mesh;
			

			info.models.push((mesh, false, transform.clone()));
		}
	}
}



// To be used with a texture rendering system
// Attach to a camera with rendering components
#[derive(Component, Debug)]
struct TextureRenderComponent {
	pub texture_id: usize,
}



#[derive(Component, Debug)]
pub struct ModelComponent {
	// Includes name of mesh/material for debugging and maybe reloading
	pub mesh: (Index, String),
	pub material: Option<(Index, String)>,
}


// If something uses this it is expected that its mesh is skinned and its shader uses this stuff
#[derive(Component, Debug)]
pub struct SkeletonComponent {
	// Base bone positions
	pub bones: Vec<Matrix4<f32>>,
}

#[derive(Component, Debug)]
pub struct ArrayVolumeComponent {
	pub volume: TypedArrayVoxelVolume<Voxel>,
}

#[derive(Component, Debug)]
pub struct SkeletalAttachmentComponent {
	pub entity: EntityId,
	pub bone: usize,
}

#[derive(Component, Debug)]
/// A straight line between two points.
/// Usually accompanied by a RenderMarkerComponent.
/// Might be accompanied by a LifetimeComponent.
pub struct SimpleLineComponent {
	pub start: Point3<f32>,
	pub end: Point3<f32>,
}

#[derive(Component, Debug)]
/// A marker to remove this entity after a point in time.
pub struct LifetimeComponent {
	pub expiry: Instant,
}




// /// For each camera gets the stuff that should be rendered
// // Todo: Buffer the instances to let renderer render independently
// pub struct RenderDataSystem;
// impl<'a> System<'a> for RenderDataSystem {
// 	type SystemData = (
// 		ReadStorage<'a, ModelComponent>,
// 		ReadStorage<'a, MapComponent>,
// 		WriteStorage<'a, CameraComponent>,
// 		ReadStorage<'a, TransformComponent>,
// 	);

// 	fn run(
// 		&mut self, 
// 		(
// 			models,
// 			maps,
// 			mut cameras,
// 			transforms,
// 		): Self::SystemData,
// 	) { 
// 		for (camera, _camera_transform) in (&mut cameras, &transforms).join() {
			
// 			let mut render_data = Vec::new();
// 			// Models
// 			for (model_c, transform_c) in (&models, &transforms).join() {
// 				let instance = Instance::new()
// 					.with_position(transform_c.position)
// 					.with_rotation(transform_c.rotation);
// 				let model_instance = ModelInstance {
// 					material_idx: model_c.material_idx,
// 					mesh_idx: model_c.mesh_idx,
// 					instance,
// 				};
// 				render_data.push(model_instance);
// 			}
// 			// Map chunks
// 			// Todo: rotation
// 			for (map_c, transform_c) in (&maps, &transforms).join() {
// 				// Renders ALL available chunks
// 				for (cp, entry) in &map_c.chunk_models {
// 					let mesh_mats = match entry {
// 						ChunkModelEntry::Complete(mesh_mats) => Some(mesh_mats),
// 						ChunkModelEntry::ReModeling(mesh_mats, _) => Some(mesh_mats),
// 						_ => None,
// 					};
// 					if let Some(mesh_mats) = mesh_mats {
// 						let position = transform_c.position + map_c.map.chunk_point(*cp);
// 						let instance = Instance::new().with_position(position);
// 						for (mesh_idx, material_idx) in mesh_mats.iter().cloned() {
// 							let model_instance = ModelInstance {
// 								material_idx,
// 								mesh_idx,
// 								instance,
// 							};
// 							render_data.push(model_instance);
// 						}
// 					}
// 				}
// 			}

// 			camera.render_data = render_data;
// 		}
// 	}
// }
