use std::{time::{Instant, Duration}, sync::Arc};

use crate::{ecs::{TransformComponent, ControlComponent, ControlMap, ControlKey, KeyModifiers, KeyCombo}, rays::FVTIterator, voxel::VoxelModification, input::KeyKey};

use super::{BlockResource, terrain::{TerrainResource, TerrainEntry}, chunks::ChunksResource};
use arrayvec::ArrayVec;
use eks::prelude::*;
use glam::Vec3;
use winit::keyboard::KeyCode;



#[derive(Debug, Component)]
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
					KeyKey::BoardKey(KeyCode::KeyE.into()),
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
					KeyKey::BoardKey(KeyCode::KeyQ.into()),
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
	cr: Res<ChunksResource>,
	terrain: Res<TerrainResource>,
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
			).find(|r| terrain.get_voxel(&cr, r.voxel).is_some());

			if let Some(v) = v {
				let position = v.voxel + v.normal;
				debug!("Place voxel at {position}");
				terrain.modify_voxel(VoxelModification {
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
			).find(|r| terrain.get_voxel(&cr, r.voxel).is_some());

			if let Some(v) = v {
				debug!("Remove voxel at {}", v.voxel);
				terrain.modify_voxel(VoxelModification {
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
	chunks: Res<ChunksResource>, 
	terrain: ResMut<TerrainResource>,
) {
	let chunks = chunks.read();
	let mut terrain_chunks = terrain.chunks.write();
	let mut mods = terrain.block_mods.write();
	
	mods.retain(|c, modifications| {
		if let Some(TerrainEntry::Complete(chunk)) = chunks.get_position(*c).and_then(|k| terrain_chunks.get_mut(k)) {
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
