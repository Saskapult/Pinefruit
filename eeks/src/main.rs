use eks::World;
use eeks::prelude::*;

#[macro_use]
extern crate log;


fn main() {
	env_logger::init();

	// Make registry
	let mut registry = ExtensionRegistry::new();
	let mut world = World::new();

	eeks::load_extensions!(world, registry).unwrap();
	registry.reload(&mut world, |_| {}).unwrap();
	
	for line in std::io::stdin().lines() {
		let line = line.unwrap();
		let parts = line.split(" ").collect::<Vec<_>>();
		if parts.is_empty() { continue }
		let first = *parts.get(0).unwrap();
		match first {
			"reload" => {
				registry.reload(&mut world, |_| {}).unwrap();
			},
			"run" => if let Err(e) = registry.run(&mut world, parts[1]) {
				error!("{}", e);
			},
			"exit" => {
				break;
			},
			"command" => if let Err(e) = registry.command(&mut world, &parts[1..]) {
				error!("{}", e);
			}
			_ => {
				error!("Invalid command '{}'", parts[0]);
			},
		}
	}
}
