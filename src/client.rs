use ekstensions::prelude::*;


// Called GameInstance, but used as Client
// We can adapt it to a server later
// Maybe we can have a trait for client and server, and then have client/server setup/tick
pub struct GameInstance {
	pub extensions: ExtensionRegistry,
	pub world: World,
}
impl GameInstance {
	pub fn new() -> Self {
		let mut world = World::new();
		let mut extensions = ExtensionRegistry::new();

		extensions.register_all_in("extensions").unwrap();

		let s = extensions.native_systems();


		// Register native systems
		
		// s.system("client_tick", name, function)

		extensions.reload(&mut world).unwrap();

		extensions.run(&mut world, "client_init");

		Self { extensions, world, }
	}

	pub fn tick(&mut self) {
		// Get time resource, tick that to here
		// Maybe we can have a tick count, but it can skip values if too much time has passed
		// idk idk

		self.extensions.run(&mut self.world, "client_tick");
	}

	pub fn connect_server(&mut self) {
		// Connect to local server by adding some storages
		// Connect to external server by adding some other storages
		// Maybe have different methods for each
		todo!()
	}
}

// Maybe just fn creat client -> Arc<Mutex<World>>
// Still needs to register native components and stuff!!


