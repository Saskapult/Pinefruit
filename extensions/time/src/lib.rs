use ekstensions::prelude::*;
use render::{BufferResource, QueueResource};
use std::time::Instant;
use krender::prelude::Buffer;

#[macro_use]
extern crate log;



#[derive(Debug, Clone, Copy, Resource)]
pub struct TimeResource {
	pub start: Instant,
	// Tick info?
}
impl TimeResource {
	pub fn new() -> Self {
		Self {
			start: Instant::now(),
		}
	}
}


/// Updates a time buffer on the GPU
pub fn time_buffer_system(
	queue: Res<QueueResource>,
	time: Res<TimeResource>,
	mut buffers: ResMut<BufferResource>,
) {
	#[repr(C)]
	#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
	struct TimeBuffer {
		pub time: f32,
	}

	let tb = TimeBuffer {
		time: time.start.elapsed().as_secs_f32(),
	};
	
	let k = buffers.key("time").unwrap_or_else(|| {
		debug!("Inserting time buffer");
		buffers.insert(Buffer::new_init("time", bytemuck::bytes_of(&tb), false, true, false))
	});

	let b = buffers.get_mut(k).unwrap();
	b.write(&queue, 0, bytemuck::bytes_of(&tb));
}


#[info]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![]
}


#[systems]
pub fn systems(loader: &mut ExtensionSystemsLoader) {	
	loader.system("client_tick", "time_buffer_system", time_buffer_system);
}


#[load]
pub fn load(storages: &mut ekstensions::ExtensionStorageLoader) {
	storages.resource(TimeResource::new());
}
