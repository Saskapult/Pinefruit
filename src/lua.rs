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

		let testfn_caller = self.lua.create_function(|_, ()| -> LuaResult<u32> {
			Ok(testfn())
		}).unwrap();
		globals.set("testfn", testfn_caller)?;

		let test_panic_caller = self.lua.create_function(|_, ()| -> LuaResult<()> {
			panic!("test panic!");
		}).unwrap();
		globals.set("test_panic", test_panic_caller)?;

		let play_sound_caller = self.lua.create_function(|_, ()| {
			play_sound("BWAAAMMP").unwrap();
			Ok(())
		}).unwrap();
		globals.set("play_sound", play_sound_caller)?;

		// Get voxel
		// Set voxel

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

	pub fn repl_loop(&self) -> Result<()> {
		println!("-- Begin REPL --");
		loop {
			self.repl()?;
			// Only do one because idk
			break;
		}
		println!("-- End REPL --");
		
		Ok(())
	}

	fn repl(&self) -> mlua::Result<()> {
		use std::io::Write;

		let mut command = String::new();
		print!("> ");
		std::io::stdout().flush().unwrap();
		loop {
			std::io::stdin().read_line(&mut command).unwrap();

			match self.lua.load(&command).eval::<mlua::MultiValue>() {
				Ok(values) => {
					println!(
						"{}", 
						values
							.iter()
							.map(|v| format!("{:?}", v))
							.collect::<Vec<_>>()
							.join("\t")
					);
					break;
				},
				Err(mlua::Error::SyntaxError { incomplete_input: true, ..}) => {
					command.push_str("\n");
					print!(">>\t");
					std::io::stdout().flush().unwrap();
				}
				Err(e) => {
					return Err(e);
				}
			}
		}
		Ok(())
	}
}



#[derive(Debug, Default)]
struct UserThing {
	a: u32,
	b: Vec<u8>,
}
impl LuaUserData for UserThing {
	fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("a", |_, this| Ok(this.a));
        fields.add_field_method_set("a", |_, this, val| {
            this.a = val;
            Ok(())
        });
    }

    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("twoa", |_, this, ()| Ok(2 * this.a));

        // Constructor
        methods.add_meta_function(mlua::MetaMethod::Call, |_, ()| Ok(UserThing::default()));
    }
}



fn testfn() -> u32 {
	42
}



fn play_sound(sound: impl AsRef<str>) -> Result<()> {
	println!("You hear a '{}'", sound.as_ref());
	Ok(())
}