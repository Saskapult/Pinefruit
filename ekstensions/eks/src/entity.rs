use serde::{Serialize, Deserialize};



#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Hash, Default)]
pub struct Entity {
	// Can pack these into one u32 (entity is u24 and gen is u8)
	index: u32,
	generation: u8,
}
impl Entity {
	pub fn new(id: impl Into<u32>, generation: impl Into<u8>) -> Self {
		Self {
			index: id.into(), 
			generation: generation.into(),
		}
	}
	pub fn get_index(&self) -> usize {
		self.index as usize
	}
	pub fn set_index(&mut self, index: impl Into<u32>) -> &mut Self {
		self.index = index.into();
		self
	}
	pub fn get_generation(&self) -> usize {
		self.generation as usize
	}
	pub fn set_generation(&mut self, generation: impl Into<u8>) -> &mut Self {
		self.generation = generation.into();
		self
	}
	pub fn inc_generation(&mut self) -> &mut Self {
		self.generation += 1;
		self
	}
}
impl std::fmt::Display for Entity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Entity {} ({})", self.get_index(), self.get_generation())?;
		Ok(())
	}
}


#[derive(Debug, Default)]
pub struct EntitySparseSet {
	next: usize,
	// If not matches index, then points to next and this is recyclable
	entities: Vec<Entity>,
}
impl EntitySparseSet {
	fn pop_next(&mut self) -> Option<Entity> {
		// If in range and index doesn't match
		let new_next = self.entities.get(self.next)
			.and_then(|e| (e.get_index() != self.next).then_some(e.get_index()));

		if let Some(new_next) = new_next {
			let old_next = self.next;
			let e = self.entities[old_next].set_index(old_next as u32).inc_generation().clone();
			self.next = new_next;
			Some(e)
		} else {
			None
		}
	}
	pub fn spawn(&mut self) -> Entity {
		if let Some(entity) = self.pop_next() {
			entity
		} else {
			let e = Entity::new(self.entities.len() as u32, 0);
			self.entities.push(e);
			e
		}
	}
	pub fn remove(&mut self, entity: Entity) {
		self.entities[entity.get_index()].set_index(self.next as u32);
		self.next = entity.get_index();
	}
	pub fn clear(&mut self) {
		self.entities.clear();
		self.next = 0;
	}
	pub fn len(&self) -> usize {
		self.entities.len()
	}
}


// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	#[test]
// 	fn test_entity_sparse_set() {
// 		let mut set = EntitySparseSet::default();
// 		let e0 = set.next();
// 		let e1 = set.next();
// 		let e2 = set.next();
// 		println!("{set:#?}");

// 		set.remove(e1);
// 		println!("-------");
// 		println!("{set:#?}");

// 		let e11 = set.next();
// 		println!("-------");
// 		println!("{set:#?}");
// 		// let mut set = BasicSparseArray::default();
// 		// let e0 = set.insert(Entity { id: 0, generation: 0 });
// 	}
// }
