#![allow(dead_code)]

pub mod sparseset;
pub mod resource;
pub mod entity;
pub mod system;
pub mod query;
pub mod prelude {
	pub use crate::entity::Entity;
	pub use crate::{World, Component, Resource, Storage};
	pub use component_derive::*;
	pub use crate::query::{Queriable, ComponentStorage, Comp, CompMut, Res, ResMut, ResOptMut, EntitiesMut};
	pub use bincode;
}

use std::{fmt::Debug, collections::HashMap};
use atomic_refcell::{AtomicRefCell, AtomicRef, AtomicRefMut};
use entity::{Entity, EntitySparseSet};
use parking_lot::RwLock;
use query::{Queriable, CompMut};
use sparseset::{SparseSet, UntypedSparseSet};
use resource::UntypedResource;
use system::SystemFunction;

#[macro_use]
extern crate log;


// It would be possible to throw all of this into a struct, allowing us to make new components at run time 
// We'd just need to record more function pointers for dropping and other stuff I haven't though of 
pub trait Storage: 'static + Send + Sync + std::fmt::Debug + Sized {
	const STORAGE_ID: &'static str;
	// If some, then we want to record this component's data when taking shapshots 
	// Data will be restored when the parent extension is reloaded
	const SERIALIZE_FN: Option<(
		// Serialization of one item
		// Used in network communication when we only want to replicate some entities
		fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>,
		// fn(&Self, &mut Vec<u8>) -> bincode::Result<()>,
		// Serialization of many items
		// Used when taking world snapshots
		// Points to &[Self]
		fn(*const [u8], &mut Vec<u8>) -> bincode::Result<()>,
		// fn(&[Self]) -> bincode::Result<()>,
		// Deserialization of one item 
		// See serialization of one item
		fn(&[u8]) -> bincode::Result<*mut u8>, // Box<Type>
		// fn(&[u8]) -> bincode::Result<Self>,
		// Deserialization of many items 
		// See serialization of many items
		fn(&[u8]) -> bincode::Result<*mut u8>, // Box<[Type]>
		// fn(&[u8]) -> bincode::Result<Vec<Self>>,
	)>;
	// A function to transform this data for passage into shaders 
	// For example, converting position, scale, and rotation into a matrix
	const RENDERDATA_FN: Option<fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>>;
}


pub trait Component: Storage {}


pub trait Resource: Storage {}
impl<R: Resource> Resource for Option<R> {}
// Do not allow serialization or render data transformation for optional resources
// There is no rationale for this decision other than I just want the thing to compile
// This will likely cause unexpected bugs if someone tries to use it
// In the future we should fix that
// Or, of course, rust could stabilize specialization... 
impl<R: Resource> Storage for Option<R> {
	const STORAGE_ID: &'static str = R::STORAGE_ID;
	const SERIALIZE_FN: Option<(
		fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>,
		fn(*const [u8], &mut Vec<u8>) -> bincode::Result<()>,
		fn(&[u8]) -> bincode::Result<*mut u8>, 
		fn(&[u8]) -> bincode::Result<*mut u8>, 
	)> = None;
	const RENDERDATA_FN: Option<fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>> = None;
}


#[derive(thiserror::Error, Debug)]
pub enum BorrowError {
	#[error("Storage `{0}` does not exist")]
	ResourceMissing(String),
	#[error("Storage `{0}` is already borrowed")]
	BorrowConflict(String),
}


struct WorldStorage<S> {
	storages: RwLock<HashMap<String, *mut AtomicRefCell<S>>>,
}
impl<S> WorldStorage<S> {
	pub fn new() -> Self {
		Self {
			storages: RwLock::new(HashMap::new()),
		}
	}

	pub fn get(&self, k: impl Into<String>) -> Option<*mut AtomicRefCell<S>> {
		let s = self.storages.read();
		s.get(&k.into()).cloned()
	}

	pub fn insert(&self, k: impl Into<String>, s: S) {
		let k = k.into();
		trace!("Creating storage '{k}'");
		let mut storages = self.storages.write();
		storages.insert(k, Box::into_raw(Box::new(AtomicRefCell::new(s))));
	}

	pub fn remove(&self, k: impl AsRef<str>) -> Option<S> {
		let mut s = self.storages.write();
		assert!(unsafe {
			(**s.get(k.as_ref()).unwrap()).try_borrow_mut().is_ok()
		}, "Cannot remove borrowed storage!");
		s.remove(k.as_ref()).and_then(|c| unsafe { 
			let c = Box::from_raw(c);
			Some(c.into_inner())
		})
	}

