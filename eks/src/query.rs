//! Making this made me feel unwell. 
//! Please avoid touching it. 
//! 
//! At least it's pretty to look at?
//! Just don't get any funny ideas. 
//! 

use atomic_refcell::{AtomicRef, AtomicRefMut};
use crate::{Component, sparseset::SparseSet, World, entity::{Entity, EntitySparseSet}, Resource};



/// Things that can be request from the world using a tuple. 
/// [Comp], [CompMut], [Res], etc.
pub trait Queriable<'q> {
	type Item;
	fn query(world: &'q World) -> Self::Item;
}
macro_rules! impl_queriable {
	($($t:ident),+) => {
		impl<'q, $($t: Queriable<'q>,)*> Queriable<'q> for ($($t,)*) {
			type Item = ($(<$t as Queriable<'q>>::Item,)*);
			fn query(world: &'q World) -> Self::Item {
				($(<$t as Queriable>::query(world),)*)
			}
		}
	};
}
impl_queriable!(A);
impl_queriable!(A, B);
impl_queriable!(A, B, C);
impl_queriable!(A, B, C, D);
impl_queriable!(A, B, C, D, E);
impl_queriable!(A, B, C, D, E, F);
impl_queriable!(A, B, C, D, E, F, G);
impl_queriable!(A, B, C, D, E, F, G, H);
impl_queriable!(A, B, C, D, E, F, G, H, I);
impl_queriable!(A, B, C, D, E, F, G, H, I, J);


/// Things that store component data. 
/// [Comp] and [CompMut] but not [Res].
pub trait ComponentStorage: Sized {
	type Item;
	fn storage_get(&self, entity: Entity) -> Option<Self::Item>;
	fn shortest_entities<'s>(&'s self) -> &'s [Entity];
	fn iter<'s>(&'s self) -> ComponentIterator<'s, Self>;
}
impl<'a, A: ComponentStorage> ComponentStorage for (A,) {
	type Item = (<A as ComponentStorage>::Item,);
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		Some((
			<A as ComponentStorage>::storage_get(&self.0, entity)?,
		))
	}
	fn shortest_entities<'s>(&'s self) -> &'s [Entity] {
		let ea = <A as ComponentStorage>::shortest_entities(&self.0);
		ea
	}
	fn iter<'s>(&'s self) -> ComponentIterator<'s, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}
impl<'a, A, B> ComponentStorage for (A, B) 
where 
	A: ComponentStorage, 
	B: ComponentStorage, {
	type Item = (
		<A as ComponentStorage>::Item,
		<B as ComponentStorage>::Item,
	);
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		Some((
			<A as ComponentStorage>::storage_get(&self.0, entity)?,
			<B as ComponentStorage>::storage_get(&self.1, entity)?,
		))
	}
	fn shortest_entities<'s>(&'s self) -> &'s [Entity] {
		[
			<A as ComponentStorage>::shortest_entities(&self.0), 
			<B as ComponentStorage>::shortest_entities(&self.1), 
		].into_iter().min_by_key(|e| e.len()).unwrap()
	}
	fn iter<'s>(&'s self) -> ComponentIterator<'s, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}
impl<'a, A, B, C> ComponentStorage for (A, B, C)
where 
	A: ComponentStorage, 
	B: ComponentStorage, 
	C: ComponentStorage, {
	type Item = (
		<A as ComponentStorage>::Item,
		<B as ComponentStorage>::Item,
		<C as ComponentStorage>::Item,
	);
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		Some((
			<A as ComponentStorage>::storage_get(&self.0, entity)?,
			<B as ComponentStorage>::storage_get(&self.1, entity)?,
			<C as ComponentStorage>::storage_get(&self.2, entity)?,
		))
	}
	fn shortest_entities<'s>(&'s self) -> &'s [Entity] {
		[
			<A as ComponentStorage>::shortest_entities(&self.0), 
			<B as ComponentStorage>::shortest_entities(&self.1), 
			<C as ComponentStorage>::shortest_entities(&self.2), 
		].into_iter().min_by_key(|e| e.len()).unwrap()
	}
	fn iter<'s>(&'s self) -> ComponentIterator<'s, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}
