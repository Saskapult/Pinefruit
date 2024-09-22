use std::sync::Arc;

use chunks::{chunk_of_voxel, chunks::{ChunkKey, ChunksResource}, generation::KGeneration, voxel_relative_to_chunk, CHUNK_SIZE};
use eeks::prelude::*;
use glam::{IVec3, Mat4, UVec3};
use parking_lot::RwLock;
use pinecore::render::{AbstractRenderTarget, Buffer, BufferKey, BufferResource, MaterialResource, QueueResource, RenderFrame, RRID};
use slotmap::SecondaryMap;
use splines::Spline;
use terrain::terrain::TerrainResource;
use pinecore::time::TimeResource;



// 2 bits for simd padding, leaves 14 bits
// leaves 4 bits per channel 
// Falloff is 16 max, which is fine I think
const LIGHT_MAX: u16 = 0b1111;
const LIGHT_MIN: u16 = 0;
fn get_r(v: u16) -> u16 { (v & 0b0011110000000000) >> 10 }
fn get_g(v: u16) -> u16 { (v & 0b0000000111100000) >> 5 }
fn get_b(v: u16) -> u16 { (v & 0b0000000000001111) >> 0 }
fn set_r(v: u16, r: u16) -> u16 { (v & !0b0011110000000000) & (r << 10) }
fn set_g(v: u16, g: u16) -> u16 { (v & !0b0000000111100000) & (g << 5) }
fn set_b(v: u16, b: u16) -> u16 { (v & !0b0000000000001111) & (b << 0) }
fn is_tinted(v: u16) -> bool { untinted(v).is_some() }
fn untinted(v: u16) -> Option<u16> { 
	(get_r(v) == get_g(v) && get_r(v) == get_b(v)).then_some(get_r(v))
}
fn splat(v: u16) -> u16 { (v << 10) | (v << 5) | (v << 0) }
fn simd_dec(v: u16) -> u16 {
	v.saturating_sub(0b00000010000100001) & 0b0011110111101111
}
fn is_max(v: u16) -> bool { v == 0b0011110111101111 }
fn is_min(v: u16) -> bool { v == 0 }

// It's helpful to know if a value is tinted because we do not want to re-test each component 
// in order to know if we need to re-pack the volume 
// We store everything as channeled light, only packing when in the data structue 

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
pub enum SunlightVolume {
	// Fully bright 
	Light,
	// Fully dark 
	#[default]
	Dark,
	// Sunlight is falling off, but it is still the same colour
	// We can actually use only 4 bits per value, but this is left for later 
	// (Access by halving index and applying bit offset determined by modulo value) 
	// min value count, max value count  
	Gradient((Box<[u8]>, u32, u32)),
	// Sunlight has been tinted by glass or something 
	// Should use u24, but it's (probably) not worth the hassle 
	// min value count, max value count, tinted value count 
	Tinted((Box<[u16]>, u32, u32, u32)),
}
impl SunlightVolume {
	/// SunlightContents initializes as being fully in darkness. 
	pub fn new() -> Self {
		Self::default()
	}

	// Follows z downward to maintian cache locality when propagating sunlight
	#[inline]
	fn index_of(pos: UVec3) -> usize {
		let [x, y, z] = pos.to_array();
		(x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z) as usize
	}

	pub fn get(&self, pos: UVec3) -> u16 {
		let i = Self::index_of(pos);
		match self {
			Self::Light => LIGHT_MAX,
			Self::Dark => LIGHT_MIN,
			Self::Gradient((contents, _, _)) => contents[i] as u16,
			Self::Tinted((contents, _, _, _)) => contents[i],
		}
	}