	pub fn clear(&mut self) {
		for (_, v) in self.storages.write().drain() {
			assert!(unsafe {
				(*v).try_borrow_mut().is_ok()
			}, "Cannot remove borrowed storage!");
			unsafe { 
				let c = Box::from_raw(v);
				drop(c.into_inner());
			}
		}
	}
}
impl<S> Drop for WorldStorage<S> {
	fn drop(&mut self) {
		for &p in self.storages.read().values() {
			drop(unsafe { Box::from_raw(p) });
		}
	}
}
unsafe impl<S> Send for WorldStorage<S> {}
unsafe impl<S> Sync for WorldStorage<S> {}
impl<S> std::fmt::Debug for WorldStorage<S> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WorldStorage")
			.field("storages", &format!("{:?}", self.storages.read().keys().collect::<Vec<_>>()))
			.finish()
	}
}


pub struct World {
	pub(crate) entities: AtomicRefCell<EntitySparseSet>,
	// These are never to be touched by anything other than world
	// Because all references to content will reference world, we can
	// remove things!
	// This is not true if we let other things touch it. 
	components: WorldStorage<UntypedSparseSet>,
	resources: WorldStorage<UntypedResource>,
}
impl World {
	pub fn new() -> Self {
		Self {
			entities: AtomicRefCell::new(EntitySparseSet::default()),
			components: WorldStorage::new(),
			resources: WorldStorage::new(),
		}
	}

	/// Clears all storages. 
	/// Useful in drop code. 
	pub fn clear(&mut self) {
		self.components.clear();
		self.resources.clear();
	}

	pub fn register_component<C: Component>(&mut self) {
		let name = C::STORAGE_ID.to_string();
		self.components.insert(name, SparseSet::<C>::new().into());
	}

	pub fn unregister_component(&mut self, id: impl AsRef<str>) -> Option<UntypedSparseSet> {
		self.components.remove(id.as_ref())
	}

	pub fn insert_resource<R: Resource>(&mut self, resource: R) {
		let name = R::STORAGE_ID.to_string();
		self.resources.insert(name, resource.into());
	}

	pub fn remove_resource_typed<R: Resource>(&mut self) -> Option<R> {
		self.remove_resource(R::STORAGE_ID).and_then(|r| Some(r.into_inner()))
	}

	pub fn remove_resource(&mut self, id: impl AsRef<str>) -> Option<UntypedResource> {
		self.resources.remove(id.as_ref())
	}

	/// Borrow anything that is [Queriable]! 
	/// I'm quite proud of this. 
	pub fn query<'q, Q: Queriable<'q>>(&'q self) -> <Q as Queriable<'q>>::Item {
		Q::query(self)
	}

	pub fn component_raw_ref(&self, id: impl AsRef<str>) -> AtomicRef<UntypedSparseSet> {
		self.components.get(id.as_ref())
			.map(|r| unsafe { &*r })
			.map(|r| r.try_borrow())
			.expect(&*format!("Failed to locate storage '{}'", id.as_ref()))
			.expect(&*format!("Failed to borrow storage '{}'", id.as_ref()))
	}

	pub fn component_ref<C: Component>(&self) -> AtomicRef<SparseSet<C>> {
		AtomicRef::map(
			self.component_raw_ref(C::STORAGE_ID), 
			|b| b.inner_ref()
		)
	}

	pub fn component_raw_mut(&self, id: impl AsRef<str>) -> AtomicRefMut<UntypedSparseSet> {
		self.components.get(id.as_ref())
			.map(|r| unsafe { &*r })
			.map(|r| r.try_borrow_mut())
			.expect(&*format!("Failed to locate storage '{}'", id.as_ref()))
			.expect(&*format!("Failed to borrow storage '{}'", id.as_ref()))
	}

	pub fn component_mut<C: Component>(&self) -> AtomicRefMut<SparseSet<C>> {
		AtomicRefMut::map(
			self.component_raw_mut(C::STORAGE_ID), 
			|b| b.inner_mut()
		)
	}

	/// A horrible thing that I did in order to make the rendering integration work. 
	/// I think that the borrow checker is not smart enough for this. 
	/// 
	/// Marked as unsafe to discourage use. 
	pub unsafe fn component_hack(&self, id: impl AsRef<str>) -> &UntypedSparseSet {
		let r = self.components.get(id.as_ref())
			.expect(&*format!("Bad component '{}'", id.as_ref()));
		let g = &*r;
		let f = g.as_ptr();
		&*f
	}

	pub fn resource_raw_ref(&self, id: impl AsRef<str>) -> AtomicRef<'_, UntypedResource> {
		let r = self.resources.get(id.as_ref())
			.expect(&*format!("Bad resource '{}'", id.as_ref()));
		let r = unsafe { &*r };
		r.try_borrow().unwrap()
	}

