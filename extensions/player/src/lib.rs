use ekstensions::{eks::entity::EntitySparseSet, prelude::*};

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
fn players_spawned(mut psr: ResMut<PlayerSpawnResource>) {
	if psr.entities.len() > 0 {
		info!("{} players have been spawned", psr.entities.len());
	}
	psr.entities.clear();
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn dependencies() -> Vec<String> {
	vec![]
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("client_tick", "player_spawn", player_spawn);
	loader.system("client_tick", "players_spawned", players_spawned)
		.run_after("player_spawn");
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	storages.resource(PlayerSpawnResource::default());
}
