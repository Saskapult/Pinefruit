use std::num::NonZeroUsize;
use serde::{Serialize, Deserialize};
use crate::{*, entity::Entity};



trait SparseArray {
	// fn contains(&self, entity: &Entity) -> bool;
	fn get(&self, entity: Entity) -> Option<usize>;
	fn insert(&mut self, entity: Entity) -> usize;
	fn remove(&mut self, entity: Entity) -> Option<usize>;
}


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct BasicSparseArray {
	sparse: Vec<Option<NonZeroUsize>>, // Points to data index (thing.get()-1)
	entities: Vec<Entity>, // Packed entities, location is data index
}
impl SparseArray for BasicSparseArray {
	fn get(&self, entity: Entity) -> Option<usize> {
		self.sparse.get(entity.get_index())
			.and_then(|&o| o)
			.and_then(|idx| {
				let sparse_idx = idx.get() - 1;
				if self.entities[sparse_idx] == entity {
					Some(sparse_idx)
				} else {
					None
				}
			})
	}

	fn insert(&mut self, entity: Entity) -> usize {
		// Extend if needed
		if entity.get_index() >= self.sparse.len() {
			self.sparse.resize(entity.get_index() + 1, None);
		}

		if let Some(old_index) = self.sparse.get(entity.get_index()).unwrap() {
			// Replace
			let old_index = old_index.get() - 1;
			self.entities[old_index] = entity;
			old_index
		} else {
			// Create
			let index = self.entities.len();
			self.sparse[entity.get_index()] = NonZeroUsize::new(index + 1);
			self.entities.push(entity);
			index
		}
	}

	fn remove(&mut self, entity: Entity) -> Option<usize> {
		if let Some(dense_index) = self.get(entity) {
			let affected_entity = self.entities.last().unwrap().get_index();

			let sparse_index = self.entities.swap_remove(dense_index).get_index();
			self.sparse[sparse_index] = None.into();

			self.sparse[affected_entity] = Some(NonZeroUsize::new(dense_index+1).unwrap()).into();

			Some(dense_index)
		} else {
			None
		}
	}
}


// const SPARSE_CHUNK_SIZE: usize = 256;
// struct ChunkedSparseArray {
// 	chunks: Vec<Option<Box<[Option<NonZeroUsize>; SPARSE_CHUNK_SIZE]>>>,
// 	entities: Vec<Entity>,
// }
// impl ChunkedSparseArray {
// 	pub fn index(&self, entity: Entity) -> Option<NonZeroUsize> {
// 		let page = entity.get_index() / SPARSE_CHUNK_SIZE;
// 		let index = entity.get_index() % SPARSE_CHUNK_SIZE;

// 		// If in length
// 		if let Some(page) = self.chunks.get(page) {
// 			// If page not empty
// 			if let Some(page) = page {
// 				let thing = page.get(index).unwrap();
// 				// Must test that 
// 				// - Exists
// 				// - Version is same
// 				// if  == Some(entity) {
// 				// 	Some()
// 				// }
// 			}
// 		}
		
// 		todo!()
// 	}
// }


/// Stores (at minimum) `size(usize)` bytes per entry
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SparseSet<T> {
	sparse: BasicSparseArray,
	data: Vec<T>,
}
impl<T> SparseSet<T> {
	pub fn new() -> Self {
		Self {
			sparse: BasicSparseArray::default(),
			data: Vec::new(),
		}
	}

	pub fn contains(&self, entity: Entity) -> bool {
		self.get(entity).is_some()
	}

	pub fn get(&self, entity: Entity) -> Option<&T> {
		self.sparse.get(entity).and_then(|i| Some(self.data.get(i).unwrap()))
	}

	pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
		self.sparse.get(entity).and_then(|i| Some(self.data.get_mut(i).unwrap()))
	}

	/// Used for iterators. 
	/// Should not be visible to the user. 
	pub fn get_ptr(&self, entity: Entity) -> Option<*const T> {
		self.get(entity).and_then(|r| Some(r as *const T))
	}

	pub fn insert(&mut self, entity: Entity, data: T) -> Option<T> {
		trace!("Insert entity {entity} ({})", std::any::type_name::<T>());
		let index = self.sparse.insert(entity);
		let old_data = match index {
			index if (0..self.data.len()).contains(&index) => {
				let old_data = std::mem::replace(&mut self.data[index], data);
				Some(old_data)
			},
			index if index == self.data.len() => {
				self.data.push(data);
				None
			},
			_ => unreachable!("If this happens it means the sparse array is broken somehow"),
		};
		old_data
	}

	pub fn remove(&mut self, entity: Entity) -> Option<T> {
		if let Some(index) = self.sparse.remove(entity) {
			let content = self.data.swap_remove(index);
			Some(content)
		} else {
			None
		}
	}

	pub fn len(&self) -> usize {
		self.data.len()
	}

	pub fn entities<'a>(&'a self) -> &'a [Entity] {
		self.sparse.entities.as_slice()
	}

	// pub fn clear(&mut self) {
	// 	self.sparse.clear();
	// 	self.data.clear();
	// 	self.entities.clear();
	// }
}
// impl<'a, T: Debug> IntoIterator for &'a SparseSet<T> {
// 	type Item = (&'a Entity, &'a T);
// 	type IntoIter = Iter<'a, T>;
// 	fn into_iter(self) -> Self::IntoIter {
// 		Iter {
// 			entities: &self.entities,
// 			components: &self.data,
// 			index: 0,
// 		}
// 	}
// }


pub struct Iter<'a, T: Debug> {
	entities: &'a [Entity],
	components: &'a [T],
	index: usize,
}
impl<'a, T: Debug> Iterator for Iter<'a, T> {
	type Item = (&'a Entity, &'a T);
	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.entities.len() {
			let r = Some((
				&self.entities[self.index], 
				&self.components[self.index],
			));
			self.index += 1;
			r
		} else {
			None
		}
	}
}


/// I tried to do some things with indirect functions but bacame well and truly stuck. 
/// Then I looked at other implementations and found that Sparsey had doen a simlar thing! 
/// That implementation is wonderful but I'm lazy and probably not smart enough to do something quite like it. 
/// Unlike Sparsy, however, EKS isn't meant to be competetive in performance. 
/// So I just wrapped my existing implementation in this thing. 
/// 
/// If we can have some specialization then we can also include serialization functions! 
#[derive(Clone)]
pub struct UntypedSparseSet {
	data: *mut u8, // Raw box of SparseSet<C>

	data_drop: fn(*mut u8), 
	data_delete: fn(*mut u8, Entity) -> bool, 
	data_get: fn(*mut u8, Entity) -> Option<(*const u8, usize)>,
	data_len: fn(*mut u8) -> usize,
	data_contains: *const u8,
	data_entities: *const u8,
	data_lua: *const u8,
	data_serde: Option<(
		// Serialize 
		fn(*const u8, &mut Vec<u8>), // &C
		fn(*const u8, &mut Vec<u8>), // &[C] 
		fn(*const u8, &mut Vec<u8>), // SparseSet<C> 
		// Deserialize 
		fn(&[u8], &mut [u8]), // deserialize into a given space 
		fn(&[u8], &mut Vec<u8>), // deserialize as extension of Vec<C>
		fn(&[u8]) -> *const u8, // SparseSet<~> -> Box<SparseSet<~>>
	)>,
	data_renderdata: Option<StorageRenderDataFn>,	
	data_command: *const u8,

	item_size: usize,
	name: &'static str,
}
impl UntypedSparseSet {	
	fn check_guards<C: Component>(&self) {
		// trace!("Access component storage '{}' ({}) as '{}' ({})", self.name, self.item_size, C::STORAGE_ID, std::mem::size_of::<C>());
		assert_eq!(self.name, C::STORAGE_ID, "Component name differs!");
		assert_eq!(self.item_size, std::mem::size_of::<C>(), "Component size differs!");
	}

	// For hot reloading without serializable data
	pub unsafe fn into_raw(self) -> *mut u8 {
		let d = self.data;
		std::mem::forget(self);
		d
	}
	pub unsafe fn load_raw(&mut self, data: *mut u8) {
		(self.data_drop)(self.data); 
		self.data = data;
	}

	pub fn is_serializable(&self) -> bool {
		self.data_serde.is_some()
	}

	// serialize_one(&self, entity: Entity, buffer: &mut Vec<u8>) -> bincode::Result<()>
	// serialize_some(&self, entities: &[Entity], buffer: &mut Vec<u8>) -> bincode::Result<()>
	// serialize_all(&self, buffer: &mut Vec<u8>) -> bincode::Result<()>
	
