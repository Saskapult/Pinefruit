use std::{collections::HashMap, path::{Path, PathBuf}, time::SystemTime};
use anyhow::{anyhow, Context};
use eks::{prelude::*, system::SystemFunction};
pub use eks;
pub mod prelude {
	pub use eks::prelude::*;
}

#[macro_use]
extern crate log;


/// Used by load functions to register and describe provisions. 
pub struct ExtensionLoader<'a> {
	world: &'a mut World, 
	provisions: ExtensionProvisions,
}
impl<'a> ExtensionLoader<'a> {
	pub fn system<S: SystemFunction<'static, (), Q, R> + Copy + 'static, R, Q: Queriable<'static>>(
		&mut self, 
		group: impl AsRef<str>,
		name: impl AsRef<str>, 
		function: S,
	) -> &mut ExtensionSystem {
		let i = self.provisions.systems.len();
		self.provisions.systems.push(ExtensionSystem::new::<S, R, Q>(group, name, function));
		self.provisions.systems.get_mut(i).unwrap()
	}

	pub fn component<C: Component>(&mut self) -> &mut Self {
		self.world.register_component::<C>();
		self.provisions.components.push(C::STORAGE_ID.to_string());
		self
	}

	pub fn resource<R: Resource>(&mut self, r: R) -> &mut Self {
		self.world.insert_resource(r);
		self.provisions.resources.push(R::STORAGE_ID.to_string());
		self
	}
}


/// Used by load functions to describe systems. 
pub struct ExtensionSystem {
	group: String,
	id: String,
	pointer: Box<dyn Fn(*const World)>,
	run_after: Vec<String>,
	// run_before: Vec<String>, 
}
impl ExtensionSystem {
	// This is just temporary
	// New should take a system function, extract its name and pointer, and then retrun this thing
	pub fn new<S: SystemFunction<'static, (), Q, R> + Copy + 'static, R, Q: Queriable<'static>>(
		group: impl AsRef<str>, id: impl AsRef<str>, s: S,
	) -> Self {

		// I hate SystemFuction
		// I hate lifetimes
		// I feel hate 
		let closure = move |world: *const World| unsafe {
			let world = &*world;
			s.run_system((), world);
		};

		Self {
			group: group.as_ref().to_string(),
			id: id.as_ref().to_string(),
			pointer: Box::new(closure),
			run_after: Vec::new(),
			// run_before: Vec::new(),
		}
	}

	pub fn run_after(&mut self, id: impl AsRef<str>) -> &mut Self {
		self.run_after.push(id.as_ref().to_string());
		self
	}

	// pub fn run_before(&mut self, id: impl AsRef<str>) -> &mut Self {
	// 	self.run_before.push(id.as_ref().to_string());
	// 	self
	// }
}
impl std::fmt::Debug for ExtensionSystem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ExtensionSystem")
			.field("id", &self.id)
			.field("run_after", &self.run_after)
			// .field("run_before", &self.run_before)
			.finish()			
	}
}


#[derive(Debug, PartialEq, Eq)]
pub enum DirtyLevel {
	Clean,
	Reload, // Load .so file again
	Rebuild, // Rebuild whole project
}


#[derive(Debug, Default)]
pub struct ExtensionProvisions {
	// (tick, fn, run_after, run_before)
	pub systems: Vec<ExtensionSystem>,
	// Just component ID, which is a string
	// What about resources? Should be here too
	pub components: Vec<String>,
	pub resources: Vec<String>,
}


pub struct ExtensionEntry {
	pub name: String,
	pub path: PathBuf,

	// Load library
	// Library must be dropped before overwiting .so file on disk? (like recompile)
	// If so, please copy to another folder when loading
	pub library: libloading::Library,
	pub read_at: SystemTime, 
	pub load_dependencies: Vec<String>,