	pub fn set(&mut self, pos: UVec3, value: u16) {
		let i = Self::index_of(pos);
		match self {
			Self::Light => if !is_max(value) {
				if is_tinted(value) {
					trace!("Light sun chunk becomes Tinted");
					*self = Self::Tinted((vec![splat(LIGHT_MAX); CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0, 0));
				} else {
					trace!("Lgiht sun chunk becomes Gradient");
					*self = Self::Gradient((vec![LIGHT_MAX as u8; CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0));
				}
				self.set(pos, value);
			},
			Self::Dark => if !is_min(value) {
				if is_tinted(value) {
					trace!("Dark sun chunk becomes Tinted");
					*self = Self::Tinted((vec![splat(LIGHT_MIN); CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0, 0));
				} else {
					trace!("Dark sun chunk becomes Gradient");
					*self = Self::Gradient((vec![LIGHT_MIN as u8; CHUNK_SIZE.pow(3) as usize].into_boxed_slice(), 0, 0));
				}
				self.set(pos, value);
			},
			Self::Gradient((contents, maxc, minc)) => {
				if is_tinted(value) {
					let s = contents.into_iter().map(|&v| splat(v as u16)).collect::<Vec<_>>();
					let tinc = s.iter().filter(|&&v| is_tinted(v)).count() as u32;
					trace!("Gradient sun chunk becomes Tinted");
					*self = Self::Tinted((s.into_boxed_slice(), *maxc, *minc, tinc));
					self.set(pos, value);
					return;
				}
				let value = untinted(value).unwrap();
				let replaced = contents[i] as u16;
				contents[i] = value as u8;
				match replaced {
					LIGHT_MAX => *maxc -= 1,
					LIGHT_MIN => *minc -= 1,
					_ => {},
				}
				match value {
					LIGHT_MAX => *maxc += 1,
					LIGHT_MIN => *minc += 1,
					_ => {},
				}

				if *minc == CHUNK_SIZE.pow(3) {
					trace!("Gradient sun chunk becomes Dark");
					*self = Self::Dark;
				} else if *maxc == CHUNK_SIZE.pow(3) {
					trace!("Gradient sun chunk becomes Light");
					*self = Self::Light;					
				}
			},
			Self::Tinted((contents, maxc, minc, tinc)) => {
				let replaced = contents[i];
				contents[i] = value;
				match replaced {
					LIGHT_MAX => *maxc -= 1,
					LIGHT_MIN => *minc -= 1,
					_ => {},
				}
				if is_tinted(replaced) { 
					*tinc -= 1;
				}
				match value {
					LIGHT_MAX => *maxc += 1,
					LIGHT_MIN => *minc += 1,
					_ => {},
				}
				if is_tinted(value) { 
					*tinc += 1;
				}

				if *minc == CHUNK_SIZE.pow(3) {
					trace!("Tinted sun chunk becomes Dark");
					*self = Self::Dark;
				} else if *maxc == CHUNK_SIZE.pow(3) {
					trace!("Tinted sun chunk becomes Light");
					*self = Self::Light;					
				} else if *tinc == 0 {
					trace!("Tinted sun chunk becomes Gradient");
					let s = contents.into_iter().map(|&v| v as u8).collect::<Vec<_>>();
					*self = Self::Gradient((s.into_boxed_slice(), *maxc, *minc));
				}
			},
		}
	}

	pub fn size(&self) -> usize {
		let base = std::mem::size_of::<Self>();
		match self {
			Self::Light | Self::Dark => base,
			Self::Gradient(_) => base + 1 * CHUNK_SIZE.pow(3) as usize,
			Self::Tinted(_) => base + 4 * CHUNK_SIZE.pow(3) as usize,
		}
	}
}


#[derive(Debug)]
pub struct SunlightChunk {
	contents: SunlightVolume,
	generation: KGeneration,
}
impl std::ops::Deref for SunlightChunk {
	type Target = SunlightVolume;
	fn deref(&self) -> &Self::Target {
		&self.contents
	}
}
impl std::ops::DerefMut for SunlightChunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.contents
	}
}


#[derive(Debug, Resource)]
pub struct SunChunksResource {
	pub chunks: Arc<RwLock<SecondaryMap<ChunkKey, SunlightChunk>>>,
	pub add_lights: Vec<(IVec3, u16)>,
	pub del_lights: Vec<IVec3>, 
}
impl SunChunksResource {
	pub fn approximate_size(&self) -> usize {
		let mut base = std::mem::size_of::<Self>();
		let chunks = self.chunks.read();
		base += chunks.capacity() * std::mem::size_of::<ChunkKey>();
		for c in chunks.values() {
			base += c.size();
		}
		base
	}
}


pub fn sunlight_update_system(
	chunks: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
	mut sun: ResMut<SunChunksResource>,
) {
	// // let mut remove_queue = Vec::new();
	
	// for pos in sun.del_lights.iter().copied() {

	// }
	// sun.del_lights.clear();

	// let mut sun_chunks = sun.chunks.write();
	let terrain_chunks = terrain.chunks.read();
	let chunks = chunks.read();

	let mut prop_queue = Vec::new();

	// Insert lights if their chunks are loaded 
	for (pos, lvl) in sun.add_lights.iter().copied() {
		let mut sun_chunks = sun.chunks.write();

		let chunk_pos = chunk_of_voxel(pos);
		let pos_in_chunk = voxel_relative_to_chunk(pos, chunk_pos).as_uvec3();
		if let Some(chunk) = chunks.get_position(chunk_pos) {
			prop_queue.push((pos, lvl));
			let sun = sun_chunks.get_mut(chunk).unwrap();
			sun.set(pos_in_chunk, lvl);
			sun.generation.increment();
		} else {
			// This will cause errors 
			// You must find a way to retain entries in this case 
			warn!("Chunk for sunlgiht is not loaded, skipping");
		}
	}
	sun.add_lights.clear();

	let mut sun_chunks = sun.chunks.write();
	while let Some((this_pos, this_light)) = prop_queue.pop() {
		for offs in [
			IVec3::X, IVec3::NEG_X, 
			IVec3::Y, IVec3::NEG_Y, 
			IVec3::Z, IVec3::NEG_Z, 
		] {
			let this_light_dec = if offs.y == -1 && is_max(this_light) {
				this_light
			} else {
				simd_dec(this_light)
			};

			let neighbour_pos = this_pos + offs;
			let neighbour_chunk_pos = chunk_of_voxel(neighbour_pos);
			let neighbour_pos_in_chunk = voxel_relative_to_chunk(neighbour_pos, neighbour_chunk_pos).as_uvec3();

			let neighbour_chunk = chunks.get_position(neighbour_chunk_pos);
			if neighbour_chunk.is_none() {
				// If chunk is not loaded, then push light into the queue for later
				// This can/will build up over time, but idk what to do about it 
				// sun.add_lights.push((neighbour_pos, this_light_dec));
				continue
			}
			let neighbour_chunk = neighbour_chunk.unwrap();
			let neighbour_sun = sun_chunks.get(neighbour_chunk).unwrap();
			let neighbour_terrain = terrain_chunks.get(neighbour_chunk);
			if neighbour_terrain.is_none() {
				// sun.add_lights.push((neighbour_pos, this_light_dec));
				continue
			}
			let neighbour_terrain = neighbour_terrain.unwrap().complete_ref().unwrap();

			if neighbour_terrain.get(neighbour_pos_in_chunk).is_some() {
				continue
			}
			let neighbour_light = neighbour_sun.get(neighbour_pos_in_chunk);
			if get_r(neighbour_light) >= get_r(this_light_dec) {
				continue
			}
			prop_queue.push((neighbour_pos, this_light_dec));
			
			let sun = sun_chunks.get_mut(neighbour_chunk).unwrap();
			sun.set(neighbour_pos_in_chunk, this_light_dec);
			sun.generation.increment();	
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
		let tas_s = std::fs::read_to_string("resources/time_angle_spline.ron")
			.expect("Failed to locate time angle spline");
		let time_angle_spline: Spline<f32, f32> = ron::de::from_str(&tas_s)
			.expect("Failed to interpret time angle spline");
		assert!(time_angle_spline.len() >= 2);
		let abs_s = std::fs::read_to_string("resources/angle_brightness_spline.ron")
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
	#[repr(C)]
	#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
	struct SunBuffer {
		// Angle of the light (radians)
		pub angle: f32,
		// Brightness of the light ([0, 1])
		pub brightness: f32,
		pub pad0: f32,
		pub pad1: f32,
		// rotation matrix for the sunbox
		pub rotation: Mat4,
	}

	let time = time.tick_time; 
	// println!("time {:.2}", time);
	let angle = sun.current_angle(time);
	// println!("angle {:.2}", angle);
	let brightness = sun.current_brightness(time);
	let rotation = Mat4::from_rotation_z(angle);
	let contents = SunBuffer {
		angle, brightness, rotation,
		pad0: 0.0, pad1: 0.0, 
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


pub fn sun_render_system(
	mut materials: ResMut<MaterialResource>,
	mut input: ResMut<RenderFrame>,
) {
	let sunbox_mtl = materials.read("resources/materials/sunbox.ron");
	input.stage("sunbox")
		.run_after("skybox")
		.run_before("models")
		.target(AbstractRenderTarget::new()
			.with_colour(RRID::context("albedo"), None)
			.with_depth(RRID::context("depth")))
		.pass(sunbox_mtl, Entity::default());	
}
