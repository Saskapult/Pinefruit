use ekstensions::eks::prelude::*;


#[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
pub struct ComponentA(u32);


// Replace with general "info" function? 
// I don't see why we'd need to do that actually 
#[no_mangle]
pub fn dependencies() -> Vec<String> {
	println!("Example0 deps");
	vec![
		// "libidk".into(),
	]
}


#[no_mangle]
pub fn load(p: &mut ekstensions::ExtensionLoader) {
	println!("Example0 load");

	p.component::<ComponentA>();

	// Needs unique name tho
	p.system("init", "init_system", init);
}


#[no_mangle]
pub fn unload() -> bool {
	true
}

#[no_mangle]
pub fn init(
	_a: Comp<ComponentA>,
) {
	println!("Init system");
}


#[no_mangle]
pub fn add(left: usize, right: usize) -> usize {
	left + right
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn it_works() {
		let result = add(2, 2);
		assert_eq!(result, 4);
	}
}