	pub provides: Option<ExtensionProvisions>,
}
impl ExtensionEntry {
	// Reads extension from disk and compiles 
	pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		let cargo_toml_path = path.as_ref().join("Cargo.toml");
		let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)
			.with_context(|| "failed to read cargo.toml")?;
		let cargo_toml_table = cargo_toml_content.parse::<toml::Table>()
			.with_context(|| "failed to parse cargo.toml")?;

		// Require rlib && dylib 
		let is_dylib = cargo_toml_table.get("lib")
			.and_then(|v| v.as_table())
			.and_then(|t| t.get("crate-type"))
			.and_then(|v| v.as_array())
			.and_then(|v| Some(
				v.contains(&toml::Value::String("rlib".to_string())) 
				&&
				v.contains(&toml::Value::String("dylib".to_string())) 
			))
			.unwrap_or(false);
		if !is_dylib {
			error!("Not rlib dylib!");
			panic!();
		}

		// Require sccache 
		let sccache_enabled = cargo_toml_table.get("build")
			.and_then(|v| v.as_table())
			.and_then(|t| t.get("rustc-wrapper"))
			.and_then(|v| v.as_str())
			.and_then(|v| Some(v.eq("/usr/bin/sccache")))
			.unwrap_or(false);
		if !sccache_enabled {
			error!("Sccache is not enabled!");
			panic!();
		}
		
		let name = cargo_toml_table
			.get("package").unwrap()
			.as_table().unwrap()
			.get("name").unwrap()
			.as_str().unwrap();
		
		let dylib_path = Self::dylib_path(path.as_ref(), name)?;
		trace!("Build files should be in {:?}", dylib_path);

		// Check if recompilation is needed
		let needs_recompilation = dylib_path.canonicalize()
			.and_then(|p| p.metadata().unwrap().modified())
			.and_then(|t| Ok(t < Self::build_files_last_modified(path.as_ref())))
			.unwrap_or(true);
		if needs_recompilation {
			trace!("Either build does not exist or is outdated, rebuilding");
			let status = std::process::Command::new("cargo")
				.arg("build")
				.current_dir(path.as_ref())
				.status()
				.with_context(|| "cargo build failed")?;
			if !status.success() {
				error!("Failed to compile extension");
				// use std::io::Write;
				// std::io::stdout().write_all(&output.stdout).unwrap();
				// std::io::stderr().write_all(&output.stderr).unwrap();
				// println!("{}", String::from_utf8(output.stdout).unwrap());
				// println!("{}", String::from_utf8(output.stderr).unwrap());
				panic!();
			}
		}

		assert!(std::fs::canonicalize(&dylib_path).is_ok(), "output path is bad");
		let library = unsafe { libloading::Library::new(&dylib_path)? };
		let library_ts = dylib_path.metadata().unwrap().modified().unwrap();

		// Fetch load dependencies 
		let load_dependencies = unsafe {
			let f = library.get::<unsafe extern fn() -> Vec<String>>(b"dependencies")?;
			f()
		};
		trace!("Depends on {:?}", load_dependencies);

		Ok(Self {
			name: name.to_string(), 
			path: path.as_ref().into(), 
			library, 
			read_at: library_ts, 
			load_dependencies,
			provides: None,
		})
	}

	/// Retuns an error if: 
	/// - the root Cargo.toml file cannot be read 
	/// - the root Cargo.toml file cannot be parsed 
	/// - the root Cargo.toml file does not contain an array of strings in workspace.members 
	fn dylib_path(extension_path: impl AsRef<Path>, name: &str) -> anyhow::Result<PathBuf> {
		let root_cargo_toml_table = std::fs::read_to_string("./Cargo.toml")
			.with_context(|| "failed to read cargo.toml")?
			.parse::<toml::Table>()
			.with_context(|| "failed to parse cargo.toml")?;
		let root_workspace = root_cargo_toml_table
			.get("workspace").unwrap()
			.as_table().unwrap()
			.get("members").unwrap()
			.as_array().unwrap()
			.iter().map(|v| v.as_str())
			.collect::<Option<Vec<_>>>().unwrap();
		
		// Output path differs if in workspace or not
		let ws_name = extension_path.as_ref().file_name().unwrap().to_str().unwrap();
		let dylib_output_dir = if root_workspace.contains(&&*format!("extensions/{}", ws_name)) {
			Path::new("./target/debug").into()
		} else {
			extension_path.as_ref().join("target/debug")
		};

		// File name varies by platform 
		#[cfg(target_os = "linux")]
		let dylib_path = dylib_output_dir.join(format!("lib{}", name)).with_extension("so");
		#[cfg(target_os = "macos")]
		let dylib_path = dylib_output_dir.join(format!("lib{}", name)).with_extension("dylib");
		#[cfg(target_os = "windows")]
		let dylib_path = dylib_output_dir.join(format!("{}", name)).with_extension("dll");

		Ok(dylib_path)
	}

	pub fn rebuild(&mut self, world: &mut World) -> anyhow::Result<()> {
		self.unload(world)?;
		*self = ExtensionEntry::new(&self.path)?;
		Ok(())
	}

	fn build_files_last_modified(path: impl AsRef<Path>) -> SystemTime {
		// We care about Cargo.toml and everthing in the src directiory
		let build_files = walkdir::WalkDir::new(path.as_ref().join("src"))
			.into_iter().filter_map(|e| e.ok())
			.map(|d| d.into_path())
			.chain(["Cargo.toml".into()]);
		let last_modified = build_files
			.map(|p| p.metadata().unwrap().modified().unwrap())
			.max().unwrap();
		last_modified
	}
	
	// Checks if this extension has been modified since it was last loaded
	pub fn dirty_level(&self) -> DirtyLevel {		
		if Self::build_files_last_modified(&self.path) > self.read_at {
			trace!("Build files modified, rebuild");
			return DirtyLevel::Rebuild;
		}

		// If newer build file exists on disk 
		// This path is old and not good and only works on linux
		let dylib_path = Self::dylib_path(&self.path, &self.name).unwrap();
		if let Ok(p) = dylib_path.canonicalize() {
			// If modified after we last loaded
			if p.metadata().unwrap().modified().unwrap().gt(&self.read_at) {
				return DirtyLevel::Reload;
			} else {
				return DirtyLevel::Clean;
			}
		} else {
			trace!("Build file does not exist, rebuild");
			return DirtyLevel::Rebuild;
		}
	}

	pub fn loaded(&self) -> bool {
		self.provides.is_some()
	}	

	pub fn load(&mut self, world: &mut World) -> anyhow::Result<()> {
		assert!(!self.loaded(), "Extension already loaded!");

		let mut loader = ExtensionLoader {
			world,
			provisions: ExtensionProvisions::default(),
		};

		unsafe {
			let f = self.library.get::<unsafe extern fn(&mut ExtensionLoader) -> ()>(b"load")?;
			f(&mut loader);
		}
		
		let _ = self.provides.insert(loader.provisions);
		
		Ok(())
	}

	// Extensions don't need much in their unlaod functions by default
	// Systems and components and resources will be removed automatically
	// In the future maybe we should be able to choose whether to serialize the data and try to reload it 
	// This would use serde so that if the data format changed the restoration can fail 
	pub fn unload(&mut self, world: &mut World) -> anyhow::Result<()> {
		assert!(self.loaded(), "Extension is not loaded!");

		unsafe {
			let f = self.library.get::<unsafe extern fn() -> ()>(b"unload")?;
			f()
		}

		let provisions = self.provides.take().unwrap();
		for component in provisions.components {
			info!("Remove component '{}'", component);
			world.unregister_component(component).expect("Component not found!");
		}
		for resource in provisions.resources {
			info!("Remove comonent '{}'", resource);
			world.remove_resource(resource).expect("Resource not found!");
		}

		Ok(())
	}
}
impl Drop for ExtensionEntry {
	fn drop(&mut self) {
		// Any references to the data in a library must be dropped before the library itself 
		self.provides = None;
	}
}


