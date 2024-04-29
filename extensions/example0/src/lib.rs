use ekstensions::prelude::*;


#[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
pub struct ComponentA(u32);


#[no_mangle]
pub fn dependencies() -> Vec<String> {
	println!("Example0 deps");
	vec![
		// "libidk".into(),
	]
}


#[no_mangle]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	println!("Example0 systems");
	
	loader.system("client_init", "example0_client_init", init);
		// .run_before("example0_client_init_after")
		// .run_after("example0_client_init_before");
}


#[no_mangle]
pub fn load(p: &mut ekstensions::ExtensionStorageLoader) {
	println!("Example0 load");

	p.component::<ComponentA>();
}


// #[no_mangle]
// pub fn unload() {}


#[no_mangle]
pub fn init(
	_a: Comp<ComponentA>,
) {
	println!("Example0 init system");
}
