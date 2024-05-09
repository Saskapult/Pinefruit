use ekstensions::prelude::*;

#[macro_use]
extern crate log;



// #[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
// pub struct ComponentA(u32);


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	info!("Example0 deps");
	vec![
		// "libidk".into(),
	]
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn systems(_loader: &mut ExtensionSystemsLoader) {
	info!("Example0 systems");
	
	// loader.system("client_init", "example0_client_init", init);
		// .run_before("example0_client_init_after")
		// .run_after("example0_client_init_before");
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn load(_storages: &mut ekstensions::ExtensionStorageLoader) {
	info!("Example0 load");

	// p.component::<ComponentA>();
}


// #[cfg_attr(not(feature = "no_export"), no_mangle)]
// pub fn unload() {}


// #[cfg_attr(not(feature = "no_export"), no_mangle)]
// pub fn init(
// 	_a: Comp<ComponentA>,
// ) {
// 	info!("Example0 init system");
// }
