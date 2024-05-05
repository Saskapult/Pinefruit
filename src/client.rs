use std::sync::Arc;

use ekstensions::prelude::*;
use krender::prelude::{BindGroupManager, BufferManager, MaterialManager, MeshManager, ShaderManager, TextureManager};
use render::{BufferResource, ContextResource, DeviceResource, MaterialResource, MeshResource, QueueResource, TextureResource};


// Called GameInstance, but used as Client
// We can adapt it to a server later
// Maybe we can have a trait for client and server, and then have client/server setup/tick
pub struct GameInstance {
	pub extensions: ExtensionRegistry,
	pub world: World,

	pub shaders: ShaderManager,
	pub bind_groups: BindGroupManager,
}
impl GameInstance {
	// I'm thinking that this should be agnostic to whether the instance will be a client or a server
	// Please make note if that changes
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		let mut world = World::new();

		// Intialize render storages
		world.insert_resource(DeviceResource(device.clone()));
		world.insert_resource(QueueResource(queue.clone()));

		// Could (should?) be in render extension
		world.insert_resource(ContextResource::default());
		world.insert_resource(MaterialResource(MaterialManager::new()));
		world.insert_resource(TextureResource(TextureManager::new()));
		world.insert_resource(BufferResource(BufferManager::new()));
		world.insert_resource(MeshResource(MeshManager::new()));

		// Not directly affected by ECS, but is indirectly affected, so lives here
		let shaders = ShaderManager::new();
		let bind_groups = BindGroupManager::new();

		let mut extensions = ExtensionRegistry::new();
		extensions.register_all_in("extensions").unwrap();
		extensions.reload(&mut world).unwrap();

		if let Err(e) = extensions.run(&mut world, "client_init") {
			warn!("Error running 'client_init': {}", e);
		}

		Self { extensions, world, shaders, bind_groups, }
	}

	pub fn tick(&mut self) {
		// Get time resource, tick that to here
		// Maybe we can have a tick count, but it can skip values if too much time has passed
		// idk idk

		self.extensions.run(&mut self.world, "client_tick").unwrap();
	}

	pub fn connect_server(&mut self) {
		// Connect to local server by adding some storages
		// Connect to external server by adding some other storages
		// Maybe have different methods for each
		todo!()
	}
}
impl Drop for GameInstance {
	fn drop(&mut self) {
		// Storage drop functions reference external library code
		// Maybe we could fix this using an Arc, but that mgiht not help if the extension code file itself is overwritten 
		self.world.clear();
	}
}
