use crate::Resource;


// Should be UntypedResource and ResourceContainer
// But it is an unholy mixture of the two
pub struct UntypedResource {
	data: *mut u8, // Borrowable pointer to Box<Resource>
	fn_drop: fn(&mut Self), // This should be some variant of drop_as (as defined externally)
	fn_serde: Option<(fn(&mut Self, &mut Vec<u8>), fn(&mut Self, &mut Vec<u8>))>,
	fn_renderdata: Option<fn(&mut Self, &mut Vec<u8>)>,
	
	data_size: usize,
	name: &'static str,
}
impl UntypedResource {
	pub fn check_guards<R: Resource>(&self) {
		assert_eq!(self.data_size, std::mem::size_of::<R>(), "Component size differs!");
		assert_eq!(self.name, R::STORAGE_ID, "Component name differs!");
	}

	pub fn into_inner<R: Resource>(self) -> R {
		self.check_guards::<R>();
		let b = unsafe { Box::from_raw(self.data as *mut R) };
		// We need to use forget here beucase otherwise the drop code will run and we will free the data memory 
		// It will not leak memory becuase `data` is the only heap-allocated field  
		std::mem::forget(self);
		*b
	}

	pub fn inner_ref<R: Resource>(&self) -> &R {
		self.check_guards::<R>();
		unsafe { &*(self.data as *mut R) }
	}

	pub fn inner_mut<R: Resource>(&self) -> &mut R {
		self.check_guards::<R>();
		unsafe { &mut *(self.data as *mut R) }
	}

	pub fn inner_raw(&self) -> &[u8] {
		unsafe { std::slice::from_raw_parts(self.data, self.data_size) }
	}

	// Should only ever be called from drop code
	fn drop_as<R: Resource>(&mut self) {
		info!("Dropping untype resource as resource of {}", R::STORAGE_ID);
		let resource = unsafe { Box::from_raw(self.data as *mut R) };
		drop(resource);
	}
}
impl Drop for UntypedResource {
	fn drop(&mut self) {
		(self.fn_drop)(self); 
	}
}
impl<R: Resource> From<R> for UntypedResource {
	fn from(value: R) -> Self {
		let b = Box::new(value);
		let data = Box::into_raw(b) as *mut u8;
		
		let fn_drop = UntypedResource::drop_as::<R>;

		let data_size = std::mem::size_of::<R>();
		let name = R::STORAGE_ID;

		UntypedResource { 
			data, 
			fn_drop, 
			fn_serde: None,
			fn_renderdata: None,
			data_size, 
			name, 
		}
	}
}
