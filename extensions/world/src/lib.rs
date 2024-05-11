// pub mod modification;
// pub mod octree;
// pub mod model; 
// pub mod looking;
// pub mod liquids;
// pub mod light;
// pub mod chunks;
// pub mod terrain;

use ekstensions::prelude::*;

#[macro_use]
extern crate log;


#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![
		// "libidk".into(),
	]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(_loader: &mut ExtensionSystemsLoader) {
	
	// loader.system("client_init", "example0_client_init", init);
		// .run_before("example0_client_init_after")
		// .run_after("example0_client_init_before");
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(_storages: &mut ekstensions::ExtensionStorageLoader) {

	// p.component::<ComponentA>();
}
