pub mod model;

use ekstensions::prelude::*;
use light::light::{torchlight_chunk_init_system, torchlight_update_system, TorchLightChunksResource};

#[macro_use]
extern crate log;



// #[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
// pub struct ComponentA(u32);


#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![
		// "libidk".into(),
	]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	// loader.system("client_tick", "torchlight_chunk_init_system", torchlight_chunk_init_system);
		// .run_before("example0_client_init_after")
		// .run_after("example0_client_init_before");
	// loader.system("client_tick", "torchlight_update_system", torchlight_update_system);
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	// storages.resource(TorchLightChunksResource::default());

	// p.component::<ComponentA>();
}


// #[cfg_attr(feature = "extension", no_mangle)]
// pub fn unload() {}


// #[cfg_attr(feature = "extension", no_mangle)]
// pub fn init(
// 	_a: Comp<ComponentA>,
// ) {
// 	info!("Example0 init system");
// }
