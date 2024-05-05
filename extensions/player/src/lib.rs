use ekstensions::{eks::entity::EntitySparseSet, prelude::*};
use transform::TransformComponent;

#[macro_use]
extern crate log;



#[derive(Debug, Resource, Default)]
pub struct PlayerSpawnResource {
	pub entities: EntitySparseSet,
}


/// Marks the beginning of spawning players. 
fn player_spawn(psr: Res<PlayerSpawnResource>) {
	if psr.entities.len() > 0 {
		info!("Spawning {} players", psr.entities.len());
	}
}


/// Marks the end of spawning players. 
fn player_spawned(mut psr: ResMut<PlayerSpawnResource>) {
	if psr.entities.len() > 0 {
		info!("{} players have been spawned", psr.entities.len());
	}
	psr.entities.clear();
}


/// An example system that is smooshed between `players_spawn` and `players_spawned`. 
/// This adds multiple components, but once could have a seperate systemf or each component. 
fn player_spawn_components(
	psr: Res<PlayerSpawnResource>,
	mut transforms: CompMut<TransformComponent>,
) {
	for entity in psr.entities.iter().copied() {
		trace!("Add TransformComponent for player entity");
		transforms.insert(entity, TransformComponent::new());
	}
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn dependencies() -> Vec<String> {
	vec![]
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("client_tick", "player_spawn", player_spawn);
	loader.system("client_tick", "player_spawned", player_spawned)
		.run_after("player_spawn");
	// Smooshed system is run between the other two
	loader.system("client_tick", "player_spawn_components", player_spawn_components)
		.run_after("player_spawn")
		.run_before("player_spawned");
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	storages.resource(PlayerSpawnResource::default());
}
