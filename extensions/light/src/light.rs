use std::{sync::Arc, num::NonZeroU16, collections::VecDeque, time::{Instant, Duration}};
use chunks::{array_volume::ArrayVolume, chunk_of_point, chunk_of_voxel, chunks::{ChunkKey, ChunksResource}, fvt::FVTIterator, generation::KGeneration, voxel_relative_to_chunk, CHUNK_SIZE};
use controls::{ControlComponent, ControlKey, ControlMap, KeyCode, KeyCombo, KeyKey, KeyModifiers};
use eeks::prelude::*;
use glam::{IVec3, UVec3, Vec3, Vec4};
use parking_lot::RwLock;
use slotmap::SecondaryMap;
use terrain::terrain::TerrainResource;
use transform::TransformComponent;


// pub struct LightRGB5 {
// 	pub r: u16,
// 	pub g: u16,
// 	pub b: u16,
// }
// impl LightRGB5 {
// 	fn check_bounds(&self) {
// 		assert!(self.r <= 2_u16.pow(5)-1, "Red channel outside of u5 max!");
// 		assert!(self.g <= 2_u16.pow(5)-1, "Green channel outside of u5 max!");
// 		assert!(self.b <= 2_u16.pow(5)-1, "Blue channel outside of u5 max!");
// 	}

// 	#[inline]
// 	fn to_simd_repr(&self) -> u32 {
// 		((self.r as u32) << 12) | ((self.g as u32) << 6) | ((self.b as u32) << 0)
// 	}

// 	#[inline]
// 	fn from_simd_repr(value: u32) -> Self {
// 		let r = (value & 0xF000) >> 10;
// 		let g = (value & 0x0F00) >> 5;
// 		let b = (value & 0x00F0) >> 0;
// 		LightRGB5 { r, g, b, }
// 	}

// 	pub fn add_simd(&self, other: Self) -> Self {
// 		let a = self.to_simd_repr();
// 		let b = other.to_simd_repr();
// 		Self::from_simd_repr(a.saturating_add(b))
// 	}

// 	pub fn sub_simd(&self, other: Self) -> Self {
// 		let a = self.to_simd_repr();
// 		let b = other.to_simd_repr();
// 		Self::from_simd_repr(a.saturating_sub(b))
// 	}

// 	pub fn to_u16(&self) -> u16 {
// 		self.r << 10 | self.g << 5 | self.b << 0
// 	}

// 	pub fn from_u16(value: u16) -> Self {
// 		let r = (value & 0xF000) >> 10;
// 		let g = (value & 0x0F00) >> 5;
// 		let b = (value & 0x00F0) >> 0;
// 		LightRGB5 { r, g, b, }
// 	}
// }



const U4_MAX_U16: u16 = 2_u16.pow(4) - 1;
const U4_MAX_F32: f32 = U4_MAX_U16 as f32;


#[derive(Debug, Clone, Copy)]
pub struct LightRGBA {
	pub r: u16,
	pub g: u16,
	pub b: u16,
	pub a: u16, // Scaling for all channels
}
impl LightRGBA {
	pub fn into_vec4(self) -> Vec4 {
		Vec4::new(
			self.r as f32 / U4_MAX_F32, 
			self.g as f32 / U4_MAX_F32, 
			self.b as f32 / U4_MAX_F32, 
			self.a as f32 / U4_MAX_F32, 
		)
	}

	// Todo: SIMD
	#[inline]
	pub fn dec(mut self) -> Self {
		self.r = self.r.saturating_sub(1);
		self.g = self.g.saturating_sub(1);
		self.b = self.b.saturating_sub(1);
		self.a = self.a.saturating_sub(1);
		self
	}
}


