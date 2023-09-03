pub mod sparseset;
pub mod containers;
pub mod entity;
pub mod system;
pub mod query;
pub mod snapshot;

#[macro_use]
extern crate component_derive;

#[macro_use]
extern crate log;

use std::{fmt::Debug, collections::HashMap};
use atomic_refcell::{AtomicRefCell, AtomicRef, AtomicRefMut};
use entity::{Entity, EntitySparseSet};
use parking_lot::RwLock;
use query::{Queriable, CompMut};
use sparseset::{SparseSet, UntypedSparseSet};
use containers::{SparseSetContainer, ResourceContainer};
use system::SystemFunction;


pub mod prelude {
	pub use crate::entity::Entity;
	pub use crate::{World, Component, Resource};
	pub use component_derive::*;
	pub use crate::query::{Queriable, ComponentStorage, Comp, CompMut, Res, ResMut, ResOptMut, EntitiesMut};
}


pub trait Component: 'static + Send + Sync + std::fmt::Debug {
	const COMPONENT_NAME: &'static str;
}


pub trait Resource: 'static + Send + Sync + std::fmt::Debug {
	const RESOURCE_NAME: &'static str;
}
impl<R: Resource> Resource for Option<R> {
	const RESOURCE_NAME: &'static str = <R as Resource>::RESOURCE_NAME;
}


#[derive(thiserror::Error, Debug)]
pub enum BorrowError {
	#[error("Resource `{0}` does not exist")]
	ResourceMissing(String),
	#[error("Resource `{0}` is already borrowed")]
	BorrowConflict(String),
}


struct WorldStorage<S> {
	storages: RwLock<HashMap<String, *mut S>>,
}
impl<S> WorldStorage<S> {
	pub fn new() -> Self {
		Self {
			storages: RwLock::new(HashMap::new()),
		}
	}

	pub fn get(&self, k: impl Into<String>) -> Option<*mut S> {
		let s = self.storages.read();
		s.get(&k.into()).cloned()
	}

	pub fn insert(&self, k: impl Into<String>, s: S) {
		let k = k.into();
		info!("Creating storage '{k}'");
		let mut storages = self.storages.write();
		storages.insert(k, Box::into_raw(Box::new(s)));
	}

	pub fn get_or_create(&self, k: impl Into<String>, f: impl FnOnce() -> S) -> *mut S {
		// let k: String = k.into();
		// let mut storages = self.storages.upgradable_read();
		// if !storages.contains_key(&k) {
		// 	storages.with_upgraded(|storages| storages.insert(k.clone(), Box::into_raw(Box::new(f()))));
		// }
		// storages.get(&k).cloned().unwrap()

		// This isn't actually Sync but the above attempt panics for some reason
		
		let k: String = k.into();
		if !self.storages.read().contains_key(&k) {
			self.insert(k.clone(), f());
		}
		self.get(&k).unwrap()
	}

	// Must ensure that the thing is not borrowed! 
	// Don't know how to do this automatically, so you must do it yourself. 
	pub fn remove(&self, k: impl AsRef<String>) {
		self.storages.write().remove(k.as_ref()).and_then(|p| unsafe {
			drop(Box::from_raw(p));
			Some(())
		});
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


pub struct World {
	pub(crate) entities: AtomicRefCell<EntitySparseSet>,
	// These are never to be touched by anything other than world
	// Because all references to content will reference world, we can
	// remove things!
	// This is not true if we let other things touch it. 
	components: WorldStorage<SparseSetContainer>,
	resources: WorldStorage<ResourceContainer>,
}
impl World {
	pub fn new() -> Self {
		Self {
			entities: AtomicRefCell::new(EntitySparseSet::default()),
			components: WorldStorage::new(),
			resources: WorldStorage::new(),
		}
	}

	pub fn insert_resource<R: Resource>(&mut self, resource: R) {
		let name = R::RESOURCE_NAME.to_string();

		self.resources.insert(name, Some(resource).into());
		// self.resources.insert(name, resource.into());
	}

	/// Borrow anything that is [Queriable]! 
	/// I'm quite proud of this. 
	pub fn borrow<'q, Q: Queriable<'q>>(&'q self) -> <Q as Queriable<'q>>::Item {
		Q::query(self)
	}

	/// Immutably borrow component storage. 
	pub fn borrow_component_ref<C: Component>(&self) -> AtomicRef<SparseSet<C>> {
		let s = self.components
			.get_or_create(C::COMPONENT_NAME.to_string(), || SparseSet::<C>::new().into());
		let s = unsafe { &*s };
		s.typed_ref::<C>().unwrap()
	}

	/// Mutably borrow component storage. 
	pub fn borrow_component_mut<C: Component>(&self) -> AtomicRefMut<SparseSet<C>> {			
		let s = self.components
			.get_or_create(C::COMPONENT_NAME.to_string(), || SparseSet::<C>::new().into());
		let s = unsafe { &*s };
		s.typed_mut::<C>().unwrap()
	}

	/// Immutably borrow untyped component storage. 
	/// 
	/// Does not create the storage automatically! It will just panic. 
	pub fn borrow_storage_ref(&self, component_name: impl Into<String>) -> AtomicRef<UntypedSparseSet> {
		self.components
			.get(component_name)
			.and_then(|p| unsafe { Some(&*p) })
			.unwrap()
			.untyped_ref()
			.unwrap()
	}

	/// No checking is done here!
	/// 
	/// Added because either I don't understand AtomicRefs or the borrow checker doesn't understand AtomicRefs.
	/// I think AtomicRef<'a, T> <=> &'a T but it disagrees. 
	/// I don't know who is correct. 
	/// 
	/// Unsafe not because I've thought it out, but because I haven't done that and want is discourage use. 
	pub unsafe fn get_storage_ref<'a>(&'a self, component_name: impl Into<String>) -> &'a UntypedSparseSet {
		self.components.get(&component_name.into())
			.and_then(|p| unsafe { Some(&*p) })
			.and_then(|s| {
				Some(&*s.data.as_ptr())
			}).unwrap()
	}

	pub fn borrow_resource_ref<R: Resource>(&self) -> AtomicRef<'_, R> {
		let o = self.resources.get(R::RESOURCE_NAME.to_string())
			.and_then(|p| unsafe { Some(&*p) })
			.and_then(|r| {
				Some(r.resource_ref::<R>().unwrap())
			}).expect(&*format!("Bad resource '{}'", R::RESOURCE_NAME));
		AtomicRef::map(o, |o| o.as_ref().unwrap())
	}