impl<'a, A, B, C, D> ComponentStorage for (A, B, C, D)
where 
	A: ComponentStorage, 
	B: ComponentStorage, 
	C: ComponentStorage, 
	D: ComponentStorage, {
	type Item = (
		<A as ComponentStorage>::Item,
		<B as ComponentStorage>::Item,
		<C as ComponentStorage>::Item,
		<D as ComponentStorage>::Item,
	);
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		Some((
			<A as ComponentStorage>::storage_get(&self.0, entity)?,
			<B as ComponentStorage>::storage_get(&self.1, entity)?,
			<C as ComponentStorage>::storage_get(&self.2, entity)?,
			<D as ComponentStorage>::storage_get(&self.3, entity)?,
		))
	}
	fn shortest_entities<'s>(&'s self) -> &'s [Entity] {
		[
			<A as ComponentStorage>::shortest_entities(&self.0), 
			<B as ComponentStorage>::shortest_entities(&self.1), 
			<C as ComponentStorage>::shortest_entities(&self.2), 
			<D as ComponentStorage>::shortest_entities(&self.3), 
		].into_iter().min_by_key(|e| e.len()).unwrap()
	}
	fn iter<'s>(&'s self) -> ComponentIterator<'s, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}


pub struct Comp<'s, C: Component> {
	// storage: Arc<SparseSetContainer>,
	borrow: AtomicRef<'s, SparseSet<C>>,
}
impl<'s, C: Component> std::ops::Deref for Comp<'s, C> {
	type Target = SparseSet<C>;
	fn deref(&self) -> &Self::Target {
		self.borrow.deref()
	}
}
impl<'q, C: Component> Queriable<'q> for Comp<'q, C> {
	type Item = Self;
	fn query(world: &'q World) -> Self {
		Comp { borrow: world.borrow_component_ref::<C>(), }
	}
}
impl<'b, 's, C: Component> ComponentStorage for &'b Comp<'s, C> {
	type Item = &'b C;
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		self.borrow.get(entity)
	}
	fn shortest_entities(&self) -> &'b [Entity] {
		self.borrow.entities()
	}
	fn iter<'a>(&'a self) -> ComponentIterator<'a, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}


pub struct CompMut<'s, C: Component> {
	// storage: Arc<SparseSetContainer>,
	borrow: AtomicRefMut<'s, SparseSet<C>>,
}
impl<'s, C: Component> std::ops::Deref for CompMut<'s, C> {
	type Target = SparseSet<C>;
	fn deref(&self) -> &Self::Target {
		self.borrow.deref()
	}
}
impl<'s, C: Component> std::ops::DerefMut for CompMut<'s, C> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.borrow
	}
}
impl<'q, C: Component> Queriable<'q> for CompMut<'q, C> {
	type Item = Self;
	fn query(world: &'q World) -> Self {
		CompMut { borrow: world.borrow_component_mut::<C>(), }
	}
}
impl<'b, 's, C: Component> ComponentStorage for &'b CompMut<'s, C> {
	type Item = &'b C;
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		self.borrow.get(entity)
	}
	fn shortest_entities(&self) -> &'b [Entity] {
		self.borrow.entities()
	}
	fn iter<'a>(&'a self) -> ComponentIterator<'a, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}
impl<'b, 's, C: Component> ComponentStorage for &'b mut CompMut<'s, C> {
	type Item = &'b mut C;
	fn storage_get(&self, entity: Entity) -> Option<Self::Item> {
		self.borrow.get_ptr(entity).and_then(|p| unsafe { Some(&mut *(p as *mut C)) })
	}
	fn shortest_entities<'a>(&'a self) -> &'a [Entity] {
		self.borrow.entities()
	}
	fn iter<'a>(&'a self) -> ComponentIterator<'a, Self> {
		ComponentIterator { 
			storage: self, 
			entities: self.shortest_entities(), 
			index: 0, 
		}
	}
}


