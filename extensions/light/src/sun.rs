use chunks::CHUNK_SIZE;
use eeks::prelude::*;
use glam::IVec3;
use render::{Buffer, BufferKey, BufferResource, QueueResource};
use splines::Spline;
use time::TimeResource;



// Time to pull some numbers out of my ass 
// 40% of chunks will be fully in light 
// 40% of chunks will be fully in dark 
// 19% of chunks will fall along a sunlight gradient 
// 1% of chunks will be affected by tinted light 
// We do not want to store a lot of extra data! 
// If it takes 32 bytes to store any of these
// And 32768 bytes for falloff 
// And 131072 bytes for tinted 
// We have 0.4 * 32 + 0.4 * 32 + 0.19 * (32 + 32768) + 0.01 * (32 + 131072) = 7568.64 bytes per average chunk 
// Vs 1.0 * (32 + 131072) = 131104 bytes per average chunk 
// Which uses about 6% of the space 
#[derive(Debug, Default)]
pub enum SunlightContents {
	// Fully bright 
	Exposed,
	// Fully dark 
	#[default]
	Hidden,
	// Sunlight is falling off, but it is still the same colour
	Falloff((Box<[u8]>, u32, u32)),
	// Sunlight has been tinted by glass or something 
	// Should use u24, but it's (probably) not worth the hassle 
	Tinted((Box<[u32]>, u32, u32, u32)),
}
impl SunlightContents {
	/// SunlightContents initializes as being fully in darkness. 
	pub fn new() -> Self {
		Self::default()
	}

	// Follows z downward to maintian cache locality when propagating sunlight
	#[inline]
	fn index_of(pos: IVec3) -> usize {
		let [x, y, z] = pos.as_uvec3().to_array();
		(x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z) as usize
	}

	pub fn get(&self, pos: IVec3) -> u32 {
		let i = Self::index_of(pos);
		match self {
			Self::Exposed => u32::MAX,
			Self::Hidden => u32::MIN,
			Self::Falloff((contents, _, _)) => contents[i] as u32,
			Self::Tinted((contents, _, _, _)) => contents[i],
		}
	}

	// Assumption: the sunlight is white light 
	#[inline]
	fn is_sun_multiple(v: u32) -> bool {
		let vals = [
			v & 0xFF000000 >> 24, 
			v & 0x00FF0000 >> 16,
			v & 0x0000FF00 >> 8,
		];
		vals[0] == vals[1] && vals[0] == vals[2]
	}

	#[inline]
	fn to_tinted(value: u8) -> u32 {
		let value = value as u32;
		(value << 24) + (value << 16) + (value << 8)
	}