// We are using NonZeroU16 here because we store the data in an ArrayVolume
// ArrayVolume uses Option<T>, which would be wasteful for storing u16
// 2 bytes per entry vs 3 bytes per entry, so this uses ~67% of the space
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct PackedLightRGBA(NonZeroU16);
impl Into<Option<PackedLightRGBA>> for LightRGBA {
	fn into(self) -> Option<PackedLightRGBA> {
		let mut packed = 0;
		assert!(self.r <= U4_MAX_U16, "Red channel outside of u4 max!");
		packed |= self.r << 12;
		assert!(self.g <= U4_MAX_U16, "Green channel outside of u4 max!");
		packed |= self.g << 8;
		assert!(self.b <= U4_MAX_U16, "Blue channel outside of u4 max!");
		packed |= self.b << 4;
		assert!(self.a <= U4_MAX_U16, "Alpha channel outside of u4 max!");
		packed |= self.a << 0;
		NonZeroU16::new(packed).and_then(|v| Some(PackedLightRGBA(v)))
	}
}
impl From<Option<PackedLightRGBA>> for LightRGBA {
	fn from(value: Option<PackedLightRGBA>) -> Self {
		if let Some(value) = value.and_then(|v| Some(v.0.get())) {
			let r = (value & 0xF000) >> 12;
			let g = (value & 0x0F00) >> 8;
			let b = (value & 0x00F0) >> 4;
			let a = (value & 0x000F) >> 0;
			LightRGBA { r, g, b, a, }
		} else {
			LightRGBA { r: 0, g: 0, b: 0, a: 0, }
		}
	}
}


pub struct LightChunk {
	volume: ArrayVolume<PackedLightRGBA>,
	pub generation: KGeneration, // Generation of light
	
	// Relight neighbour if outgoing lights changed
	// pub outgoing_lights: Vec<(IVec3, DefaultKey)>,
	// pub incoming_lights: RwLock<(SlotMap<DefaultKey, (IVec3, bool)>, KGeneration)>,
}
impl LightChunk {
	pub fn new() -> Self {
		Self {
			volume: ArrayVolume::new(UVec3::splat(CHUNK_SIZE)),
			generation: KGeneration::new(),
		}
	}
}
impl std::fmt::Debug for LightChunk {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LightChunk")
			.field("generation", &self.generation)
			.finish()
	}
}
impl std::ops::Deref for LightChunk {
	type Target = ArrayVolume<PackedLightRGBA>;
	fn deref(&self) -> &Self::Target {
		&self.volume
	}
}
impl std::ops::DerefMut for LightChunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.volume
	}
}



#[derive(Debug, Default, Resource)]
pub struct TorchLightChunksResource {
	pub chunks: Arc<RwLock<SecondaryMap<ChunkKey, LightChunk>>>,
	pub add_lights: Vec<(IVec3, LightRGBA)>,
	pub del_lights: Vec<IVec3>, 
}


/// Creates torchlight storage for existing chunks. 
/// Removes torchlight storage for non-existing chunks. 
pub fn torchlight_chunk_init_system(
	chunks: Res<ChunksResource>,
	torchlight_chunks: ResMut<TorchLightChunksResource>,
) {
	let chunks = chunks.read();
	let mut torchlight_chunks = torchlight_chunks.chunks.write();

	{ // Insert new
		for (key, &pos) in chunks.chunks.iter() {
			if !torchlight_chunks.contains_key(key) {
				debug!("Add torchlight for chunk {}", pos);
				torchlight_chunks.insert(key, LightChunk::new());
			}
		}
	}
	
	{ // Remove old
		torchlight_chunks.retain(|key, _| chunks.chunks.contains_key(key));
	}
}


