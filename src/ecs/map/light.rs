use std::{sync::Arc, num::NonZeroU16, collections::VecDeque, time::{Instant, Duration}};

use arrayvec::ArrayVec;
use eks::prelude::*;
use glam::{UVec3, Vec4, IVec3, Vec3};
use parking_lot::RwLock;
use slotmap::SecondaryMap;
use crate::{voxel::{CHUNK_SIZE, ArrayVolume, chunk_of_voxel, voxel_relative_to_chunk, chunk_of_point}, util::KGeneration, ecs::{ControlKey, ControlMap, KeyCombo, KeyModifiers, TransformComponent, ControlComponent}, input::KeyKey, rays::FVTIterator};
use super::{model::MapModelResource, chunks::{ChunkKey, ChunksResource}, terrain::TerrainResource};



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
	pub fn as_vec4(self) -> Vec4 {
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



#[derive(Debug, Default, ResourceIdent)]
pub struct TorchLightChunksResource {
	pub chunks: Arc<RwLock<SecondaryMap<ChunkKey, LightChunk>>>,
	pub new_lights: Vec<(IVec3, LightRGBA)>,
	pub old_lights: Vec<(IVec3, LightRGBA)>,
}


/// Creates torchlight storage for existing chunks. 
/// Removes torchlight storage for non-existing chunks. 
#[profiling::function]
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
#[profiling::function]
pub fn torchlight_update_system(
	chunks: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
	mut torchlight: ResMut<TorchLightChunksResource>,
) {
	let mut torchlight_chunks = torchlight.chunks.write();
	let terrain_chunks = terrain.chunks.read();	

	for (pos, light) in torchlight.new_lights.iter().copied() {
		let mut queue = VecDeque::new();
		queue.push_back((pos, light));

		let mut n = 0;

		while let Some((pos, light)) = queue.pop_front() {
			n += 1;

			let cpos = chunk_of_voxel(pos);
			let vpos = voxel_relative_to_chunk(pos, cpos).as_uvec3();
			// Todo: Cache these things so we don't hash as much
			let ck = chunks.read().get_position(cpos)
				.expect("Todo: Skip/Queue if chunk unloaded");
			let lc = torchlight_chunks.get_mut(ck)
				.expect("Todo: Skip/Queue if chunk unloaded");
			
			// Set light level
			trace!("Set {} to {}", pos, light.r);
			let packed: Option<PackedLightRGBA> = light.into();
			if let Some(packed) = packed {
				lc.insert(vpos, packed);
			}
			lc.generation.increment();

			let nlight = light.dec();
			for offs in [
				IVec3::X, IVec3::NEG_X, 
				// IVec3::Y, IVec3::NEG_Y, 
				IVec3::Z, IVec3::NEG_Z, 
			] {
				let npos = pos + offs;
				let ncpos = chunk_of_voxel(npos);
				let nvpos = voxel_relative_to_chunk(npos, ncpos).as_uvec3();
				// Again, please cache these
				// AND ALSO benchmark it so I can have pretty data!
				let nck = chunks.read().get_position(ncpos)
					.expect("Todo: Skip/Queue if chunk unloaded");
				let nlc = torchlight_chunks.get_mut(nck)
					.expect("Todo: Skip/Queue if chunk unloaded");
				let ntc = terrain_chunks.get(nck)
					.expect("Unreachable, light chunk always has a corresponding terrain chunk");

				// If solid, continue
				// Todo: Test for opacity
				if ntc.complete_ref().unwrap().get(nvpos).is_some() {
					warn!("Skip due to solid");
					continue;
				}

				// If greater than or equal to self level, continue
				// This might be wrong, needs testing
				let nl: LightRGBA = nlc.get(nvpos).copied().into();
				if nl.r >= nlight.r {
					// warn!("Skip due to ge light value");
					continue;
				}

				// Add to BFS queue
				queue.push_back((npos, nlight));
			}
		}

		info!("Set {n} light values");
		// panic!()
		// std::thread::sleep(Duration::from_secs(5));
	}
	drop(torchlight_chunks);
	torchlight.new_lights.clear();

	for (pos, _) in torchlight.old_lights.iter().copied() {
		let mut queue = VecDeque::new();
		queue.push_back(pos);

		// Set light to zero
		// For each neighbour
		// If level less than, add to queue
		// Else mark for propagation

	}
}




#[derive(Debug, ComponentIdent)]
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
			control_map.add_control_binding(control, KeyCombo {
				modifiers: KeyModifiers::EMPTY,
				keys: ArrayVec::try_from([
					KeyKey::BoardKey(winit::event::VirtualKeyCode::T),
				].as_slice()).unwrap(),
			});
			control
		};

		let remove = {
			let control = control_map.new_control(
				"remove torchlight", 
				"Remove torchlight where you are looking",
			);
			control_map.add_control_binding(control, KeyCombo {
				modifiers: KeyModifiers::EMPTY,
				keys: ArrayVec::try_from([
					KeyKey::BoardKey(winit::event::VirtualKeyCode::Y),
				].as_slice()).unwrap(),
			});
			control
		};

		let wipe = {
			let control = control_map.new_control(
				"wipe models", 
				"wipe models",
			);
			control_map.add_control_binding(control, KeyCombo {
				modifiers: KeyModifiers::EMPTY,
				keys: ArrayVec::try_from([
					KeyKey::BoardKey(winit::event::VirtualKeyCode::M),
				].as_slice()).unwrap(),
			});
			control
		};

		let set_chunk = {
			let control = control_map.new_control(
				"set chunk", 
				"set whole chunk light to maximum",
			);
			control_map.add_control_binding(control, KeyCombo {
				modifiers: KeyModifiers::EMPTY,
				keys: ArrayVec::try_from([
					KeyKey::BoardKey(winit::event::VirtualKeyCode::N),
				].as_slice()).unwrap(),
			});
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


#[profiling::function]
pub fn torchlight_debug_place_system(
	chunks: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
	mut torchlight: ResMut<TorchLightChunksResource>,
	transforms: Comp<TransformComponent>,
	controls: Comp<ControlComponent>,
	mut modifiers: CompMut<TorchLightModifierComponent>,
	mut models: ResMut<MapModelResource>,
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
		
		// Only for testing, doesn't wipe jobs in progress
		if control.last_tick_pressed(modifier.wipe) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			models.chunks.clear();
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
				torchlight.new_lights.push((position, l));
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

			if let Some(_) = v {
				// debug!("Remove voxel at {}", v.voxel);
				// map.modify_voxel(VoxelModification {
				// 	position: v.voxel,
				// 	set_to: None,
				// 	priority: 0,
				// });
				todo!()
			}
		}
	}
}
