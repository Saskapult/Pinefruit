use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::ecs::*;
use nalgebra::*;




#[derive(Component)]
#[storage(VecStorage)]
pub struct MarkerComponent {

}


pub struct MarkerSystem {
	// Systems should not have data!
	marker_entity: Option<Entity>,
	can_modify_block: bool,
}
impl MarkerSystem {
	pub fn new() -> Self {
		Self {
			marker_entity: None,
			can_modify_block: true,
		}
	}
}
impl<'a> System<'a> for MarkerSystem {
	type SystemData = (
		Entities<'a>,
		WriteExpect<'a, RenderResource>,
		WriteExpect<'a, PhysicsResource>,
		ReadExpect<'a, InputResource>,
		WriteStorage<'a, MapComponent>,
		ReadStorage<'a, CameraComponent>,
		WriteStorage<'a, TransformComponent>,
		WriteStorage<'a, ModelComponent>,
	);
	fn run(
		&mut self, 
		(
			mut entities,
			mut render_resource,
			mut physics_resource,
			input_resource,
			mut maps,
			cameras,
			mut transforms,
			mut models,
		): Self::SystemData,
	) { 
		// Set up/retrieve marker entity
		let marker_entity = match self.marker_entity {
			Some(e) => e,
			None => {
				let e = entities.create();
				transforms.insert(e, TransformComponent::new()).unwrap();
				models.insert(e, ModelComponent::new(0, 0)).unwrap();
				self.marker_entity = Some(e);
				self.marker_entity.unwrap()
			},
		};

		let mut new_marker_pos = None;

		for (_camera, transform) in (&cameras, &transforms).join() {
			let closest_start_cast = (&mut maps).join().map(|map| {
				let map_raypositions = map.map.voxel_ray(
					&transform.position,
					&(transform.rotation * vector![0.0, 0.0, 1.0]),
					0.0,
					25.0,
				);
				let f = map_raypositions[0].map(|i| i as f32 + 0.5);
				let blockv = Vector3::from(f);
				let distance = (transform.position - blockv).norm_squared();
				(distance, map, map_raypositions)
			}).min_by(|(d1, _, _), (d2, _, _)| d1.partial_cmp(d2).expect("NaN detected!"));

			if closest_start_cast.is_none() {
				continue
			}
			let (_, map, map_raypositions) = closest_start_cast.unwrap();

			// Find first non-empty and the voxel before it
			let first_block_index = map_raypositions.iter().position(|&pos| {
				if let Some(v) = map.map.get_voxel_world(pos) {
					match v {
						crate::world::Voxel::Block(_) => true,
						_ => false,
					}
				} else {
					false
				}
			});
			let back_block_index = match first_block_index {
				Some(idx) => {
					if idx > 0 {
						Some(idx-1)
					} else {
						None
					}
				},
				None => None,
			};
			let first_block_pos = match first_block_index {
				Some(idx) => Some(map_raypositions[idx]),
				None => None,
			};
			let back_block_pos = match back_block_index {
				Some(idx) => Some(map_raypositions[idx]),
				None => None,
			};

			if self.can_modify_block {	
				// Casted block placement
				if input_resource.mouse_keys.contains_key(&winit::event::MouseButton::Right) {
					self.can_modify_block = false;

					if let Some(pos) = back_block_pos {
						map.set_voxel(pos, crate::world::Voxel::Block(0));
						error!("Set {:?}", back_block_pos);
					} else {
						error!("Did not set because no block to set");
					}
				}

				// Casted block removal
				if input_resource.mouse_keys.contains_key(&winit::event::MouseButton::Left) {
					self.can_modify_block = false;

					if let Some(pos) = first_block_pos {
						map.set_voxel(pos, crate::world::Voxel::Empty);
						error!("Set {:?}", first_block_pos);
					} else {
						error!("Did not set because no block to set");
					}
				}
			} else if !(input_resource.mouse_keys.contains_key(&winit::event::MouseButton::Left) || input_resource.mouse_keys.contains_key(&winit::event::MouseButton::Right)) {
				self.can_modify_block = true;
			} else {
				debug!("Did not set block because of timeout stuff");
			}

			// Positional block placement
			if input_resource.board_keys.contains_key(&winit::event::VirtualKeyCode::H) {
				// The voxel the camera is in
				let pos = map.map.point_world_voxel(&transform.position);
				// Set it to dirt
				map.set_voxel(pos, crate::world::Voxel::Block(0));
			}

			// Update marker position
			if let Some(pos) = back_block_pos {
				new_marker_pos = Some(Vector3::new(
					pos[0] as f32 + 0.5, 
					pos[1] as f32 + 0.5, 
					pos[2] as f32 + 0.5,
				));
			} else {
				new_marker_pos = Some(Vector3::new(
					0.5, 
					0.5, 
					0.5,
				));
			}
		}

		if let Some(pos) = new_marker_pos {
			let marker_transform = transforms.entry(marker_entity).unwrap().or_insert(TransformComponent::new());
			marker_transform.position = pos;
		}
	}
}
