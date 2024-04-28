use ekstensions::prelude::*;
use krender::vertex::{InstanceAttributeSource, FetchedInstanceAttributeSource};
use krender::prelude::{InstanceComponentProvider, InstanceDataProvider};
use ekstensions::eks::sparseset::UntypedSparseSet;


pub struct WorldWrapper<'world> {
	pub world: &'world World,
}
impl<'world> InstanceDataProvider<'world, Entity> for WorldWrapper<'world> {
	fn get_storage(
		&self, component_id: impl AsRef<str>,
	) -> Option<impl InstanceComponentProvider<'world, Entity>> {
		let component_id = component_id.as_ref().to_string();
		Some(StorageWrapper {
			storage: unsafe { self.world.component_hack(component_id) },
		})
	}

	fn get_resource(&self, resource_id: impl Into<String>) -> Option<&'world [u8]> {
		Some(unsafe { self.world.resource_hack(resource_id.into()).as_ref() })
	}

	fn fetch_source(
		&self, attribute: &InstanceAttributeSource,
	) -> Option<FetchedInstanceAttributeSource<'world, Entity>> {
		Some(match attribute {
			InstanceAttributeSource::Component(component_id) => {
				// I don't know why it mut be this way, but it must be this way
				let s = StorageWrapper {
					storage: unsafe { self.world.component_hack(component_id) },
				};
				FetchedInstanceAttributeSource::<'world, _>::Component(Box::new(s))
			},
			InstanceAttributeSource::Resource(resource_id) => FetchedInstanceAttributeSource::<'world, _>::Resource(self.get_resource(resource_id)?),
		})
	}
}


/// Uses an unchecked reference 
/// Either I don't understand AtomicRefs or the borrow checker doesn't understand AtomicRefs.
/// I think AtomicRef<'a, T> <=> &'a T but it disagrees. 
/// I don't know who is correct. 
struct StorageWrapper<'borrow> {
	storage: &'borrow UntypedSparseSet,
}
impl<'borrow> InstanceComponentProvider<'borrow, Entity> for StorageWrapper<'borrow> {
	fn get_component(&self, entity: Entity) -> Option<&'borrow [u8]> {
		self.storage.get(entity)
	}
}