	pub fn resource_raw_mut(&self, id: impl AsRef<str>) -> AtomicRefMut<UntypedResource> {
		self.resources.get(id.as_ref())
			.map(|r| unsafe { &*r })
			.map(|r| r.try_borrow_mut())
			.expect(&*format!("Failed to locate storage '{}'", id.as_ref()))
			.expect(&*format!("Failed to borrow storage '{}'", id.as_ref()))
	}

	pub fn resource_ref<R: Resource>(&self) -> AtomicRef<'_, R> {
		AtomicRef::map(
			self.resource_raw_ref(R::STORAGE_ID), 
			|b| b.inner_ref()
		)
	}

	pub fn resource_mut<R: Resource>(&self) -> AtomicRefMut<'_, R> {
		let r = self.resources.get(R::STORAGE_ID.to_string())
			.expect(&*format!("Bad resource '{}'", R::STORAGE_ID));
		let r = unsafe { &*r };
		AtomicRefMut::map(
			r.try_borrow_mut().unwrap(), 
			|b| b.inner_mut()
		)
	}

	pub unsafe fn resource_hack(&self, id: impl AsRef<str>) -> AtomicRef<'_, &[u8]> {
		let f = AtomicRef::map(
			self.resource_raw_ref(id.as_ref()), 
			|b| std::mem::transmute(&b.inner_raw()),
		);
		f
	}
	
	pub fn spawn<'w>(&'w mut self) -> WorldEntitySpawn<'w> {
		let entity = self.entities.get_mut().spawn();
		WorldEntitySpawn { 
			world: self, entity, 
		}
	}

	pub fn add_component<C: Component>(&mut self, entity: Entity, component: C) {
		let mut s = self.query::<CompMut<C>>();
		s.insert(entity, component);
	}

	/// Removes all components from an entity, then marks it for recycling. 
	pub fn destroy(&mut self, entity: Entity) {
		for storage in self.components.storages.read().values().copied() {
			// This is safe becuase the function requires that we have a mutable reference to world and thus exclusive access
			unsafe {&mut *storage}.get_mut().delete(entity);
		}
		self.entities.borrow_mut().remove(entity);
		todo!()
	}

	pub fn run<'q, S: SystemFunction<'q, (), Q, R>, R, Q: Queriable<'q>>(&'q self, system: S) -> R {
		system.run_system((), self)
	}

	pub fn run_with_data<'q, S: SystemFunction<'q, Data, Q, R>, Data, R, Q: Queriable<'q>>(&'q self, data: Data, system: S) -> R {
		system.run_system(data, self)
	}
}


// Replace with entity builder? 
// Then send to world somehow
pub struct WorldEntitySpawn<'w> {
	world: &'w mut World,
	entity: Entity
}
impl<'w> WorldEntitySpawn<'w> {
	pub fn with<C: Component>(self, component: C) -> Self {
		let mut s = self.world.query::<CompMut<C>>();
		s.insert(self.entity, component);
		drop(s);
		self
	}
	pub fn finish(self) -> Entity {
		self.entity
	}
}


#[cfg(test)]
mod tests {
	use std::ops::DerefMut;
	use crate::prelude::*;

	#[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
	pub struct ComponentA(u32);
	#[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
	pub struct ComponentB(u32);
	#[derive(Debug, Resource, PartialEq, Eq, Clone, Copy)]
	pub struct Ressy(u32);

	#[derive(Debug, serde::Serialize, serde::Deserialize)]
	// #[storage_options(snap = true)]
	pub struct Ikd(u32);
	impl Storage for Ikd {
		const STORAGE_ID: &'static str = "Ikd";
		const RENDERDATA_FN: Option<fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>> = Some(|p, b| Ok(()));
		const SERIALIZE_FN: Option<(
			fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>,
			fn(*const [u8], &mut Vec<u8>) -> bincode::Result<()>,
			fn(&[u8]) -> bincode::Result<*mut u8>, 
			fn(&[u8]) -> bincode::Result<*mut u8>, 
		)> = Some((
			|p, buffer| {
				let s = p as *const Self;
				let s = unsafe { &*s };
				bincode::serialize_into(buffer, s)?;
				Ok(())
			},
			|p, buffer| {
				let s = p as *const [Self];
				let s = unsafe { &*s };
				bincode::serialize_into(buffer, s)?;
				Ok(())
			},
			|buffer| {
				let t = bincode::deserialize::<Self>(buffer)?;
				let p = Box::into_raw(Box::new(t)) as *mut u8;
				Ok(p)
			},
			|buffer| {
				let t = bincode::deserialize::<Box<[Self]>>(buffer)?;
				let p = Box::into_raw(t) as *mut u8;
				Ok(p)
			},
		));
	}

