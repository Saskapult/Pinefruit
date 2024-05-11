pub mod generator;
pub mod modification;
pub mod terrain;

use ekstensions::prelude::*;
use modification::{terrain_modification_application, terrain_placement_queue, VoxelModifierComponent};

#[macro_use]
extern crate log;



#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![
		"chunks".into(),
	]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	
	// loader.system("client_tick", "terrain_placement_queue", terrain_placement_queue);
	// loader.system("client_tick", "terrain_modification_application", terrain_modification_application)
	// 	.run_after("terrain_placement_queue");
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	// storages.component::<VoxelModifierComponent>();
}