pub struct EntitiesMut<'b> {
	entities: AtomicRefMut<'b, EntitySparseSet>,
}
impl<'b> std::ops::Deref for EntitiesMut<'b> {
	type Target = EntitySparseSet;
	fn deref(&self) -> &Self::Target {
		self.entities.deref()
	}
}
impl<'b> std::ops::DerefMut for EntitiesMut<'b> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.entities
	}
}
impl<'b> Queriable<'b> for EntitiesMut<'b> {
	type Item = Self;
	fn query(world: &'b World) -> Self {
		Self { entities: world.entities.borrow_mut() }
	}
}


pub struct Res<'s, R: Resource> {
	data: AtomicRef<'s, R>,
}
impl<'s, R: Resource> std::ops::Deref for Res<'s, R> {
	type Target = R;
	fn deref(&self) -> &Self::Target {
		self.data.deref()
	}
}
impl<'s, R: Resource> Queriable<'s> for Res<'s, R> {
	type Item = Self;
	fn query(world: &'s World) -> Self {
		Res { data: world.borrow_resource_ref::<R>() }
	}
}


pub struct ResMut<'s, R: Resource> {
	data: AtomicRefMut<'s, R>,
}
impl<'s, R: Resource> std::ops::Deref for ResMut<'s, R> {
	type Target = R;
	fn deref(&self) -> &Self::Target {
		self.data.deref()
	}
}
impl<'s, R: Resource> std::ops::DerefMut for ResMut<'s, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}
impl<'s, R: Resource> Queriable<'s> for ResMut<'s, R> {
	type Item = Self;
	fn query(world: &'s World) -> Self {
		ResMut { data: world.borrow_resource_mut::<R>() }
	}
}


/// Creates a resource (as None) if it does not exist. 
pub struct ResOptMut<'s, R: Resource> {
	data: AtomicRefMut<'s, Option<R>>,
}
impl<'s, R: Resource> std::ops::Deref for ResOptMut<'s, R> {
	type Target = Option<R>;
	fn deref(&self) -> &Self::Target {
		self.data.deref()
	}
}
impl<'s, R: Resource> std::ops::DerefMut for ResOptMut<'s, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}
impl<'s, R: Resource> Queriable<'s> for ResOptMut<'s, R> {
	type Item = Self;
	fn query(world: &'s World) -> Self {
		let data = world.borrow_resource_opt_mut::<R>();
		ResOptMut { data, }
	}
}

// pub struct CompUp<'s, C: Component> {
// 	storage: RefCe, // Refcell?
// 	read: Option<>, 
// 	write: Option<>, // Can't have both being some
// }
// pub struct Exclude<'s, C: Component> {}
// pub struct CompOpt<'s, C: Component> {}
// pub struct CompMutOpt<'s, C: Component> {}

// pub struct ResUp<'s, R: Component> {}

pub struct ComponentIterator<'s, S: ComponentStorage> {
	storage: &'s S,
	entities: &'s [Entity],
	index: usize,
}
impl<'s, S: ComponentStorage> ComponentIterator<'s, S> {
	pub fn with_entities(self) -> ComponentEntityIterator<'s, S> {
		ComponentEntityIterator { 
			storage: self.storage, 
			entities: self.entities, 
			index: self.index, 
		}
	}
}
impl<'s, S: ComponentStorage> Iterator for ComponentIterator<'s, S> {
	type Item = <S as ComponentStorage>::Item;

	fn next(&mut self) -> Option<Self::Item> {
		while let Some(entity) = self.entities.get(self.index).cloned() {
			self.index += 1;
			if let Some(c) = self.storage.storage_get(entity) {
				return Some(c)
			}
		}
		None
	}
}

pub struct ComponentEntityIterator<'s, S: ComponentStorage> {
	storage: &'s S,
	entities: &'s [Entity],
	index: usize,
}
impl<'s, S: ComponentStorage> Iterator for ComponentEntityIterator<'s, S> {
	type Item = (Entity, <S as ComponentStorage>::Item);

	fn next(&mut self) -> Option<Self::Item> {
		while let Some(entity) = self.entities.get(self.index).cloned() {
			self.index += 1;
			if let Some(c) = self.storage.storage_get(entity) {
				return Some((entity, c))
			}
		}
		None
	}
}
