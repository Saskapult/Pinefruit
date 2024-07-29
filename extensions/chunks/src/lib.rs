pub mod array_volume;
pub mod blocks;
pub mod chunk;
pub mod chunks;
pub mod fvt;
pub mod generation;

use blocks::BlockResource;
use chunks::{chunk_loading_system, ChunkLoadingComponent, ChunksResource};
use eeks::prelude::*;
use glam::{Vec3, IVec3, UVec3};
use player::PlayerSpawnResource;

#[macro_use]
extern crate log;



/// Chunk side length. 
/// Determines the chunk extent for the whole project! 
/// It's here so I can replace it with 32 to test things. 
/// 
/// Hopefully I used this instead of just plugging in numbers...
pub const CHUNK_SIZE: u32 = 32;


pub fn voxel_of_point(point: Vec3) -> IVec3 {
	point.floor().as_ivec3()
}


pub fn voxel_relative_to_chunk(voxel: IVec3, chunk: IVec3) -> IVec3 {
	let cv = chunk * IVec3::new(CHUNK_SIZE as i32, CHUNK_SIZE as i32, CHUNK_SIZE as i32);
	voxel - cv
}


pub fn chunk_of_voxel(voxel: IVec3) -> IVec3 {
	IVec3::new(
		voxel.x.div_euclid(CHUNK_SIZE as i32),
		voxel.y.div_euclid(CHUNK_SIZE as i32),
		voxel.z.div_euclid(CHUNK_SIZE as i32),
	)
}


pub fn chunk_of_point(point: Vec3) -> IVec3 {
	chunk_of_voxel(voxel_of_point(point))
}


pub fn cube_iterator_xyz_uvec(size: UVec3) -> impl Iterator<Item = UVec3> {
	let [x, y, z] = size.to_array();
	(0..x).flat_map(move |x| {
		(0..y).flat_map(move |y| {
			(0..z).map(move |z| {
				UVec3::new(x, y, z)
			})
		})
	})
} 


#[derive(Debug)]
pub struct VoxelSphere {
	pub centre: IVec3,
	pub radius: i32,
}
impl VoxelSphere {
	pub fn new(centre: IVec3, radius: i32) -> Self {
		Self { centre, radius, }
	}

	pub fn is_within(&self, position: IVec3) -> bool {
		let x = position.x - self.centre.x;
		let y = position.y - self.centre.y;
		let z = position.z - self.centre.z;
		(x.pow(2) + y.pow(2) + z.pow(2)) <= self.radius.pow(2)
	}

	pub fn iter(&self) -> impl IntoIterator<Item = IVec3> + '_ {
		((self.centre.x-self.radius)..=(self.centre.x+self.radius)).flat_map(move |x|
			((self.centre.y-self.radius)..=(self.centre.y+self.radius)).flat_map(move |y|
				((self.centre.z-self.radius)..=(self.centre.z+self.radius)).map(move |z| IVec3::new(x, y, z))))
					.filter(|&position| self.is_within(position))
	}
}


#[derive(Debug)]
pub struct VoxelCube {
	pub centre: IVec3,
	pub half_edge_length: UVec3,
}
impl VoxelCube {
	pub fn new(centre: IVec3, half_edge_length: UVec3) -> Self {
		Self { centre, half_edge_length, }
	}

	/// Expands the half edge length by a value
	pub fn expand(mut self, by: UVec3) -> Self {
		self.half_edge_length += by;
		self
	}

	pub fn edge_length(&self) -> UVec3 {
		self.half_edge_length * 2 + UVec3::ONE
	}

	pub fn min(&self) -> IVec3 {
		self.centre - self.half_edge_length.as_ivec3()
	}

	pub fn max(&self) -> IVec3 {
		self.centre + self.half_edge_length.as_ivec3()
	}

	pub fn contains(&self, postion: IVec3) -> bool {
		(postion - self.centre).abs().as_uvec3().cmplt(self.half_edge_length + 1).all()
	}

	pub fn iter(&self) -> impl IntoIterator<Item = IVec3> + '_ {
		let base_pos = self.min();
		cube_iterator_xyz_uvec(self.edge_length())
			.map(move |p| base_pos + p.as_ivec3() )
	}
}


fn player_chunk_loader(
	psr: Res<PlayerSpawnResource>,
	mut loaders: CompMut<ChunkLoadingComponent>,
) {
	for entity in psr.entities.iter().copied() {
		debug!("Insert chunk loding component for player");
		loaders.insert(entity, ChunkLoadingComponent::new(5));
	}
}


#[info]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![]
}


#[systems]
pub fn systems(loader: &mut ExtensionSystemsLoader) {	
	loader.system("client_tick", "chunk_loading_system", chunk_loading_system);
	
	loader.system("client_tick", "player_chunk_loader", player_chunk_loader)
		.run_after("player_spawn")
		.run_before("player_spawned");
}


#[load]
pub fn load(storages: &mut ExtensionStorageLoader) {
	storages.resource(ChunksResource::new());
	storages.resource(BlockResource::default());
	storages.component::<ChunkLoadingComponent>();
}
