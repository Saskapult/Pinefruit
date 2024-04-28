use std::time::Instant;
use ekstensions::prelude::*;
use krender::prelude::Buffer;
use crate::game::{BufferResource, QueueResource};



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
