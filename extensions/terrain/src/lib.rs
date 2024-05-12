pub mod generator;
pub mod modification;
pub mod terrain;

use controls::{ControlComponent, ControlMap};
use ekstensions::prelude::*;
use modification::{terrain_modification_application, terrain_placement_queue, VoxelModifierComponent};
use player::PlayerSpawnResource;
use terrain::{terrain_loading_system, TerrainLoadingResource, TerrainResource};

#[macro_use]
extern crate log;



fn player_terrain_modifier(
	psr: Res<PlayerSpawnResource>,
	mut controls: ResMut<ControlMap>,
	mut vm: CompMut<VoxelModifierComponent>,
) {
	for entity in psr.entities.iter().copied() {
		vm.insert(entity, VoxelModifierComponent::new(&mut controls));
	}
}



#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![
		"chunks".into(),
	]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {	
	loader.system("client_tick", "terrain_loading_system", terrain_loading_system)
		.run_after("chunk_loading_system");

	loader.system("client_tick", "terrain_placement_queue", terrain_placement_queue);

	loader.system("client_tick", "terrain_modification_application", terrain_modification_application)
		.run_after("terrain_placement_queue");

	loader.system("client_tick", "player_terrain_modifier", player_terrain_modifier)
		.run_after("player_spawn")
		.run_before("player_spawned");
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	storages.component::<VoxelModifierComponent>();
	storages.resource(TerrainLoadingResource::new(0));
	storages.resource(TerrainResource::default());
}
