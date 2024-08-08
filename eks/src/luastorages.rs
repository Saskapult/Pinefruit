use std::sync::Arc;
use atomic_refcell::AtomicRefMut;
use crate::{entity::Entity, query::Queriable, sparseset::SparseSet, World, WorldBorrowError, WorldStorage};



pub struct LuaStorages {
	components: WorldStorage<SparseSetWrapper>,
	resources: WorldStorage<mlua::RegistryKey>,
}
impl LuaStorages {
	pub fn new() -> Self {
		Self {
			components: WorldStorage::new(),
			resources: WorldStorage::new(),
		}
	}

	pub(super) fn create_resource(&self, lua: &mlua::Lua, id: impl AsRef<str>, value: mlua::Table) {
		let k = lua.create_registry_value(value).unwrap();
		self.resources.insert(id.as_ref().to_string(), k);
	}

	pub(super) fn get_resource_key(&self, id: impl AsRef<str>) -> Result<AtomicRefMut<mlua::RegistryKey>, WorldBorrowError> {
		let v = self.resources.get(id.as_ref())
			.map(|r| unsafe { &*r })?;
		let b = v.try_borrow_mut().map_err(|_| WorldBorrowError::Exclusion(id.as_ref().to_string()))?;
		Ok(b)
	}

	pub(super) fn get_component_key(&self, id: impl AsRef<str>) -> Result<AtomicRefMut<SparseSetWrapper>, WorldBorrowError> {
		let v = self.components.get(id.as_ref())
			.map(|r| unsafe { &*r })?;
		let b = v.try_borrow_mut().map_err(|_| WorldBorrowError::Exclusion(id.as_ref().to_string()))?;
		Ok(b)
	}

	pub fn add_scoped_methods<'a>(&'a mut self, lua: &mlua::Lua, scope: &'a mlua::Scope<'_, 'a>) -> anyhow::Result<mlua::AnyUserData> {
		lua.globals().set("get_component", scope.create_function_mut(|_lua, id: String| {
			let g = self.components.get(&id)
				.map(|r| unsafe { &mut *r }).unwrap();
			let m = g.get_mut();
			let u = scope.create_userdata_ref_mut(m)?;
			Ok(u)
		})?)?;
		
		let s = scope.create_userdata_ref(self)?;
		Ok(s)
	}

}
impl mlua::UserData for LuaStorages {
	fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
		methods.add_method("create_resource", |lua, this, (id, value): (String, mlua::Table)| {
			this.create_resource(lua, &id, value);
			Ok(())
		});
		methods.add_method("get_resource", |lua, this, id: String| {
			// Becuase the reference is dropped immediately, we don't actually do any borrow checking when lua requests values 
			// This is perfectly fine (or as good as it can be), as lua already passes tables by reference and we can't really enforce borrow requirements 
			let r = this.get_resource_key(&id).unwrap();
			let g = lua.registry_value::<mlua::Table>(&*r)?;
			Ok(g)
		})
	}
}


pub struct Lua<'s> {
	// In upcoming versions of mlua, mlua::Lua will be Clone
	// Todo: update mlua and remove this Arc 
	pub lua: Arc<mlua::Lua>,
	pub(crate) storages: AtomicRefMut<'s, LuaStorages>,
}
impl<'s> Lua<'s> {
	pub fn get_resource(&self, id: impl AsRef<str>) -> Result<BorrowedTable, WorldBorrowError> {
		let borrow = self.storages.get_resource_key(id.as_ref())?;
		let table = self.lua.registry_value::<mlua::Table>(&borrow).unwrap();
		Ok(BorrowedTable {
			table, 
			borrow,
		})
	}
}
impl<'s> std::ops::Deref for Lua<'s> {
	type Target = LuaStorages;
	fn deref(&self) -> &Self::Target {
		self.storages.deref()
	}
}
impl<'s> std::ops::DerefMut for Lua<'s> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.storages
	}
}
impl<'q> Queriable<'q> for Lua<'q> {
	type Item = Self;
	fn query(world: &'q World) -> Self {
		world.lua_borrow().unwrap()
	}
}


pub struct BorrowedTable<'s, 'lua> {
	table: mlua::Table<'lua>,
	borrow: AtomicRefMut<'s, mlua::RegistryKey>,
}
impl<'s, 'lua> std::ops::Deref for BorrowedTable<'s, 'lua> {
	type Target = mlua::Table<'lua>;
	fn deref(&self) -> &Self::Target {
		&self.table
	}
}
impl<'s, 'lua> std::ops::DerefMut for BorrowedTable<'s, 'lua> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.table
	}
}


pub struct SparseSetWrapper(SparseSet<mlua::RegistryKey>);
impl mlua::UserData for SparseSetWrapper {
	fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
		methods.add_method_mut("get", |lua, this, entity: mlua::UserDataRef<Entity>| {
			let v = this.0.get(*entity)
				.map(|key| lua.registry_value::<mlua::Table>(key).unwrap());
			if let Some(t) = v {
				Ok(mlua::Value::Table(t))
			} else {
				Ok(mlua::Nil)
			}
		});
		methods.add_method_mut("insert", |lua, this, (entity, value): (mlua::UserDataRef<Entity>, mlua::Table)| {
			if let Some(key) = this.0.get(*entity) {
				lua.replace_registry_value(key, value)?;
			} else {
				let key = lua.create_registry_value(value)?;
				this.0.insert(*entity, key);
			}
			Ok(())
		});
		methods.add_method_mut("remove", |lua, this, entity: mlua::UserDataRef<Entity>| {
			if let Some(key) = this.0.remove(*entity) {
				let t = lua.registry_value::<mlua::Table>(&key)?;
				lua.remove_registry_value(key)?;
				Ok(mlua::Value::Table(t))
			} else {
				Ok(mlua::Nil)
			}
		});
	}
}