	// deserialize_one(&mut self, entity: Entity, buffer: &mut Vec<u8>) -> bincode::Result<()>
	// deserialize_some(&mut self, entities: &[Entity], buffer: &mut Vec<u8>) -> bincode::Result<()>
	// deserialize_all(&mut self, buffer: &mut Vec<u8>) -> bincode::Result<()>

	// // For taking snapshots
	// pub fn serialize(&self, buffer: &mut Vec<u8>) -> bincode::Result<()> {
	// 	// call serialization function
	// 	// Include name and data size guard? No becuase serde has that already I think
	// 	let (f, _) = self.data_serde
	// 		.expect("UntypedResource has no serialization function!");
	// 	(f)(self.data, buffer)
	// }
	// pub fn deserialize(&mut self, buffer: &[u8]) -> bincode::Result<()> {
	// 	// Drop old data
	// 	(self.data_drop)(self.data); 
	// 	// Load new data
	// 	let (_, f) = self.data_serde
	// 		.expect("UntypedResource has no serialization function!");
	// 	let data = f(buffer)?;
	// 	self.data = data;
	// 	Ok(())
	// }

	/// If you try this with the wrong type then stuff will only not break by coincidence. 
	pub fn inner_ref<C: Component>(&self) -> &SparseSet<C> {
		self.check_guards::<C>();
		unsafe { &*(self.data as *mut SparseSet<C>) }
	}

	/// If you try this with the wrong type then stuff will only not break by coincidence. 
	pub fn inner_mut<C: Component>(&mut self) -> &mut SparseSet<C> {
		self.check_guards::<C>();
		unsafe { &mut *(self.data as *mut SparseSet<C>) }
	}

	pub fn delete(&mut self, entity: Entity) -> bool {
		(self.data_delete)(self.data, entity)
	}

	pub fn get(&self, entity: Entity) -> Option<&[u8]> {
		(self.data_get)(self.data, entity).and_then(|(data, len)| 
			Some(unsafe { std::slice::from_raw_parts(data, len) }))
	}

	pub fn get_scoped_ref<'lua, 'scope>(&'scope self, entity: Entity, scope: &mlua::Scope<'lua, 'scope>) -> Option<Result<mlua::AnyUserData<'lua>, mlua::Error>> {
		let thing: fn(*const u8, &mlua::Scope) -> Option<Result<mlua::AnyUserData<'lua>, mlua::Error>> = unsafe { std::mem::transmute(self.data_lua) };
		(self.data_get)(self.data, entity).and_then(|(p, _)| (thing)(p, scope))
	}

	pub fn len(&self) -> usize {
		// let f: fn(*const u8) -> usize = unsafe { std::mem::transmute(self.data_len) };
		(self.data_len)(self.data)
	}
	pub fn entities<'a>(&'a self) -> &'a [Entity] {
		let f: fn(*const u8) -> &'a [Entity] = unsafe { std::mem::transmute(self.data_entities) };
		(f)(self.data)
	}
	pub fn contains(&self, entity: Entity) -> bool {
		let f: fn(*const u8, Entity) -> bool = unsafe { std::mem::transmute(self.data_entities) };
		(f)(self.data, entity)
	}

	// -> bincode::Result<()>
	// serialize_one(&self, buffer: &mut Vec<u8>)
	// serialize_many(&self, entities: &[Entity], buffer: &mut Vec<u8>)
	// serialize_all(&self, buffer: &mut Vec<u8>)
	// deserialize_one(&mut self, entity: Entity, data: &[u8])
	// deserialize_many(&mut self, entities: &[Entity], data: &[u8])
	// deserialize_all(&mut self, data: &[u8])

	// Used by krender to extract render data and append it to a buffer 
	pub fn render_extend(&self, entity: Entity, buffer: &mut Vec<u8>) -> bool {
		if let Some(d) = self.get(entity) {
			if let Some(f) = self.data_renderdata {
				(f)(d.as_ptr(), buffer).is_ok()
			} else {
				buffer.extend_from_slice(d);
				true
			}
		} else {
			false
		}
	}

	pub fn command(&mut self, entity: Entity, command: &[&str]) -> anyhow::Result<()> {
		let data = self.get(entity)
			.with_context(|| "Failed to find entity")
			.unwrap();
		let p = data.as_ptr();
		let f: fn(*const u8, &[&str]) -> anyhow::Result<()> = unsafe { std::mem::transmute(self.data_command) };
		(f)(p, command)
	}

	fn drop_data_as<C: Component>(data: *mut u8) {
		trace!("Dropping untyped sparseset as sparseset of {}", C::STORAGE_ID);
		let resource = unsafe { Box::from_raw(data as *mut SparseSet<C>) };
		drop(resource);
	}
}
impl Drop for UntypedSparseSet {
	fn drop(&mut self) {
		(self.data_drop)(self.data)
	}
}
impl<C: Component> From<SparseSet<C>> for UntypedSparseSet {
	fn from(value: SparseSet<C>) -> Self {
		let b = Box::new(value);
		let p = Box::into_raw(b);

		UntypedSparseSet { 
			data: p as *mut u8, 
			data_drop: Self::drop_data_as::<C>, 
			data_delete: |p, entity| unsafe {
				(*(p as *mut SparseSet<C>)).remove(entity).is_some()
			}, 
			data_get: |p, entity| unsafe {
				(*(p as *mut SparseSet<C>)).get_ptr(entity).and_then(|data| Some((data as *const u8, std::mem::size_of::<C>())))
			},
			data_len: |p| unsafe {
				(*(p as *mut SparseSet<C>)).len()
			},
			data_contains: SparseSet::<C>::contains as *const u8,
			data_entities: SparseSet::<C>::entities as *const u8,
			data_serde: None,
			data_renderdata: C::get_render_data_fn(),
			data_command: C::command as *const u8,
			data_lua: C::create_scoped_ref as *const u8,
			
			item_size: std::mem::size_of::<C>(), 
			name: C::STORAGE_ID,
		}
	}
}


