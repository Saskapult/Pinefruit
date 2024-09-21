use std::{time::{Instant, Duration}, sync::Arc};
use chunks::{blocks::{BlockKey, BlockResource}, chunk_of_voxel, chunks::ChunksResource, fvt::FVTIterator, CHUNK_SIZE};
use pinecore::controls::{ControlComponent, ControlKey, ControlMap, KeyCode, KeyCombo, KeyKey, KeyModifiers};
use eeks::prelude::*;
use glam::{IVec3, Vec3};
use pinecore::transform::TransformComponent;
use crate::terrain::{TerrainEntry, TerrainResource};



#[derive(Debug, Clone, Copy)]
pub struct VoxelModification {
	pub position: IVec3, // Usually world-relative, but it's left unclear so we don't have to write as much code
	pub set_to: Option<BlockKey>,
	pub priority: u32,
}
impl VoxelModification {
	// This should return another type of struct but I'm lazy
	pub fn as_chunk_relative(mut self) -> (IVec3, Self) {
		let c = chunk_of_voxel(self.position);
		self.position -= c * (CHUNK_SIZE as i32);
		(c, self)
	}
}


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
			control_map.add_control_binding(control, KeyCombo::new(
				[KeyKey::BoardKey(KeyCode::KeyE.into())],
				KeyModifiers::EMPTY,	
			));
			control
		};

		let remove = {
			let control = control_map.new_control(
				"remove voxel", 
				"Remove a voxel where you are looking",
			);
			control_map.add_control_binding(control, KeyCombo::new(
				[KeyKey::BoardKey(KeyCode::KeyQ.into())],
				KeyModifiers::EMPTY,	
			));
			control
		};

		Self {
			place, 
			remove,
			last_modification: None,
		}
	}
}


pub fn terrain_placement_queue(
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
pub fn terrain_modification_application(
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
