// use eks::World;
// use ekstensions::ExtensionRegistry;

// #[macro_use]
// extern crate log;


fn main() {
	// env_logger::init();

	// // Make registry
	// let mut registry = ExtensionRegistry::new();
	// let mut world = World::new();

	// // Load exerything in ./extensions 
	// registry.register("extensions/example0").unwrap();
	// registry.reload(&mut world, |_| {}).unwrap();
	
	// for line in std::io::stdin().lines() {
	// 	let line = line.unwrap();
	// 	let parts = line.split(" ").collect::<Vec<_>>();
	// 	match parts[0] {
	// 		"reload" => {
	// 			registry.reload(&mut world, |_| {}).unwrap();
	// 		},
	// 		"run" => {
	// 			// registry.test_run(&mut world);
	// 			if let Err(e) = registry.run(&mut world, parts[1]) {
	// 				error!("{}", e);
	// 			}
	// 		},
	// 		"exit" => {
	// 			break;
	// 		},
	// 		_ => {
	// 			error!("Invalid command '{}'", parts[0]);
	// 		},
	// 	}
	// }
}