	pub fn set(&mut self, pos: IVec3, value: u32) {
		let i = Self::index_of(pos);
		match self {
			Self::Exposed => if value != u32::MAX {
				if value > u8::MAX as u32 {
					*self = Self::Tinted((vec![u32::MAX; CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0, 0));
				} else {
					*self = Self::Falloff((vec![u8::MAX; CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0));
				}
				self.set(pos, value);
			},
			Self::Hidden => if value != u32::MIN {
				if value > u8::MAX as u32 {
					*self = Self::Tinted((vec![u32::MIN; CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0, 0));
				} else {
					*self = Self::Falloff((vec![u8::MIN; CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0));
				}
				self.set(pos, value);
			},
			Self::Falloff((contents, maxc, minc)) => {
				if value > u8::MAX as u32 {
					let s = contents.into_iter().map(|&v| Self::to_tinted(v)).collect::<Vec<_>>();
					let tinc = s.iter().filter(|&&v| !Self::is_sun_multiple(v)).count() as u32;
					*self = Self::Tinted((s.into_boxed_slice(), *maxc, *minc, tinc));
					self.set(pos, value);
					return;
				}
				let value = value as u8;

				let replaced = contents[i];
				contents[i] = value;
				match replaced {
					u8::MAX => *maxc -= 1,
					u8::MIN => *minc -= 1,
					_ => {},
				}
				match value {
					u8::MAX => *maxc += 1,
					u8::MIN => *minc += 1,
					_ => {},
				}

				if *minc == CHUNK_SIZE.pow(3) {
					*self = Self::Hidden;
				} else if *maxc == CHUNK_SIZE.pow(3) {
					*self = Self::Exposed;					
				}
			},
			Self::Tinted((contents, maxc, minc, tinc)) => {
				let replaced = contents[i];
				contents[i] = value;
				match replaced {
					u32::MAX => *maxc -= 1,
					u32::MIN => *minc -= 1,
					_ => {},
				}
				if !Self::is_sun_multiple(replaced) { 
					*tinc -= 1;
				}
				match value {
					u32::MAX => *maxc += 1,
					u32::MIN => *minc += 1,
					_ => {},
				}
				if !Self::is_sun_multiple(value) { 
					*tinc += 1;
				}

				if *minc == CHUNK_SIZE.pow(3) {
					*self = Self::Hidden;
				} else if *maxc == CHUNK_SIZE.pow(3) {
					*self = Self::Exposed;					
				} else if *tinc == 0 {
					let s = contents.into_iter().map(|&v| v as u8).collect::<Vec<_>>();
					*self = Self::Falloff((s.into_boxed_slice(), *maxc, *minc));
				}
			},
		}
	}

	pub fn size(&self) -> usize {
		let base = std::mem::size_of::<Self>();
		match self {
			Self::Exposed | Self::Hidden => base,
			Self::Falloff(_) => base + 1 * CHUNK_SIZE.pow(3) as usize,
			Self::Tinted(_) => base + 4 * CHUNK_SIZE.pow(3) as usize,
		}
	}
}


#[derive(Debug, Resource)]
#[sda(lua = true)]
pub struct SunResource {
	time_angle_spline: Spline<f32, f32>, // time -> angle (radians)
	angle_brightness_spline: Spline<f32, f32>, // angle -> brightness ([0, 1])
	buffer: Option<BufferKey>,
}
impl SunResource {
	pub fn new() -> Self {
		let tas_s = std::fs::read_to_string("idk.ron")
			.expect("Failed to locate time angle spline");
		let time_angle_spline: Spline<f32, f32> = ron::de::from_str(&tas_s)
			.expect("Failed to interpret time angle spline");
		assert!(time_angle_spline.len() >= 2);
		let abs_s = std::fs::read_to_string("idk.ron")
			.expect("Failed to locate angle brightness spline");
		let angle_brightness_spline: Spline<f32, f32> = ron::de::from_str(&abs_s)
			.expect("Failed to interpret angle brightness spline");
		assert!(angle_brightness_spline.len() >= 1);
		Self {
			time_angle_spline, angle_brightness_spline, buffer: None,
		}
	}

	pub fn current_angle(&self, time: f32) -> f32 {
		let max_t = self.time_angle_spline.keys().iter()
			.map(|v| v.t)
			.reduce(|a, v| f32::max(a, v))
			.unwrap();
		self.time_angle_spline.sample(time % max_t).unwrap()
	}

	pub fn current_brightness(&self, time: f32) -> f32 {
		let angle = self.current_angle(time);
		self.angle_brightness_spline.sample(angle).unwrap()
	}
}
impl mlua::UserData for SunResource {
	fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
		methods.add_method("current_angle", |_lua, this, time: f32| {
			Ok(this.current_angle(time))
		});
		methods.add_method("current_brightness", |_lua, this, time: f32| {
			Ok(this.current_brightness(time))
		});
	}
}


pub fn sun_buffer_system(
	mut sun: ResMut<SunResource>,
	time: Res<TimeResource>,
	mut buffers: ResMut<BufferResource>,
	queue: Res<QueueResource>,
) {
	#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
	#[repr(C)]
	struct SunBuffer {
		// Angle of the light (radians)
		pub angle: f32,
		// Brightness of the light ([0, 1])
		pub brightness: f32,
	}

	let time = time.tick_time; 
	let angle = sun.current_angle(time);
	let brightness = sun.current_brightness(time);
	let contents = SunBuffer {
		angle, brightness, 
	};
	let bytes = bytemuck::bytes_of(&contents);

	if let Some(k) = sun.buffer.as_ref().copied() {
		let b = buffers.get_mut(k).unwrap();
		b.write(&queue, 0, bytes);
	} else {
		let b = buffers.insert(Buffer::new_init("SunBuffer", bytes, false, true, false));
		sun.buffer = Some(b);
	}
}
