
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum BorrowType {
	Read,
	Write,
	// Upgradable,
	Exclude,
}


/// Views are used to build queries.
/// 
/// "I want x and y but not z."
pub struct View {
	items: Vec<(String, BorrowType)>, 
}
impl View {
	pub fn new() -> Self {
		Self { items: Vec::new() }
	}
	pub fn include(mut self, component: impl Into<String>) -> Self {
		self.items.push((component.into(), BorrowType::Read));
		self
	}
	pub fn include_mut(mut self, component: impl Into<String>) -> Self {
		self.items.push((component.into(), BorrowType::Write));
		self
	}
	// Could replace bool with internal enum (read, write, upgradable, (exclude?))
	// pub fn include_upgradable(mut self, component: impl Into<String>) -> Self {
	// 	self.include.push((component.into(), true));
	// 	self
	// }
	pub fn exclude(mut self, component: impl Into<String>) -> Self {
		self.items.push((component.into(), BorrowType::Exclude));
		self
	}
}


pub struct Query<'a> {
	// Pick main (shortest) storage
	// Iterate over that, fetching from others
	// Currently I assume that the first entry is the shortest storage
	storages: Vec<(String, BorrowType, ComponentID, &'a SparseSetUntyped)>,
	
	// Current component data
	// Should be a raw pointer to allow for mutation
	// Can we add a wrapper to guard against unflagged mutation?
	// Look at how std or parking_lot does it. 
	components: Vec<BorrowGuard>, 
	
	// Current index in main storage
	// We should start at the end of the thing and then move to the start
	// This way we can delete things whenever we want to
	index: usize,
}
impl<'a> Query<'a> {
	// We can't implement iterator (all iterator items must be able to be alive at the same time). 
	// We can do this instead. 
	// Use a "while let Some(items) = query.next_items()" to loop over items. 
	pub fn next_items(&mut self) -> Option<(Entity, &mut [BorrowGuard])> {
		self.components.clear();
		if let Some(&entity) = self.storages[0].3.entities.get(self.index) {
			self.index += 1;
			for &(_, borrow_type, _, storage) in self.storages.iter() {
				match borrow_type {
					BorrowType::Read => {
						if let Some(data) = storage.get(entity) {
							let pointer = data.as_ptr();
							self.components.push(BorrowGuard { 
								borrow_type, pointer, data_size: storage.component_size_bytes, 
							});
						} else {
							return None;
						}
					},
					BorrowType::Write => {
						todo!()
					},
					BorrowType::Exclude => if storage.contains(entity) { return None; },
				}
			}
			Some((
				entity, 
				self.components.as_mut_slice(),
			))
		} else {
			None
		}
	}
}




pub struct BorrowGuard {
	borrow_type: BorrowType, // Never exclude
	pointer: *const u8,
	data_size: usize,
}
impl BorrowGuard {
	/// Reads (unsafely!) the data at the end of the pointer. 
	/// Also gives it a lifetime so you can feel better about yourself. 
	pub fn read<'a>(&'a self) -> &'a [u8] {
		let slice = unsafe { std::slice::from_raw_parts(self.pointer, self.data_size) };
		slice
	}

	/// Gets (no safety!) a mutable reference to the end of the pointer. 
	/// If this borrow is not writable, then it returns None. 
	pub fn write<'a>(&'a mut self) -> Option<&'a mut [u8]> {
		match self.borrow_type {
			BorrowType::Write => {
				let slice = unsafe { std::slice::from_raw_parts_mut(self.pointer as *mut u8, self.data_size) };
				Some(slice)
			},
			_ => None,
		}
	}
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_query() {
		let mut world = World::default();
		world.insert_storage("storage 0", 1);
		world.insert_storage("storage 1", 1);
		world.insert_storage("storage 2", 1);

		let entity_0 = Entity {
			id: 0,
			generation: 0,
		};
		let entity_1 = Entity {
			id: 1,
			generation: 0,
		};
		let entity_2 = Entity {
			id: 2,
			generation: 0,
		};

		let storage_0 = world.storage_mut("storage 0").unwrap();
		storage_0.storage.insert(entity_0, &[10]);
		storage_0.storage.insert(entity_2, &[2]);

		let storage_1 = world.storage_mut("storage 1").unwrap();
		storage_1.storage.insert(entity_1, &[1]);
		storage_1.storage.insert(entity_2, &[2]);

		let storage_2 = world.storage_mut("storage 2").unwrap();
		storage_2.storage.insert(entity_0, &[102]);
		storage_2.storage.insert(entity_2, &[2]);

		let view = View::new()
			.include("storage 0")
			.include("storage 2")
			.exclude("storage 1");

		let mut query = world.query_of(&view);
		
		while let Some((entity, components)) = query.next_items() {
			let c0 = &components[0];
			let c1 = &components[1];
			println!("{entity:?} - {:?}, {:?}", c0.read(), c1.read());
		}
	}
}

pub fn query_of<'a>(&'a mut self, view: &View) -> Query<'a> {
	let storages = view.items.iter()
		.cloned()
		.map(|(name, borrow_type)| {
			let component = self.components.iter().find(|c| c.name == name).unwrap();
			(name, borrow_type, component.id, &component.storage)
		}).collect();

	let q = Query {
		storages,
		components: Vec::new(),
		index: 0,
	};
	q
}