#[cfg(test)]
mod tests {
	use crate::prelude::*;
	use super::*;

	#[derive(Debug, Component)]
	struct ComponentA(pub u32);

	#[test]
	fn test_serde() {
		// let mut storage: UntypedSparseSet = SparseSet::<ComponentA>::new().into();

		// Serialization is not yet implemented 
		// We do not know what use case to expect, anmely how data will be transmitted across networks
		// It is better to serialize (entity, component) or (entity) and (component)? 

		assert!(false, "TODO: component serialization");
	}

	#[test]
	fn test_insert_remove() {
		let mut set = SparseSet::new();
		// println!("{set:#?}");

		println!("Inserting entity 0");
		assert_eq!(None, set.insert(Entity::new(0_u32, 0), "entity 0"));
		// println!("{set:#?}");

		println!("Inserting entity 2");
		assert_eq!(None, set.insert(Entity::new(2_u32, 0), "entity 2"));
		// println!("{set:#?}");

		println!("Removing entity 1 (does not exist)");
		assert_eq!(None, set.remove(Entity::new(1_u32, 0)));
		// println!("{set:#?}");

		println!("Removing entity 0");
		assert_eq!(Some("entity 0"), set.remove(Entity::new(0_u32, 0)));
		// println!("{set:#?}");

		println!("Inserting entity 2 (again)");
		assert_eq!(Some("entity 2"), set.insert(Entity::new(2_u32, 0), "entity 2 (2)"));
		// println!("{set:#?}");
	}

	// #[test]
	// fn test_iter() {
	// 	let mut set = SparseSet::new();
		
	// 	set.insert(Entity::new(0_u32, 0), "entity 0");
	// 	set.insert(Entity::new(2_u32, 0), "entity 2");

	// 	for (e, s) in set.into_iter() {
	// 		println!("{e:?} - '{s}'");
	// 	}
	// }

	#[test]
	fn test_untyped_len() {
		let mut set = SparseSet::new();
		
		set.insert(Entity::new(0_u32, 0), ComponentA(0));
		set.insert(Entity::new(2_u32, 0), ComponentA(1));

		let l0 = set.len();

		let untyped: UntypedSparseSet = set.into();
		let l1 = untyped.len();

		assert_eq!(l0, l1);
	}

	#[test]
	fn test_untyped_entities() {
		let mut set = SparseSet::new();
		
		set.insert(Entity::new(0_u32, 0), ComponentA(0));
		set.insert(Entity::new(2_u32, 0), ComponentA(1));

		let e0 = set.entities().to_vec();

		let untyped: UntypedSparseSet = set.into();
		let e1 = untyped.entities().to_vec();

		assert_eq!(e0, e1);
	}
}
