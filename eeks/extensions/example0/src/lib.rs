use eeks::prelude::*;

#[macro_use]
extern crate log;


#[derive(Debug, Component, PartialEq, Eq, Clone, Copy)]
#[sda(lua = true, commands = true)]
pub struct ExampleComponent(pub u32);
impl StorageCommandExpose for ExampleComponent {
	fn command(&mut self, command: &[&str]) -> anyhow::Result<()> {
		match command[0] {
			"echo" => println!("echo"),
			"get" => println!("{}", self.0),
			"inc" => self.0 += 1,
			_ => {},
		}
		Ok(())
	}
}
impl mlua::UserData for ExampleComponent {
	fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
		methods.add_method("test2", |_lua, this, ()| {
			info!("Called test2");
			Ok(())
		});
	}
}


#[derive(Debug, Resource, PartialEq, Eq, Clone, Copy)]
#[sda(lua = true, commands = true)]
pub struct ExampleResource(pub u32);
impl StorageCommandExpose for ExampleResource {
	fn command(&mut self, command: &[&str]) -> anyhow::Result<()> {
		match command[0] {
			"test" => println!("test"),
			"get" => println!("{}", self.0),
			"inc" => self.0 += 1,
			_ => {},
		}
		Ok(())
	}
}
impl mlua::UserData for ExampleResource {
	fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
		methods.add_method("test0", |_lua, this, ()| {
			info!("Called test0");
			Ok(())
		});
		methods.add_method("test1", |_lua, this, s: String| {
			info!("Called test1");
			info!("String is '{}'", s);
			Ok(42)
		});
	}
}


pub fn example_system(
	mut excs: CompMut<ExampleComponent>,
) {
	for exc in (&mut excs).iter() {
		exc.0 += 1;
	}
}


#[info]
pub fn info() -> Vec<String> {
	env_logger::init();
	info!("Example0 deps");
	vec![]
}


#[systems]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	info!("Example0 systems");
	loader.system("group", "example_system", example_system);
}


#[load]
pub fn load(storages: &mut ExtensionStorageLoader) {
	info!("Example0 load");
	storages.component::<ExampleComponent>();
	storages.resource(ExampleResource(0));
}
