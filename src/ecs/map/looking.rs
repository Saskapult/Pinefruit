use eks::prelude::*;
use glam::{Vec3, Quat};
use crate::{voxel::BlockKey, ecs::TransformComponent, rays::{FVTIterator, FVTIteratorItem}, game::{MeshResource, ModelMatrixComponent}};
use super::MapResource;



#[derive(Debug, ComponentIdent, Default, Clone, Copy)]
pub struct VoxelLookingComponent {
	pub result: Option<(BlockKey, FVTIteratorItem)>
}


pub fn voxel_looking_system(
	mut lookers: CompMut<VoxelLookingComponent>,
	transforms: Comp<TransformComponent>,
	map: Res<MapResource>,
) {
	for (looker, transform) in (&mut lookers, &transforms).iter() {
		looker.result = FVTIterator::new(
			transform.translation, 
			transform.rotation.mul_vec3(Vec3::Z), 
			0.0, 100.0, 1.0,
		).find_map(|i| map.get_voxel(i.voxel).and_then(|v| Some((v, i))));
	}
}


#[derive(Debug, ComponentIdent, Default)]
pub struct VoxelLookingMarkerComponent {
	pub entity: Entity,
}


pub fn voxel_looking_marker_system(
	lookers: Comp<VoxelLookingComponent>,
	mut markers: CompMut<VoxelLookingMarkerComponent>,
	mut meshes: ResMut<MeshResource>,
	mut transforms: CompMut<TransformComponent>,
	mut mmc: CompMut<ModelMatrixComponent>,
	mut entities: EntitiesMut,
) {
	let mut stuff = Vec::new();

	// Fetch entities and transforms
	// 

	for (entity, (looker, transform)) in (&lookers, &transforms).iter().with_entities() {
		stuff.push((entity, looker.clone(), transform.clone()));
	}

	for (entity, looker, transform) in stuff {
		if !markers.contains(entity) {
			// Make marker entity
			let marker_entity = entities.spawn();
			transforms.insert(marker_entity, TransformComponent::new());
			mmc.insert(marker_entity, ModelMatrixComponent::new());

			// Add thing
			markers.insert(entity, VoxelLookingMarkerComponent { entity: marker_entity, });
		}
		let marker = markers.get(entity).unwrap();

		
		if let Some((_, r)) = looker.result {
			// if hit
			let mt = transforms.get_mut(marker.entity).unwrap();
			mt.translation = transform.translation + transform.rotation.mul_vec3(Vec3::Z) * r.t;
			mt.rotation = Quat::from_rotation_arc(Vec3::Z, r.normal.as_vec3()).normalize();

			// add model component if not exists
		} else {
			// if not hit
			// remove model component if exists
		}
	}
}
