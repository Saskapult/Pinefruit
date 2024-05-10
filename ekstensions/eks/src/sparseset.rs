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
		info!("Insert entity {entity} ({})", std::any::type_name::<T>());
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

	fn_drop: fn(&mut Self), 
	fn_delete: fn(*mut u8, Entity) -> bool, 
	fn_get: fn(*mut u8, Entity) -> Option<(*const u8, usize)>,
	fn_serde: Option<(
		// Serialize 
		fn(&Self, Entity, &mut Vec<u8>), // One 
		fn(&Self, &[Entity], &mut Vec<u8>), // Many 
		fn(&Self, &mut Vec<u8>), // All 
		// Deserialize 
		fn(&mut Self, Entity, &[u8]), 
		fn(&mut Self, &[Entity], &[u8]), 
		fn(&mut Self, &[u8]),
	)>,
	fn_renderdata: Option<
		fn(*const u8, &mut Vec<u8>)
	>,	

	item_size: usize,
	name: &'static str,
}
impl UntypedSparseSet {	
	fn check_guards<C: Component>(&self) {
		// trace!("Access component storage '{}' ({}) as '{}' ({})", self.name, self.item_size, C::STORAGE_ID, std::mem::size_of::<C>());
		assert_eq!(self.name, C::STORAGE_ID, "Component name differs!");
		assert_eq!(self.item_size, std::mem::size_of::<C>(), "Component size differs!");
	}

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
		(self.fn_delete)(self.data, entity)
	}

	pub fn get(&self, entity: Entity) -> Option<&[u8]> {
		(self.fn_get)(self.data, entity).and_then(|(data, len)| 
			Some(unsafe { std::slice::from_raw_parts(data, len) }))
	}

	// Used by krender to extract render data and append it to a buffer 
	pub fn render_extend(&self, entity: Entity, buffer: &mut Vec<u8>) -> bool {
		if let Some(d) = self.get(entity) {
			if let Some(f) = self.fn_renderdata {
				(f)(d.as_ptr(), buffer);
			} else {
				buffer.extend_from_slice(d);
			}
			return true
		}
		false
	}
}
impl Drop for UntypedSparseSet {
	fn drop(&mut self) {
		info!("Dropping UntypedSparseSet '{}'", self.name);
		(self.fn_drop)(self)
	}
}
impl<C: Component> From<SparseSet<C>> for UntypedSparseSet {
	fn from(value: SparseSet<C>) -> Self {
		let b = Box::new(value);
		let p = Box::into_raw(b);

		let g = |p: *const u8, b: &mut Vec<u8>| unsafe {
			let c = p as *const C;
			// TODO: error handling
			(C::RENDERDATA_FN.unwrap())(&*c, b).unwrap();
		};

		UntypedSparseSet { 
			data: p as *mut u8, 
			
			fn_drop: |s: &mut Self| unsafe {
				let b = Box::from_raw(s.data as *mut SparseSet<C>);
				drop(b);
			}, 
			fn_delete: |p, entity| unsafe {
				(*(p as *mut SparseSet<C>)).remove(entity).is_some()
			}, 
			fn_get: |p, entity| unsafe {
				(*(p as *mut SparseSet<C>)).get_ptr(entity).and_then(|data| Some((data as *const u8, std::mem::size_of::<C>())))
			},
			fn_serde: None,
			fn_renderdata: if C::RENDERDATA_FN.is_some() { Some(g) } else { None },
			// fn_renderdata: None,
			
			item_size: std::mem::size_of::<C>(), 
			name: C::STORAGE_ID,
		}
	}
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_array_insert_remove() {
		// let mut set = BasicSparseArray::default();
		// let e0 = set.insert(Entity { id: 0, generation: 0 });
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
}
