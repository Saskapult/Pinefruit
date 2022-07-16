use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::ecs::*;
use nalgebra::*;




#[derive(Component)]
#[storage(VecStorage)]
pub struct MarkerComponent {
	pub look_pos: [i32; 3],
	pub look_normal: [f32; 3],
	pub look_v: Option<crate::world::Voxel>,
}
impl MarkerComponent {
	pub fn new() -> Self {
		Self {
			look_pos: [0; 3],
			look_normal: [0.0; 3],
			look_v: None,
		}
	}
}



pub struct MarkerSystem {
	// Systems should not have data!
	marker_entity: Option<Entity>,
	can_modify_block: bool,
	look_pos: [i32; 3],
	look_normal: [f32; 3],
}
impl MarkerSystem {
	pub fn new() -> Self {
		Self {
			marker_entity: None,
			can_modify_block: true,
			look_pos: [0; 3],
			look_normal: [0.0; 3],
		}
	}
}
impl<'a> System<'a> for MarkerSystem {
	type SystemData = (
		Entities<'a>,
		WriteExpect<'a, GPUResource>,
		WriteExpect<'a, PhysicsResource>,
		ReadExpect<'a, InputResource>,
		WriteStorage<'a, MapComponent>,
		ReadStorage<'a, CameraComponent>,
		WriteStorage<'a, TransformComponent>,
		WriteStorage<'a, ModelComponent>,
		WriteStorage<'a, MarkerComponent>,
	);
	fn run(
		&mut self, 
		(
			entities,
			_gpu,
			_physics_resource,
			input_resource,
			mut maps,
			cameras,
			mut transforms,
			mut models,
			mut mcs
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
		let mut new_marker_rot = None;

		for (_camera, transform, mc) in (&cameras, &transforms, &mut mcs).join() {
			let closest_start_cast = (&mut maps).join().map(|map| {
				let map_raypositions = crate::world::voxel_ray_v2(
					&transform.position,
					&(transform.rotation * vector![0.0, 0.0, 1.0]),
					25.0,
				);
				let f = map_raypositions[0].coords.map(|i| i as f32 + 0.5);
				let blockv = Vector3::from(f);
				let distance = (transform.position - blockv).norm_squared();
				(distance, map, map_raypositions)
			}).min_by(|(d1, _, _), (d2, _, _)| d1.partial_cmp(d2).expect("NaN detected!"));

			if closest_start_cast.is_none() {
				continue
			}
			let (_, map, map_raypositions) = closest_start_cast.unwrap();

			// Find first non-empty and the voxel before it
			let first_block_index = map_raypositions.iter().position(|ray_hit| {
				if let Ok(v) = map.map.get_voxel_world(ray_hit.coords) {
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
			let first_block_hit = match first_block_index {
				Some(idx) => Some(map_raypositions[idx]),
				None => None,
			};
			let back_block_pos = match back_block_index {
				Some(idx) => Some(map_raypositions[idx].coords),
				None => None,
			};

			if self.can_modify_block {	
				// Casted block placement
				if input_resource.mouse_keys.contains_key(&winit::event::MouseButton::Right) {
					self.can_modify_block = false;

					if let Some(pos) = back_block_pos {
						map.set_voxel(pos, crate::world::Voxel::Block(7));
						error!("Set {:?}", back_block_pos);
					} else {
						error!("Did not set because no block to set");
					}

					// std::thread::sleep(std::time::Duration::from_millis(1000));
				}

				// Casted block removal
				if input_resource.mouse_keys.contains_key(&winit::event::MouseButton::Left) {
					self.can_modify_block = false;

					if let Some(hit) = first_block_hit {
						map.set_voxel(hit.coords, crate::world::Voxel::Empty);
						error!("Set {:?}", hit.coords);
					} else {
						error!("Did not set because no block to set");
					}

					// std::thread::sleep(std::time::Duration::from_millis(1000));
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
			if let Some(hit) = first_block_hit {
				new_marker_pos = Some(
					transform.position + transform.rotation * vector![0.0, 0.0, 1.0] * hit.t
					// hit.coords.map(|v| v as f32 + 0.5).into()
				);
				new_marker_rot = Some(hit.normal);

				mc.look_pos = hit.coords;
				mc.look_normal = hit.normal.into();
				mc.look_v = map.map.get_voxel_world(hit.coords).ok();
			} else {
				new_marker_pos = Some(Vector3::new(
					0.5, 
					0.5, 
					0.5,
				));
				new_marker_rot = Some(Vector3::new(
					0.0, 
					0.0, 
					0.0,
				));

				mc.look_pos = [0; 3];
				mc.look_normal = [0.0; 3];
				mc.look_v = None;
			}
		}

		if let Some(pos) = new_marker_pos {
			let marker_transform = transforms.entry(marker_entity).unwrap().or_insert(TransformComponent::new());
			marker_transform.position = pos;
		}
		if let Some(rot) = new_marker_rot {
			let marker_transform = transforms.entry(marker_entity).unwrap().or_insert(TransformComponent::new());
			marker_transform.rotation = UnitQuaternion::face_towards(&rot, &vector![0.001, 1.0, 0.001].normalize());
		}
	}
}
