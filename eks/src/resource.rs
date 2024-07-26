use crate::Resource;


// Should be UntypedResource and ResourceContainer
// But it is an unholy mixture of the two
pub struct UntypedResource {
	data: *mut u8, // Borrowable pointer to Box<Resource>
	data_drop: fn(*mut u8), // This should be some variant of drop_as (as defined externally)
	data_serde: Option<(
		// These are from the derived functions for Resource
		// Should return result 
		fn(*const u8, &mut Vec<u8>) -> bincode::Result<()>, 
		fn(&[u8]) -> bincode::Result<*mut u8>, // Box<Resource>
	)>,
	data_renderdata: Option<fn(&Self, &mut Vec<u8>)>,
	data_command: *const u8,
	data_lua: *const u8,
	
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

	// For taking snapshots
	pub fn serialize(&self, buffer: &mut Vec<u8>) -> bincode::Result<()> {
		// call serialization function
		// Include name and data size guard? No becuase serde has that already I think
		let (f, _) = self.data_serde
			.expect("UntypedResource has no serialization function!");
		(f)(self.data, buffer)
	}
	pub fn deserialize(&mut self, buffer: &[u8]) -> bincode::Result<()> {
		// Drop old data
		(self.data_drop)(self.data); 
		// Load new data
		let (_, f) = self.data_serde
			.expect("UntypedResource has no serialization function!");
		let data = f(buffer)?;
		self.data = data;
		Ok(())
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

	// Used by krender to extract render data and append it to a buffer 
	pub fn render_extend(&self, buffer: &mut Vec<u8>) {
		if let Some(f) = self.data_renderdata {
			(f)(self, buffer);
		} else {
			buffer.extend_from_slice(self.inner_raw());
		}
	}

	pub fn command(&mut self, command: &[&str]) -> anyhow::Result<()> {
		let p = self.data;
		let f: fn(*const u8, &[&str]) -> anyhow::Result<()> = unsafe { std::mem::transmute(self.data_command) };
		(f)(p, command)
	}

	pub fn create_scoped_ref<'lua, 'scope>(&'scope mut self, scope: &mlua::Scope<'lua, 'scope>) -> Option<Result<mlua::AnyUserData<'lua>, mlua::Error>> {
		let f: fn(*const u8, &mlua::Scope<'lua, 'scope>) -> Option<Result<mlua::AnyUserData<'lua>, mlua::Error>> = unsafe { std::mem::transmute(self.data_lua) };
		f(self.data, scope)
	}

	// Should only ever be called from drop code
	fn drop_data_as<R: Resource>(data: *mut u8) {
		trace!("Dropping untype resource as resource of {}", R::STORAGE_ID);
		let resource = unsafe { Box::from_raw(data as *mut R) };
		drop(resource);
	}
}
impl Drop for UntypedResource {
	fn drop(&mut self) {
		(self.data_drop)(self.data); 
	}
}
impl<R: Resource> From<R> for UntypedResource {
	fn from(value: R) -> Self {
		let b = Box::new(value);
		let data = Box::into_raw(b) as *mut u8;
		
		let data_size = std::mem::size_of::<R>();
		let name = R::STORAGE_ID;

		UntypedResource { 
			data, 
			data_drop: Self::drop_data_as::<R>, 
			data_serde: None, //R::get_serde_fns(),
			data_renderdata: None,
			data_command: R::command as *const u8,
			data_lua: R::create_scoped_ref as *const u8,
			data_size, 
			name, 
		}
	}
}


#[cfg(test)]
mod tests {
	use crate::prelude::*;
	use super::UntypedResource;

	#[derive(Debug, Resource, serde::Serialize, serde::Deserialize)]
	// #[storage_options(snap = true)]
	struct ResourceA(pub u32);

	#[test]
	fn test_serde() {
		let mut storage: UntypedResource = ResourceA(42).into();

		assert!(storage.is_serializable());

		let mut buffer = Vec::new();
		storage.serialize(&mut buffer).unwrap();

		let res = storage.inner_mut::<ResourceA>();
		res.0 += 1;

		assert_eq!(43, res.0);

		storage.deserialize(&buffer).unwrap();
		let res = storage.inner_mut::<ResourceA>();
		assert_eq!(42, res.0);
	}
}