pub struct ExtensionRegistry {
	extensions: Vec<ExtensionEntry>,
	// Rebuilt when anything changes
	systems: HashMap<String, Vec<Vec<(usize, usize)>>>,
}
impl ExtensionRegistry {
	pub fn new() -> Self {
		Self {
			extensions: Vec::new(),
			// load_order: Vec::new(),
			systems: HashMap::new(),
		}
	}

	pub fn reload(&mut self, world: &mut World) -> anyhow::Result<()> {
		let mut reload_queue = Vec::new();
		let mut did_anything = false;

		trace!("Look for rebuilds");

		// Rebuild
		for i in 0..self.extensions.len() {
			match self.extensions[i].dirty_level() {
				DirtyLevel::Rebuild => {
					// Edit this one
					debug!("Rebuild extension {}", self.extensions[i].name);
					self.extensions[i].rebuild(world).unwrap();
					// Push dependents to reload queue
					for (j, e) in self.extensions.iter().enumerate() {
						if e.load_dependencies.contains(&self.extensions[i].name) {
							reload_queue.push(j);
						}
					}
					did_anything = true;
				},
				DirtyLevel::Reload => {
					reload_queue.push(i);
					did_anything = true;
				},
				DirtyLevel::Clean => {},
			}
		}

		trace!("Propagate reloads");

		// Reload propagation
		reload_queue.sort();
		reload_queue.dedup();
		while let Some(i) = reload_queue.pop() {
			debug!("Unload extension {}", self.extensions[i].name);
			
			self.extensions[i].unload(world)?;
			for (i, e) in self.extensions.iter().enumerate() {
				if e.loaded() && e.load_dependencies.contains(&self.extensions[i].name) {
					reload_queue.push(i);
				}
			}
		}

		// Load everything not loaded
		self.load(world)?;

		// Rebuild exectuion order if anything changed
		if did_anything {
			self.rebuild_systems()?;
		}
		
		Ok(())
	}

