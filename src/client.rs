use std::sync::Arc;
use ekstensions::prelude::*;
use parking_lot::RwLock;


// Used in here and in server, remane to GameInstance? 
pub struct Client {
	pub extensions: Arc<RwLock<ExtensionRegistry>>,
	pub world: World,
}
impl Client {
	pub fn new(
		extensions: Arc<RwLock<ExtensionRegistry>>
	) -> Self {
		let mut world = World::new();

		extensions.write().reload(&mut world).unwrap();

		// You need to add base fucntion to this! 
		// We don't want to do everyhting through extensions
		extensions.read().run(&mut world, "client_init");

		Self { extensions, world, }
	}

	pub fn tick(&mut self) {
		// Get time resource, tick that to here
		// Maybe we can have a tick count, but it can skip values if too much time has passed
		// idk idk

		self.extensions.read().run(&mut self.world, "client_tick");

		// Send input to server 
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