/// Relights chunks
pub fn torchlight_update_system(
	chunks: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
	mut torchlight: ResMut<TorchLightChunksResource>,
) {
	// Idea: only do a few of these each frame, inserion sorted by distance to player

	// I've set it up this way because I may wish to parallelize it later
	let mut to_remove = Vec::new();
	let mut to_propagate = Vec::new();
	for pos in torchlight.del_lights.iter().copied() {
		debug!("Remove light at {:?}", pos);
		let torchlight_chunks = torchlight.chunks.read();
		let terrain_chunks = terrain.chunks.read();
		let mut queue = Vec::new();

		let chunk_pos = chunk_of_voxel(pos);
		let pos_in_chunk = voxel_relative_to_chunk(pos, chunk_pos).as_uvec3();
		let chunk = chunks.read().get_position(chunk_pos).unwrap();
		let torch = torchlight_chunks.get(chunk).unwrap();
		let light: LightRGBA = torch.get(pos_in_chunk).copied().into();
		queue.push((pos, light));
		to_remove.push((chunk, pos_in_chunk));

		while let Some((this_pos, this_light)) = queue.pop() {
			for offs in [
				IVec3::X, IVec3::NEG_X, 
				// IVec3::Y, IVec3::NEG_Y, 
				IVec3::Z, IVec3::NEG_Z, 
			] {
				let neighbour_pos = this_pos + offs;
				let neighbour_chunk_pos = chunk_of_voxel(neighbour_pos);
				let neighbour_pos_in_chunk = voxel_relative_to_chunk(neighbour_pos, neighbour_chunk_pos).as_uvec3();

				let neighbour_chunk = chunks.read().get_position(neighbour_chunk_pos).unwrap();
				let neighbour_torch = torchlight_chunks.get(neighbour_chunk).unwrap();
				let neighbour_terrain = terrain_chunks.get(neighbour_chunk).unwrap().complete_ref().unwrap();
				
				if neighbour_terrain.get(neighbour_pos_in_chunk).is_some() {
					continue
				}

				let neighbour_light: LightRGBA = neighbour_torch.get(neighbour_pos_in_chunk).copied().into();
				if neighbour_light.r >= this_light.r {
					if neighbour_light.r != 0 {
						to_propagate.push((neighbour_pos, neighbour_light));
					}
				} else {
					to_remove.push((neighbour_chunk, neighbour_pos_in_chunk));
					queue.push((neighbour_pos, neighbour_light));
				}
			}
		}
	}
	// This is where one would deduplicate (if one wanted to bother)
	{
		if to_remove.len() > 0 {
			let n = to_remove.len();
			debug!("Removed {} lights", n);
			to_remove.sort_unstable_by_key(|&(k, p)| (k, p.x, p.y, p.z));
			to_remove.dedup();
			if n != to_remove.len() {
				warn!("Deduplicated to {} lights", to_remove.len());
			}
		}
		let mut torchlight_chunks = torchlight.chunks.write();
		for (key, pos) in to_remove {
			let torch = torchlight_chunks.get_mut(key).unwrap();
			torch.remove(pos);
			torch.generation.increment();
		}
	}
	// torchlight.add_lights.extend(to_propagate);

	// We cannot delay setting here, because otherwise we can loop infinitely 
	// Maybe we could track what was set, but it's more work so no 
	for (pos, light) in torchlight.add_lights.iter().copied() {
		debug!("Create light at {:?}", pos);
		let mut torchlight_chunks = torchlight.chunks.write();
		let terrain_chunks = terrain.chunks.read();
		let mut queue = Vec::new();

		let chunk_pos = chunk_of_voxel(pos);
		let pos_in_chunk = voxel_relative_to_chunk(pos, chunk_pos).as_uvec3();
		let chunk = chunks.read().get_position(chunk_pos).unwrap();
		queue.push((pos, light));
		let torch = torchlight_chunks.get_mut(chunk).unwrap();
		let packed: Option<PackedLightRGBA> = light.into();
		if let Some(packed) = packed {
			torch.insert(pos_in_chunk, packed);
			torch.generation.increment();
		}

		while let Some((this_pos, this_light)) = queue.pop() {
			let this_light_dec = this_light.dec();

			for offs in [
				IVec3::X, IVec3::NEG_X, 
				// IVec3::Y, IVec3::NEG_Y, 
				IVec3::Z, IVec3::NEG_Z, 
			] {
				let neighbour_pos = this_pos + offs;
				let neighbour_chunk_pos = chunk_of_voxel(neighbour_pos);
				let neighbour_pos_in_chunk = voxel_relative_to_chunk(neighbour_pos, neighbour_chunk_pos).as_uvec3();

				let neighbour_chunk = chunks.read().get_position(neighbour_chunk_pos).unwrap();
				let neighbour_torch = torchlight_chunks.get(neighbour_chunk).unwrap();
				let neighbour_terrain = terrain_chunks.get(neighbour_chunk).unwrap().complete_ref().unwrap();

				if neighbour_terrain.get(neighbour_pos_in_chunk).is_some() {
					continue
				}
				let neighbour_light: LightRGBA = neighbour_torch.get(neighbour_pos_in_chunk).copied().into();
				if neighbour_light.r >= this_light_dec.r {
					continue
				}
				queue.push((neighbour_pos, this_light_dec));
				let torch = torchlight_chunks.get_mut(neighbour_chunk).unwrap();
				let packed: Option<PackedLightRGBA> = this_light_dec.into();
				if let Some(packed) = packed {
					torch.insert(neighbour_pos_in_chunk, packed);
					torch.generation.increment();
				}
			}
		}
	}
	torchlight.add_lights.clear();
	torchlight.del_lights.clear();
}