	#[test]
	fn test_spawn_get() {
		let mut world = World::new();
		world.register_component::<ComponentA>();
		world.register_component::<ComponentB>();

		let e0 = world.spawn()
			.with(ComponentA(42))
			// .with(ComponentB(43))
			.finish();
		let e1 = world.spawn()
			.with(ComponentA(44))
			.with(ComponentB(45))
			.finish();
		let e2 = world.spawn()
			// .with(ComponentA(46))
			.with(ComponentB(47))
			.finish();
		
		let (a, b) = world.query::<(Comp<ComponentA>, Comp<ComponentB>)>();
		
		assert_eq!(Some(ComponentA(42)), a.get(e0).cloned(), 	"e0 ComponentA mismatch");
		assert_eq!(None, b.get(e0).cloned(), 					"e0 ComponentB exists!?");

		assert_eq!(Some(ComponentA(44)), a.get(e1).cloned(), 	"e1 ComponentA mismatch");
		assert_eq!(Some(ComponentB(45)), b.get(e1).cloned(), 	"e1 ComponentB mismatch");

		assert_eq!(None, a.get(e2).cloned(), 					"e2 ComponentB exists!?");
		assert_eq!(Some(ComponentB(47)), b.get(e2).cloned(), 	"e2 ComponentB mismatch");
		
		
	}

	#[test]
	fn test_query_iter() {
		let mut world = World::new();
		world.register_component::<ComponentA>();
		world.register_component::<ComponentB>();

		let _e0 = world.spawn()
			.with(ComponentA(42))
			// .with(ComponentB(43))
			.finish();
		let _e1 = world.spawn()
			.with(ComponentA(44))
			.with(ComponentB(45))
			.finish();
		let _e2 = world.spawn()
			// .with(ComponentA(46))
			.with(ComponentB(47))
			.finish();
		
		let (a, mut b) = world.query::<(Comp<ComponentA>, CompMut<ComponentB>)>();
		// let g = (&a, &mut b).storage_get(entity).unwrap();
		for (c, d) in (&a, &mut b).iter() {
			println!("{c:?}, {d:?}");
		}
	}

	#[test]
	fn test_system_run() {
		let mut world = World::new();
		world.register_component::<ComponentA>();
		world.register_component::<ComponentB>();

		fn testing_system(
			_a: Comp<ComponentA>,
			_b: Comp<ComponentB>,
		) {
			println!("heyo");
		}

		world.run(testing_system);
	}

	#[test]
	fn test_system_run_res() {
		let mut world = World::new();
		world.register_component::<ComponentA>();
		world.register_component::<ComponentB>();

		world.insert_resource(Ressy(42));

		fn testing_system(
			_a: Comp<ComponentA>,
			_b: Comp<ComponentB>,
			r: Res<Ressy>,
		) {
			assert_eq!(42, r.0)
		}

		world.run(testing_system);
	}

	#[test]
	fn test_system_run_data() {
		let mut world = World::new();
		world.register_component::<ComponentA>();
		world.register_component::<ComponentB>();

		fn testing_system(
			(d,): (i32,),
			_a: Comp<ComponentA>,
			_b: Comp<ComponentB>,
		) {
			assert_eq!(42, d);
		}

		world.run_with_data((42,), testing_system);
	}

	#[test]
	fn test_res_opt() {
		let mut world = World::new();
		world.register_component::<ComponentA>();
		world.register_component::<ComponentB>();

		world.insert_resource::<Option<Ressy>>(None);

		fn testing_system(
			mut r: ResOptMut<Ressy>,
		) {
			if let Some(r) = r.as_ref() {
				println!("r = {r:?}");
			} else {
				println!("Creating");
				let g = r.deref_mut();
				*g = Some(Ressy(42));
			}
		}

		world.run(testing_system);
		world.run(testing_system);
	}
}
