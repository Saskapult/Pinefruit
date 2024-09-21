use std::collections::HashSet;
use crate::controls::ControlMap;
use eeks::prelude::*;
use crate::transform::{MovementComponent, TransformComponent};


// This coul also be accomplished using a component 
#[derive(Debug, Resource, Default)]
pub struct PlayerSpawnResource {
	pub entities: HashSet<Entity>,
}


/// Marks the beginning of spawning players. 
pub fn player_spawn(psr: Res<PlayerSpawnResource>) {
	if psr.entities.len() > 0 {
		info!("Spawning {} players", psr.entities.len());
	}
}


/// Marks the end of spawning players. 
pub fn player_spawned(mut psr: ResMut<PlayerSpawnResource>) {
	if psr.entities.len() > 0 {
		info!("{} players have been spawned", psr.entities.len());
	}
	psr.entities.clear();
}


/// An example system that is smooshed between `players_spawn` and `players_spawned`. 
/// This adds multiple components, but once could have a seperate systemf or each component. 
pub fn player_spawn_components(
	psr: Res<PlayerSpawnResource>,
	mut transforms: CompMut<TransformComponent>,
	mut movements: CompMut<MovementComponent>,
	mut control_map: ResMut<ControlMap>,
) {
	for entity in psr.entities.iter().copied() {
		trace!("Add TransformComponent for player entity");
		transforms.insert(entity, TransformComponent::new());

		trace!("Add MovementComponent for player entity");
		movements.insert(entity, MovementComponent::new(&mut control_map));
	}
}
