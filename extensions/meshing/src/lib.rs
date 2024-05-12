pub mod model;

use ekstensions::prelude::*;
use model::{map_rendering_system, map_modelling_system, model_wipe_system, MapMeshingComponent, MapModelResource};
use player::PlayerSpawnResource;


#[macro_use]
extern crate log;


fn player_meshing_component(
	psr: Res<PlayerSpawnResource>,
	mut meshings: CompMut<MapMeshingComponent>,
) {
	for entity in psr.entities.iter().copied() {
		meshings.insert(entity, MapMeshingComponent::new(4, 2));
	}
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("client_tick", "player_meshing_component", player_meshing_component)
		.run_after("player_spawn")
		.run_before("player_spawned");

	loader.system("client_tick", "model_wipe_system", model_wipe_system);

	loader.system("client_tick", "map_modelling_system", map_modelling_system)
		.run_after("terrain_loading_system")
		.run_after("torchlight_update_system");

	loader.system("render", "map_rendering_system", map_rendering_system);
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	storages.resource(MapModelResource::new(8));
	storages.component::<MapMeshingComponent>();
}