	// Loads extensions in order 
	fn load(&mut self, world: &mut World) -> anyhow::Result<()> {
		trace!("Begin load");

		// Find unload extension with met dependencies
		while let Some((i, e)) = self.extensions.iter().enumerate().find(|(_, e)| {
			!e.loaded() && e.load_dependencies.iter()
				.map(|dep| self.extensions.iter().find(|e| e.name.eq(dep) && e.loaded()).is_some())
				.fold(true, |a, v| a && v)
		}) {
			debug!("Load extension '{}'", e.name);
			self.extensions[i].load(world)?;
		}

		trace!("Check load");

		// Assert that all are loaded 
		let unloaded = self.extensions.iter()
			// Map to list of unmet dependencies
			.map(|e| (e, e.load_dependencies.iter().filter(|d| self.extensions.iter().find(|e| e.name.eq(*d)).is_none()).collect::<Vec<_>>()))
			.filter(|(_, d)| !d.is_empty())
			.collect::<Vec<_>>();
		if !unloaded.is_empty() {
			for (e, unmet) in unloaded {
				error!("Extension '{}' is missing dependencies {:?}", e.name, unmet);
			}
			return Err(anyhow::anyhow!("Failed to resolve dependencies"));
		}

		trace!("Everything seems to have loaded");

		Ok(())
	}

	pub fn register(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
		let e = ExtensionEntry::new(path)?;
		self.extensions.push(e);

		Ok(())
	}
 
	pub fn remove(&mut self, path: impl AsRef<Path>, world: &mut World) -> anyhow::Result<()> {
		if let Some(i) = self.extensions.iter().position(|e| e.path.eq(path.as_ref())) {
			let mut e = self.extensions.remove(i);
			e.unload(world)?;
		} else {
			return Err(anyhow!("Extension not found"));
		}
		self.reload(world)?;

		Ok(())
	}

	pub fn run(&self, world: &mut World, group: impl AsRef<str>) {
		let stages = self.systems.get(&group.as_ref().to_string()).unwrap();

		for stage in stages {
			for &(ei, si) in stage {
				let e = &self.extensions[ei];
				let s = &e.provides.as_ref().unwrap().systems[si];
				debug!("Extension '{}' system '{}'", e.name, s.id);
				let w = world as *const World;
				(s.pointer)(w);
			}
		} 

	}

	fn rebuild_systems(&mut self) -> anyhow::Result<()> {		
		let groups = self.extensions.iter()
			.enumerate()
			.filter_map(|(i, e)| e.provides.as_ref().and_then(|p| Some((i, p))))
			.flat_map(|(i, p)| p.systems.iter().enumerate().map(move |(j, s)| (i, j, s)))
			.fold(HashMap::new(), |mut a, v| {
				a.entry(&v.2.group).or_insert(Vec::new()).push(v);
				a
			});

		let mut orders = HashMap::new();
		for (group, mut queue) in groups {
			debug!("Group '{}'", group);
			let mut stages = vec![vec![]];

			// Satisfied if in any of the PREVIOUS stages (but NOT the current stage) 
			fn satisfied(stages: &Vec<Vec<(usize, usize, &ExtensionSystem)>>, id: &String) -> bool {
				(&stages[0..stages.len()-1]).iter()
				.any(|stage| stage.iter()
					.map(|v| v.2)
					.map(|s: &ExtensionSystem| &s.id)
					.fold(false, |a, v| a || v.eq(id)))
			}

			while !queue.is_empty() {
				let next = queue.iter().position(|s| 
					s.2.run_after.iter().all(|id| satisfied(&stages, id)));
				
				if let Some(i) = next {
					let (i, j, s) = queue.remove(i);
					debug!("Run '{}'", s.id);
					stages.last_mut().unwrap().push((i, j, s));
				} else {
					if stages.last().and_then(|s| Some(s.is_empty())).unwrap_or(false) {
						error!("Failing to meet some dependency!");
						panic!();
					}
					debug!("New stage");
					stages.push(Vec::new());
				}
			}

			let stages = stages.iter().map(|stage| 
				stage.iter().map(|&(i, j, _)| (i, j)).collect::<Vec<_>>()
			).collect::<Vec<_>>();
			orders.insert(group.clone(), stages);
		}

		self.systems = orders;

		Ok(())
	}
}
// impl Drop for ExtensionRegistry {
// 	fn drop(&mut self) {
// 		// Any references to the data in a library must be dropped before the library itself 
// 		self.systems.clear();
// 	}
// }
