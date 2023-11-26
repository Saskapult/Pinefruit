use atomic_refcell::{AtomicRefCell, AtomicRef, AtomicRefMut, BorrowError, BorrowMutError};
use crate::{Component, Resource, sparseset::{SparseSet, UntypedSparseSet}};


// I've created the framework for snappable things (persist data over module reloading)
// but I can't figure out how to extract the serialization/deserialization functions. 
// Hopefully I won't need to use those yet. 
// You might not be so lucky. 
// You might have specialization though so go deal with it. 
// I am limited by the technology of my time. 


// struct SnappingInfo {
// 	// Serializes the data (sparse set or resource)
// 	pub serialization: fn(*mut ()) -> Vec<u8>,
// 	// Deserializes the data (you must still box it)
// 	pub deserialization: fn(&[u8]) -> Option<*mut ()>,
// }


// pub trait Container<UT> {
// 	fn typed_ref<T>(&self) -> Result<AtomicRef<T>, BorrowError>;
// 	fn typed_mut<T>(&self) -> Result<AtomicRefMut<T>, BorrowError>;
// 	fn untyped_ref(&self) -> Result<AtomicRef<UT>, BorrowError>;
// 	fn untyped_mut(&self) -> Result<AtomicRefMut<UT>, BorrowError>;
// 	fn is_borrowed(&self) -> bool;
// }


/// If a component is defined in a dll or a wasm module, then we don't know how to drop it. 
/// This lets us store that a sparse set as undefined data and also have a drop function. 
/// 
/// It should run correctly *if* the pointers remain valid (which I'm not sure of). 
pub(crate) struct SparseSetContainer {
	pub data: AtomicRefCell<UntypedSparseSet>,
	
	/// This should be some if T is [Snappable]. 
	/// If it's some and the provider module is going to be reloaded
	/// then we can take a snapshot of its data. 
	/// Once the new module is loaded we can feed the data into the deserialization funtion (assuming it has one). 
	/// You will want to have that return a result (for if the data format changes). 
	/// 
	/// Untyped sparse set changed some of that info, but it's still good info
	// snapping: Option<SnappingInfo>,
	//
	// Update: this should now be in UntypedSparseSet

	// Size of component, used to panic if something is obviously wrong
	component_size_bytes: usize,
	// Name of component, used to panic if something is very wrong
	component_name: &'static str, // Should this be a String? Nah it's probably fine
}
impl SparseSetContainer {
	pub fn untyped_ref(&self) -> Result<AtomicRef<'_, UntypedSparseSet>, BorrowError> {
		self.data.try_borrow()
	}
	pub fn untyped_mut(&self) -> Result<AtomicRefMut<'_, UntypedSparseSet>, BorrowMutError> {
		self.data.try_borrow_mut()
	}

	// We can't use the type id of the component to ensure that it is correct. 
	pub fn check_guards<C: Component>(&self) {
		assert_eq!(self.component_size_bytes, std::mem::size_of::<C>(), "Component size differs!");
		assert_eq!(self.component_name, C::COMPONENT_NAME, "Component name differs!");
	}

	pub fn typed_ref<C: Component>(&self) -> Result<AtomicRef<'_, SparseSet<C>>, BorrowError> {
		self.check_guards::<C>();
		Ok(AtomicRef::map(
			self.data.try_borrow()?, 
			|p| unsafe { p.inner_ref_of::<C>() }
		))
	}
	pub fn typed_mut<C: Component>(&self) -> Result<AtomicRefMut<'_, SparseSet<C>>, BorrowMutError> {
		self.check_guards::<C>();
		Ok(AtomicRefMut::map(
			self.data.try_borrow_mut()?, 
			|p| unsafe { p.inner_mut_of::<C>() }
		))
	}
}
impl<'a, C: Component> From<SparseSet<C>> for SparseSetContainer {
	// uses from and not into becuase drop_as should be private
	fn from(value: SparseSet<C>) -> Self {
		// let b = Box::new(value);
		// let ptr = Box::into_raw(b) as *mut ();
		let data = AtomicRefCell::new(value.into());
		// let drop_fn = SparseSetContainer::drop_as::<C>;

		let component_size_bytes = std::mem::size_of::<C>();
		let component_name = C::COMPONENT_NAME;
		SparseSetContainer { 
			data, component_size_bytes, component_name, 
		}
	}
}


pub(crate) struct ResourceContainer {
	data: AtomicRefCell<*mut u8>,
	drop_fn: fn(&mut Self), // This should be some variant of drop_as (as defined externally)
	
	// Here if R is serializable but idk how to do that
	// Maybe snappable has a constant for it?
	// But can we know if its Ser/De in the proc macro?
	// snapping: Option<SnappingInfo>,
	
	data_size_guard: usize,
	resource_name: &'static str,
}
impl ResourceContainer {
	pub fn check_guards<R: Resource>(&self) {
		assert_eq!(self.data_size_guard, std::mem::size_of::<R>(), "Component size differs!");
		assert_eq!(self.resource_name, R::RESOURCE_NAME, "Component name differs!");
	}

	pub fn resource_ref<R: Resource>(&self) -> Result<AtomicRef<'_, Option<R>>, BorrowError> {
		self.check_guards::<Option<R>>();
		Ok(AtomicRef::map(
			self.data.try_borrow()?, 
			|&p| unsafe { Box::leak(Box::from_raw(p as *mut Option<R>)) }
		))
	}

	pub fn resource_mut<R: Resource>(&self) -> Result<AtomicRefMut<'_, Option<R>>, BorrowMutError> {
		self.check_guards::<Option<R>>();
		Ok(AtomicRefMut::map(
			self.data.try_borrow_mut()?, 
			|&mut p| unsafe { Box::leak(Box::from_raw(p as *mut Option<R>)) }
		))
	}

	pub fn untyped_ref(&self) -> Result<AtomicRef<'_, &'_ [u8]>, BorrowError> {
		// std::slice::from_raw_parts(&mut self.data.get_mut(), self.data_size_guard)
		todo!()
	}

	// Should only ever be called from drop code
	fn drop_as<R: Resource>(&mut self) {
		println!("Drop as R with name {}", R::RESOURCE_NAME);
		let ptr = *self.data.get_mut();
		let resource = unsafe { (ptr as *mut R).read() };
		drop(resource);
	}
}
impl Drop for ResourceContainer {
	fn drop(&mut self) {
		(self.drop_fn)(self); 
	}
}
impl<R: Resource> From<R> for ResourceContainer {
	fn from(value: R) -> Self {
		let b = Box::new(value);
		let ptr = Box::into_raw(b) as *mut u8;
		let data = AtomicRefCell::new(ptr);
		let drop_fn = ResourceContainer::drop_as::<R>;

		let data_size_guard = std::mem::size_of::<R>();
		let resource_name = R::RESOURCE_NAME;

		ResourceContainer { 
			data, drop_fn, data_size_guard, resource_name, 
		}
	}
}
