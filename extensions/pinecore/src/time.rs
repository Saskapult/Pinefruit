use eeks::prelude::*;
use crate::render::{BufferResource, QueueResource};
use std::time::Instant;
use krender::prelude::Buffer;


#[derive(Debug, Clone, Copy, Resource)]
pub struct TimeResource {
	pub start: Instant,
	/// The seconds that have passed since 'start'
	pub tick_time: f32,
}
impl TimeResource {
	pub fn new() -> Self {
		Self {
			start: Instant::now(),
			tick_time: 0.0,
		}
	}
}


pub fn time_update_system(
	mut time: ResMut<TimeResource>,
) {
	time.tick_time = time.start.elapsed().as_secs_f32();
}


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
	
	let k = buffers.key_of("time").unwrap_or_else(|| {
		debug!("Inserting time buffer");
		buffers.insert(Buffer::new_init("time", bytemuck::bytes_of(&tb), false, true, false))
	});

	let b = buffers.get_mut(k).unwrap();
	b.write(&queue, 0, bytemuck::bytes_of(&tb));
}
