pub mod model;

use eeks::prelude::*;
use model::{chunk_bounds_rendering_system, map_modelling_system, map_rendering_system, model_wipe_system, MapMeshingComponent, MapModelResource};
use pinecore::player::PlayerSpawnResource;


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


#[info]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![]
}


#[systems]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("client_tick", "player_meshing_component", player_meshing_component)
		.run_after("player_spawn")
		.run_before("player_spawned");

	loader.system("client_tick", "model_wipe_system", model_wipe_system);

	loader.system("client_tick", "map_modelling_system", map_modelling_system)
		.run_after("terrain_loading_system")
		.run_after("torchlight_update_system");

	loader.system("render", "map_rendering_system", map_rendering_system);
	loader.system("render", "chunk_bounds_rendering_system", chunk_bounds_rendering_system);
}


#[load]
pub fn load(storages: &mut eeks::ExtensionStorageLoader) {
	storages.resource(MapModelResource::new(8));
	storages.component::<MapMeshingComponent>();
}