#[derive(Debug, Component)]
pub struct TorchLightModifierComponent {
	pub place: ControlKey,
	pub remove: ControlKey,
	pub wipe: ControlKey,
	pub set_chunk: ControlKey,
	pub last_modification: Option<Instant>
}
impl TorchLightModifierComponent {
	pub fn new(control_map: &mut ControlMap) -> Self {
		let place = {
			let control = control_map.new_control(
				"place torchlight", 
				"Creates torchlight where you are looking",
			);
			control_map.add_control_binding(control, KeyCombo::new(
				[KeyKey::BoardKey(KeyCode::KeyT.into())],
				KeyModifiers::EMPTY,
			));
			control
		};

		let remove = {
			let control = control_map.new_control(
				"remove torchlight", 
				"Remove torchlight where you are looking",
			);
			control_map.add_control_binding(control, KeyCombo::new(
				[KeyKey::BoardKey(KeyCode::KeyY.into())],
				KeyModifiers::EMPTY,
			));
			control
		};

		let wipe = {
			let control = control_map.new_control(
				"wipe models", 
				"wipe models",
			);
			control_map.add_control_binding(control, KeyCombo::new(
				[KeyKey::BoardKey(KeyCode::KeyM.into())],
				KeyModifiers::EMPTY,
			));
			control
		};

		let set_chunk = {
			let control = control_map.new_control(
				"set chunk", 
				"set whole chunk light to maximum",
			);
			control_map.add_control_binding(control, KeyCombo::new(
				[KeyKey::BoardKey(KeyCode::KeyN.into())],
				KeyModifiers::EMPTY,
			));
			control
		};

		Self {
			place, 
			remove,
			wipe,
			set_chunk,
			last_modification: None,
		}
	}
}


pub fn torchlight_debug_place_system(
	chunks: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
	mut torchlight: ResMut<TorchLightChunksResource>,
	transforms: Comp<TransformComponent>,
	controls: Comp<ControlComponent>,
	mut modifiers: CompMut<TorchLightModifierComponent>,
) {
	for (transform, control, modifier) in (&transforms, &controls, &mut modifiers).iter() {

		// Only for testing, doesn't wipe jobs in progress
		if control.last_tick_pressed(modifier.set_chunk) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			let c = chunk_of_point(transform.translation);
			let k = chunks.read().get_position(c).unwrap();

			let mut tlc = torchlight.chunks.write();
			let lc = tlc.get_mut(k).unwrap();
			lc.volume.fill_with(PackedLightRGBA(NonZeroU16::MAX));
			lc.generation.increment();
		}

		if control.last_tick_pressed(modifier.place) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			// Find the first filled voxel
			let v = FVTIterator::new(
				transform.translation, 
				transform.rotation.mul_vec3(Vec3::Z), 
				0.0, 10.0, 1.0,
			).find(|r| terrain.get_voxel(&chunks, r.voxel).is_some());

			if let Some(v) = v {
				let position = v.voxel + v.normal;
				debug!("Place torchlight at {position}");
				let l = LightRGBA {
					r: 4,
					g: 4,
					b: 4,
					a: 4,
				};
				torchlight.add_lights.push((position, l));
			}
		}

		if control.last_tick_pressed(modifier.remove) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			// Find the first filled voxel
			let v = FVTIterator::new(
				transform.translation, 
				transform.rotation.mul_vec3(Vec3::Z), 
				0.0, 10.0, 1.0,
			).find(|r| terrain.get_voxel(&chunks, r.voxel).is_some());

			if let Some(v) = v {
				let position = v.voxel + v.normal;
				debug!("Remove torchlight at {position}");
				torchlight.del_lights.push(position);
			}
		}
	}
}


// // In the future, this should nto be derived directly form time. 
// pub fn daylight_buffer_system(
// 	buffers: ResMut<BufferResource>,
// ) {
// 	struct DaylightBuffer {
// 		// Angle of the light (radians)
// 		pub angle: f32,
// 		// Brightness of the light ([0, 1])
// 		pub brightness: f32,
// 	}
// }
