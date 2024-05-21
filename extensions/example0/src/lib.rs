use ekstensions::prelude::*;

#[macro_use]
extern crate log;



#[info]
pub fn info() -> Vec<String> {
	env_logger::init();
	info!("Example0 deps");
	vec![]
}


#[systems]
pub fn systems(_loader: &mut ExtensionSystemsLoader) {
	info!("Example0 systems");
}


#[load]
pub fn load(_storages: &mut ekstensions::ExtensionStorageLoader) {
	info!("Example0 load");
}
