#![feature(test)]

pub mod generator;
pub mod modification;
pub mod terrain;

use controls::ControlMap;
use eeks::prelude::*;
use modification::{terrain_modification_application, terrain_placement_queue, VoxelModifierComponent};
use player::PlayerSpawnResource;
use terrain::{terrain_loading_system, TerrainLoadingResource, TerrainResource};
use transform::TransformComponent;

#[macro_use]
extern crate log;

extern crate test;



fn player_terrain_modifier(
	psr: Res<PlayerSpawnResource>,
	mut controls: ResMut<ControlMap>,
	mut vm: CompMut<VoxelModifierComponent>,
) {
	for entity in psr.entities.iter().copied() {
		vm.insert(entity, VoxelModifierComponent::new(&mut controls));
	}
}


fn place_player_at_terrain_height(
	psr: Res<PlayerSpawnResource>,
	mut transforms: CompMut<TransformComponent>,
) {
	for entity in psr.entities.iter().copied() {
		let t = transforms.get_mut(entity).unwrap();
		warn!("TODO: set player y to terrain height");
		t.translation.y = 42.0;
	}
}



#[info]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![
		"chunks".into(),
	]
}


#[systems]
pub fn systems(loader: &mut ExtensionSystemsLoader) {	
	loader.system("client_tick", "terrain_loading_system", terrain_loading_system)
		.run_after("chunk_loading_system");

	loader.system("client_tick", "terrain_placement_queue", terrain_placement_queue);

	loader.system("client_tick", "terrain_modification_application", terrain_modification_application)
		.run_after("terrain_placement_queue");

	loader.system("client_tick", "player_terrain_modifier", player_terrain_modifier)
		.run_after("player_spawn")
		.run_before("player_spawned");
	loader.system("client_tick", "place_player_at_terrain_height", place_player_at_terrain_height)
		.run_after("player_spawn_components")
		.run_after("player_spawn")
		.run_before("player_spawned");
}


#[load]
pub fn load(storages: &mut eeks::ExtensionStorageLoader) {
	storages.component::<VoxelModifierComponent>();
	storages.resource(TerrainLoadingResource::new(0));
	storages.resource(TerrainResource::default());
}
