use std::collections::HashSet;
use std::path::{Path, PathBuf};
use mlua::prelude::*;
use anyhow::*;




#[derive(Debug)]
pub struct ScriptManager {
	lua: Lua,
	loaded: HashSet<PathBuf>
}
impl ScriptManager {
	pub fn new() -> Self {
		Self {
			lua: Lua::new(), 
			loaded: HashSet::new(),
		}
	}

	/// Loads bindings to rust fucntions into the lua context
	pub fn init_bindings(&mut self) -> Result<()> {
		let globals = self.lua.globals();

		let testfn_caller = self.lua.create_function(|_, ()| {
			Ok(testfn())
		}).unwrap();
		globals.set("testfn", testfn_caller)?;

		Ok(())
	}

	/// Loads a file into the context
	pub fn load(&mut self, path: impl AsRef<Path>) -> Result<()> {
		let path = path.as_ref();

		let canonical_path = path.canonicalize()
			.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
		let content = std::fs::read_to_string(&canonical_path)
			.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
		
		let path_string = canonical_path.clone().into_os_string().into_string().unwrap();
		self.lua.load(&content)
			.set_name(&path_string)?
			.exec()?;
		self.loaded.insert(canonical_path);

		Ok(())
	}

	/// Unloads a file from the context (it's more complex than that, see mlua docs)
	pub fn unload(&mut self, path: impl AsRef<Path>) -> Result<()> {
		let path = path.as_ref();
		let canonical_path = path.canonicalize()
			.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
		
		self.loaded.remove(&canonical_path);

		let path_string = canonical_path.into_os_string().into_string().unwrap();
		self.lua.unload(&path_string)?;

		Ok(())
	}
}



fn testfn() -> u32 {
	42
}