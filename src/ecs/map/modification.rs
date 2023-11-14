use std::{time::{Instant, Duration}, sync::Arc};

use crate::{ecs::{TransformComponent, ControlComponent, ControlMap, ControlKey, KeyModifiers, KeyCombo, ChunkEntry}, rays::FVTIterator, voxel::{VoxelModification, chunk_of_voxel, voxel_relative_to_chunk}, input::KeyKey};

use super::{MapResource, BlockResource};
use arrayvec::ArrayVec;
use eks::prelude::*;
use glam::Vec3;



#[derive(Debug, ComponentIdent)]
pub struct VoxelModifierComponent {
	pub place: ControlKey,
	pub remove: ControlKey,
	pub last_modification: Option<Instant>
}
impl VoxelModifierComponent {
	pub fn new(control_map: &mut ControlMap) -> Self {
		let place = {
			let control = control_map.new_control(
				"place voxel", 
				"Creates a voxel where you are looking",
			);
			control_map.add_control_binding(control, KeyCombo {
				modifiers: KeyModifiers::EMPTY,
				keys: ArrayVec::try_from([
					// KeyKey::MouseKey(winit::event::MouseButton::Right),
					KeyKey::BoardKey(winit::event::VirtualKeyCode::E),
				].as_slice()).unwrap(),
			});
			control
		};

		let remove = {
			let control = control_map.new_control(
				"remove voxel", 
				"Remove a voxel where you are looking",
			);
			control_map.add_control_binding(control, KeyCombo {
				modifiers: KeyModifiers::EMPTY,
				keys: ArrayVec::try_from([
					// KeyKey::MouseKey(winit::event::MouseButton::Left),
					KeyKey::BoardKey(winit::event::VirtualKeyCode::Q),
				].as_slice()).unwrap(),
			});
			control
		};

		Self {
			place, 
			remove,
			last_modification: None,
		}
	}
}


#[profiling::function]
pub fn map_placement_system(
	map: Res<MapResource>,
	transforms: Comp<TransformComponent>,
	controls: Comp<ControlComponent>,
	mut modifiers: CompMut<VoxelModifierComponent>,
	blocks: Res<BlockResource>,
) {
	for (transform, control, modifier) in (&transforms, &controls, &mut modifiers).iter() {
		// Voxel placement
		if control.last_tick_pressed(modifier.place) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			// Find the first filled voxel
			let v = FVTIterator::new(
				transform.translation, 
				transform.rotation.mul_vec3(Vec3::Z), 
				0.0, 10.0, 1.0,
			).find(|r| map.get_voxel(r.voxel).is_some());

			if let Some(v) = v {
				let position = v.voxel + v.normal;
				debug!("Place voxel at {position}");
				map.modify_voxel(VoxelModification {
					position,
					set_to: Some(blocks.read().key_by_name(&"grass".to_string()).unwrap()),
					priority: 0,
				});
			}
		}

		if control.last_tick_pressed(modifier.remove) && modifier.last_modification.and_then(|i| Some(i.elapsed() > Duration::from_secs_f32(0.1))).unwrap_or(true) {
			modifier.last_modification = Some(Instant::now());

			// Find the first filled voxel
			let v = FVTIterator::new(
				transform.translation, 
				transform.rotation.mul_vec3(Vec3::Z), 
				0.0, 10.0, 1.0,
			).find(|r| map.get_voxel(r.voxel).is_some());

			if let Some(v) = v {
				debug!("Remove voxel at {}", v.voxel);
				map.modify_voxel(VoxelModification {
					position: v.voxel,
					set_to: None,
					priority: 0,
				});
			}
		}
	}
}


/// Applies queued voxel modifications. 
#[profiling::function]
pub fn map_modification_system(
	map: ResMut<MapResource>,
) {
	let mut chunks = map.chunks.write();
	let mut mods = map.block_mods.write();
	
	mods.retain(|c, modifications| {
		if let Some(ChunkEntry::Complete(chunk)) = chunks.key(c).and_then(|k| chunks.get_mut(k)) {
			let inner = Arc::make_mut(chunk);
			for modification in modifications {
				if let Some(b) = modification.set_to {
					inner.insert(modification.position.as_uvec3(), b);
				} else {
					inner.remove(modification.position.as_uvec3());
				}
			}
			inner.generation.increment();
			
			false
		} else {
			true
		}
	});
}