	pub fn borrow_resource_mut<R: Resource>(&self) -> AtomicRefMut<'_, R> {
		let o = self.resources.get(R::RESOURCE_NAME.to_string())
			.and_then(|p| unsafe { Some(&*p) })
			.and_then(|r| {
				Some(r.resource_mut::<R>().unwrap())
			}).expect(&*format!("Bad resource '{}'", R::RESOURCE_NAME));
		AtomicRefMut::map(o, |o| o.as_mut().unwrap())
	}

	pub(crate) fn borrow_resource_opt_mut<R: Resource>(&self) -> AtomicRefMut<'_, Option<R>> {
		let r = self.resources.get(R::RESOURCE_NAME.to_string()).unwrap_or_else(|| {
			self.resources.insert(R::RESOURCE_NAME.to_string(), None::<R>.into());
			self.resources.get(R::RESOURCE_NAME.to_string()).unwrap()
		});
		let r = unsafe { Some(&*r) }.unwrap();
		let r = Some(r.resource_mut::<R>().unwrap());
		r.unwrap()
	}

	pub fn borrow_resource_bytes(&self, resource_name: impl Into<String>) -> AtomicRef<'_, &[u8]> {
		self.resources.get(&resource_name.into())
			.and_then(|p| unsafe { Some(&*p) })
			.and_then(|r| {
				Some(r.untyped_ref().unwrap())
			}).unwrap()
		// Could use unsafe pointer deref stuff
	}
	
	pub fn spawn<'w>(&'w mut self) -> WorldEntitySpawn<'w> {
		let entity = self.entities.get_mut().spawn();
		WorldEntitySpawn { 
			world: self, entity, 
		}
	}

	pub fn add_component<C: Component>(&mut self, entity: Entity, component: C) {
		let mut s = self.borrow::<CompMut<C>>();
		s.insert(entity, component);
	}

	/// Removes all components from an entity, then marks it for recycling. 
	pub fn destroy(&mut self, entity: Entity) {
		for storage in self.components.storages.read().values().copied() {
			// This is safe becuase the function requires that we have a mutable reference to world and thus exclusive access
			unsafe {&*storage}.untyped_mut().unwrap().delete_untyped(entity);
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
		let mut s = self.world.borrow::<CompMut<C>>();
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

use query::Res;

use crate::query::{Comp, CompMut, ResOptMut};
	use crate::query::ComponentStorage;
	use super::*;

	#[derive(Debug, component_derive::ComponentIdent, PartialEq, Eq, Clone, Copy)]
	pub struct ComponentA(u32);
	#[derive(Debug, component_derive::ComponentIdent, PartialEq, Eq, Clone, Copy)]
	pub struct ComponentB(u32);
	#[derive(Debug, component_derive::ResourceIdent, PartialEq, Eq, Clone, Copy)]
	pub struct Ressy(u32);

	#[test]
	fn test_spawn_get() {
		let mut world = World::new();

		// world.insert_component_storage::<ComponentA>();
		// world.insert_component_storage::<ComponentB>();

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
		
		let (a, b) = world.borrow::<(Comp<ComponentA>, Comp<ComponentB>)>();
		
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

		// world.insert_component_storage::<ComponentA>();
		// world.insert_component_storage::<ComponentB>();

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
		
		let (a, mut b) = world.borrow::<(Comp<ComponentA>, CompMut<ComponentB>)>();
		// let g = (&a, &mut b).storage_get(entity).unwrap();
		for (c, d) in (&a, &mut b).iter() {
			println!("{c:?}, {d:?}");
		}
	}

	#[test]
	fn test_system_run() {
		let world = World::new();

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
		let world = World::new();

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
		let world = World::new();

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