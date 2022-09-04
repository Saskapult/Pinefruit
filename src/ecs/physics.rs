use rapier3d::prelude::*;
use crate::mesh::*;
use nalgebra::*;
use shipyard::*;
use crate::ecs::*;




#[derive(Unique)]
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
}
impl PhysicsResource {
	pub fn new(
	) -> Self {
		Self {
			rigid_body_set: RigidBodySet::new(),
			collider_set: ColliderSet::new(),
			query_pipeline: QueryPipeline::new(),
			gravity: Vector3::new(0.0, -9.81, 0.0),
			integration_parameters: IntegrationParameters::default(),
			physics_pipeline: PhysicsPipeline::new(),
			island_manager: IslandManager::new(),
			broad_phase: BroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			joint_set: JointSet::new(),
			ccd_solver: CCDSolver::new(),
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

	/// Creates a new rigid body with a trimesh collider
	pub fn rb_mesh_trimesh<'a>(
		&mut self, 
		transform: &TransformComponent,
		mesh: &mut Mesh, 
		dynamic: bool,
	) -> RigidBodyHandle {

		let axis_angle = match transform.rotation.axis_angle() {
			Some((axis, angle)) => axis.into_inner() * angle,
			None => Vector3::zeros(),
		};
		let rigid_body = match dynamic {
			true => RigidBodyBuilder::new_dynamic().additional_mass(1.0),
			false => RigidBodyBuilder::new_static(),
		}.position(Isometry3::new(transform.position, axis_angle)).build();
		let rigid_body_handle = self.rigid_body_set.insert(rigid_body);

		let collider = ColliderBuilder::new(mesh.make_trimesh().unwrap())
			.restitution(0.7)
			.density(1.0)
			.build();
		let _collider_handle = self.collider_set.insert_with_parent(collider, rigid_body_handle, &mut self.rigid_body_set);

		rigid_body_handle
	}

	// Borrow checker hates if self if physics_resource
	// I don't understand why
	pub fn remove_collider(&mut self, ch: ColliderHandle) {
		self.collider_set.remove(
			ch, 
			&mut self.island_manager, 
			&mut self.rigid_body_set, 
			false,
		);
	}
}



/// A static physics thing
#[derive(Component, Debug)]
pub struct StaticPhysicsComponent {
	pub rigid_body_handle: RigidBodyHandle,
	pub collider_handles: Vec<ColliderHandle>,
}
impl StaticPhysicsComponent {
	/// Creates a new ridgid body handle
	pub fn new(physics_resource: &mut PhysicsResource) -> Self {
		let rigid_body = RigidBodyBuilder::new_static()
			.build();
		let rigid_body_handle = physics_resource.rigid_body_set.insert(rigid_body);
		Self { 
			rigid_body_handle,
			collider_handles: Vec::new(),
		}
	}

	pub fn with_transform(
		self, 
		physics_resource: &mut PhysicsResource, 
		transform: &TransformComponent,
	) -> Self {
		let rb = &mut physics_resource.rigid_body_set[self.rigid_body_handle];
		let axis_angle = match transform.rotation.axis_angle() {
			Some((axis, angle)) => axis.into_inner() * angle,
			None => Vector3::zeros(),
		};
		rb.set_position(Isometry3::new(transform.position, axis_angle), true);
		self
	}

	pub fn add_collider(
		&mut self, 
		physics_resource: &mut PhysicsResource,
		collider: Collider,
	) -> ColliderHandle {
		let collider_handle = physics_resource.collider_set.insert_with_parent(
			collider, 
			self.rigid_body_handle, 
			&mut physics_resource.rigid_body_set,
		);
		self.collider_handles.push(collider_handle);
		collider_handle
	}
}
impl From<RigidBodyHandle> for StaticPhysicsComponent {
	fn from(item: RigidBodyHandle) -> Self {
		Self { 
			rigid_body_handle: item,
			collider_handles: Vec::new(),
		}
	}
}



/// A non-static physics thing
#[derive(Component, Debug)]
pub struct DynamicPhysicsComponent {
	pub rigid_body_handle: RigidBodyHandle,
	pub collider_handles: Vec<ColliderHandle>,
}
impl DynamicPhysicsComponent {
	/// Creates a new rigid body handle
	pub fn new(physics_resource: &mut PhysicsResource) -> Self {
		let rigid_body = RigidBodyBuilder::new_dynamic()
			.additional_mass(1.0)
			.build();
		let rigid_body_handle = physics_resource.rigid_body_set.insert(rigid_body);
		Self { 
			rigid_body_handle,
			collider_handles: Vec::new(),
		}
	}

	pub fn with_transform(
		self, 
		physics_resource: &mut PhysicsResource, 
		transform: &TransformComponent,
	) -> Self {
		let rb = &mut physics_resource.rigid_body_set[self.rigid_body_handle];
		let axis_angle = match transform.rotation.axis_angle() {
			Some((axis, angle)) => axis.into_inner() * angle,
			None => Vector3::zeros(),
		};
		rb.set_position(Isometry3::new(transform.position, axis_angle), true);
		self
	}

	pub fn add_collider(
		&mut self, 
		physics_resource: &mut PhysicsResource,
		collider: Collider,
	) -> ColliderHandle {
		let collider_handle = physics_resource.collider_set.insert_with_parent(
			collider, 
			self.rigid_body_handle, 
			&mut physics_resource.rigid_body_set,
		);
		self.collider_handles.push(collider_handle);
		collider_handle
	}
}
impl From<RigidBodyHandle> for DynamicPhysicsComponent {
	fn from(item: RigidBodyHandle) -> Self {
		Self { 
			rigid_body_handle: item,
			collider_handles: Vec::new(),
		}
	}
}



/// Ticks physics and moves affected objects
pub fn dynamic_physics_system(
	dynamic_objects: View<DynamicPhysicsComponent>,
	mut transforms: ViewMut<TransformComponent>,
	mut physics: UniqueViewMut<PhysicsResource>,
) {
	// Tick physics
	physics.tick();

	// For each thing with dynamic physics, put it where it should be
	for (dynamic_object, transform) in (&dynamic_objects, &mut transforms).iter() {
		// get position and rotation of object using id
		let body = &physics.rigid_body_set[dynamic_object.rigid_body_handle];
		// Update transform component
		transform.position = *body.translation();
		transform.rotation = *body.rotation();
	}
}


// #[derive(Debug, Component)]
// pub struct DirectionalGravityComponent {
// 	pub gravity: Vector3<f32>,
// }
// pub fn directional_gravity_system(
// 	gravity_components: View<DirectionalGravityComponent>,
// 	dynamic_physics_components: View<DynamicPhysicsComponent>,
// ) {
// 	for (gravity, c) in (&gravity_components, dynamic_physics_components).iter() {
// 		todo!("foce based on mass and stuff");
// 	}
// }


// // Things affected by gravity
// #[derive(Component, Debug)]
// pub struct GravityComponent {
// 	pub gravity_acceleration: Vector3<f32>,
// }

// // Pull derived from physics mass?
// #[derive(Component, Debug)]
// pub struct GravitySourceComponent;
