use std::collections::HashMap;

use rapier3d::prelude::*;
use crate::mesh::*;
use nalgebra::*;
use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::ecs::*;



pub struct PhysicsResource {
	pub rigid_body_set: RigidBodySet,
	pub collider_set: ColliderSet,
	pub query_pipeline: QueryPipeline,
	pub gravity: Vector3<f32>,
	pub integration_parameters: IntegrationParameters,
	pub physics_pipeline: PhysicsPipeline,
	pub island_manager: IslandManager,
	pub broad_phase: BroadPhase,
	pub narrow_phase: NarrowPhase,
	pub joint_set: JointSet,
	pub ccd_solver: CCDSolver,

	pub rigid_body_handle_map: HashMap<RigidBodyHandle, Entity>,
}
impl PhysicsResource {
	pub fn new(
	) -> Self {
		Self {
			rigid_body_set: RigidBodySet::new(),
			collider_set: ColliderSet::new(),
			query_pipeline: QueryPipeline::new(),
			gravity: vector![0.0, -1.0, 0.0],
			integration_parameters: IntegrationParameters::default(),
			physics_pipeline: PhysicsPipeline::new(),
			island_manager: IslandManager::new(),
			broad_phase: BroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			joint_set: JointSet::new(),
			ccd_solver: CCDSolver::new(),
			
			rigid_body_handle_map: HashMap::new(),
		}
	}

	pub fn add_ground(&mut self) {
		self.collider_set.insert(
			ColliderBuilder::cuboid(100.0, 0.1, 100.0).build()
		);
	}

	/// Casts a ray, returns collision position
	pub fn ray(
		&self, 
		origin: Point3<f32>, 
		direction: Vector3<f32>,
	) -> Option<(ColliderHandle, Point3<f32>)> {
		let ray = Ray::new(origin.into(), direction.into());
		
		if let Some((handle, toi)) = self.query_pipeline.cast_ray(
			&self.collider_set, 
			&ray, 
			100.0, 
			true, 
			InteractionGroups::all(), 
			None,
		) {
			// The first collider hit has the handle `handle` and it hit after
			// the ray travelled a distance equal to `ray.dir * toi`.
			Some((handle, ray.point_at(toi)))
		} else {
			None
		}		
	}

	pub fn tick(&mut self) {
		info!("Physics tick!");
		self.physics_pipeline.step(
			&self.gravity,
			&self.integration_parameters,
			&mut self.island_manager,
			&mut self.broad_phase,
			&mut self.narrow_phase,
			&mut self.rigid_body_set,
			&mut self.collider_set,
			&mut self.joint_set,
			&mut self.ccd_solver,
			&(),
			&(),
		);
		
		self.query_pipeline.update(&self.island_manager, &self.rigid_body_set, &self.collider_set);
	}

	pub fn add_rigid_body_with_mesh(
		&mut self, 
		mesh: Mesh, 
		dynamic: bool,
	) -> RigidBodyHandle {
		let rigid_body = match dynamic {
			true => RigidBodyBuilder::new_dynamic(),
			false => RigidBodyBuilder::new_static(),
		}.build();
		let rb_handle = self.rigid_body_set.insert(rigid_body);
		let collider = ColliderBuilder::trimesh(
			mesh.positions.unwrap().iter().map(|pos| Point3::new(pos[0], pos[1], pos[2])).collect::<Vec<_>>(),
			mesh.indices.unwrap().chunks_exact(3).map(|i| [i[0] as u32, i[1] as u32, i[2] as u32]).collect::<Vec<_>>(),
		).restitution(0.7).build();
		let _c_handle = self.collider_set.insert_with_parent(collider, rb_handle, &mut self.rigid_body_set);

		self.query_pipeline.update(&self.island_manager, &self.rigid_body_set, &self.collider_set);

		rb_handle
	}

	pub fn add_mesh_trimesh<'a>(
		&mut self, 
		entity: &Entity,
		transform: &TransformComponent,
		mesh: &mut Mesh, 
		static_physics: &mut WriteStorage<'a, StaticPhysicsComponent>,
		dynamic_physics: &mut WriteStorage<'a, DynamicPhysicsComponent>,
		dynamic: bool,
	) -> RigidBodyHandle {

		let axis_angle = match transform.rotation.axis_angle() {
			Some((axis, angle)) => axis.into_inner() * angle,
			None => Vector3::zeros(),
		};
		let rigid_body = match dynamic {
			true => RigidBodyBuilder::new_dynamic(),
			false => RigidBodyBuilder::new_static(),
		}.position(Isometry3::new(transform.position, axis_angle)).build();
		let rigid_body_handle = self.rigid_body_set.insert(rigid_body);
		if mesh.collider_shape.is_none() {
			mesh.make_trimesh().unwrap();
		}
		let collider = ColliderBuilder::new(mesh.collider_shape.as_ref().unwrap().clone())
			.restitution(0.7)
			.build();
		let _collider_handle = self.collider_set.insert_with_parent(collider, rigid_body_handle, &mut self.rigid_body_set);

		// Add to the handle map
		self.rigid_body_handle_map.insert(rigid_body_handle, *entity);
		
		// Insert the proper component
		if dynamic {
			let prev = dynamic_physics.insert(*entity, DynamicPhysicsComponent::new(rigid_body_handle))
				.unwrap();
			if prev.is_some() {
				warn!("Overwrote dynamic physics component for entity {entity:?}");
			}
		} else {
			let prev = static_physics.insert(*entity, StaticPhysicsComponent::new(rigid_body_handle))
				.unwrap();
			if prev.is_some() {
				warn!("Overwrote static physics component for entity {entity:?}");
			}
		}

		rigid_body_handle
	}
}



/// A static physics thing
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct StaticPhysicsComponent {
	pub rigid_body_handle: RigidBodyHandle,
}
impl StaticPhysicsComponent {
	pub fn new(rigid_body_handle: RigidBodyHandle) -> Self {
		Self { rigid_body_handle }
	}
}



/// A non-static physics thing
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct DynamicPhysicsComponent {
	pub rigid_body_handle: RigidBodyHandle,
}
impl DynamicPhysicsComponent {
	pub fn new(rigid_body_handle: RigidBodyHandle) -> Self {
		Self { rigid_body_handle }
	}
}



/// Ticks physics and moves affected objects
pub struct DynamicPhysicsSystem;
impl<'a> System<'a> for DynamicPhysicsSystem {
	type SystemData = (
		ReadStorage<'a, DynamicPhysicsComponent>,
		WriteStorage<'a, TransformComponent>,
		WriteExpect<'a, PhysicsResource>,
	);

	fn run(
		&mut self, 
		(
			p_dynamic,
			mut transform,
			mut p_resource,
		): Self::SystemData,
	) { 
		// Tick physics
		p_resource.tick();

		// For each thing with dynamic physics, put it where it should be
		for (p_dynamic_c, transform_c) in (&p_dynamic, &mut transform).join() {
			// get position and rotation of object using id
			let rbid = p_dynamic_c.rigid_body_handle;
			let body = &p_resource.rigid_body_set[rbid];
			// Update transform component
			transform_c.position = *body.translation();
			transform_c.rotation = *body.rotation();
		}
	}
}