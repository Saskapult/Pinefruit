use std::{collections::HashMap, path::{Path, PathBuf}, time::SystemTime};
use anyhow::{anyhow, Context};
use eks::{prelude::*, system::SystemFunction, WorldEntitySpawn};
pub use eks;
pub mod prelude {
	pub use eks::prelude::*;
	pub use crate::{ExtensionRegistry, ExtensionSystemsLoader};
}

#[macro_use]
extern crate log;


/// Used by load functions to register and describe storages. 
pub struct ExtensionStorageLoader<'a> {
	world: &'a mut World, 
	storages: ExtensionStorages,
}
impl<'a> ExtensionStorageLoader<'a> {
	pub fn component<C: Component>(&mut self) -> &mut Self {
		self.world.register_component::<C>();
		self.storages.components.push(C::STORAGE_ID.to_string());
		self
	}

	pub fn resource<R: Resource>(&mut self, r: R) -> &mut Self {
		self.world.insert_resource(r);
		self.storages.resources.push(R::STORAGE_ID.to_string());
		self
	}

	pub fn spawn(&mut self) -> WorldEntitySpawn<'_> {
		self.world.spawn()
	}

	// Should have functions to access world
	// Some resources might need info from other resources 
	// But that's outside of our current scope 
}


/// Passed to the systems function to collect system data. 
pub struct ExtensionSystemsLoader<'a> {
	// The IDs of all loaded extensions
	// Used to conditionally enable systems
	// Although now that I think aobut it, this would require us to have a loads_after condition *if* some other extension is present
	// I'll leave this here and future me can deal with implementing that 
	// extensions: Vec<String>, 
	// All systems provided by this extension
	// In the future we can pass the entire set of extensions so that overwrites can happen
	// Oh but wait, that's a bad idea! 
	// We'd need to track what was added for each world so that it can be unloaded for each world
	systems: &'a mut Vec<ExtensionSystem>,
}
impl<'a> ExtensionSystemsLoader<'a> {
	pub fn system<S: SystemFunction<'static, (), Q, R> + Copy + 'static, R, Q: Queriable<'static>>(
		&mut self, 
		group: impl AsRef<str>,
		name: impl AsRef<str>, 
		function: S,
	) -> &mut ExtensionSystem {
		let i = self.systems.len();
		self.systems.push(ExtensionSystem::new::<S, R, Q>(group, name, function));
		self.systems.get_mut(i).unwrap()
	}
}


pub struct ExtensionSystem {
	group: String,
	id: String,
	pointer: Box<dyn Fn(*const World)>,
	run_after: Vec<String>,
	run_before: Vec<String>, 
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
			run_before: Vec::new(),
		}
	}

	pub fn run_after(&mut self, id: impl AsRef<str>) -> &mut Self {
		self.run_after.push(id.as_ref().to_string());
		self
	}

	pub fn run_before(&mut self, id: impl AsRef<str>) -> &mut Self {
		self.run_before.push(id.as_ref().to_string());
		self
	}
}
impl std::fmt::Debug for ExtensionSystem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ExtensionSystem")
			.field("group", &self.group)
			.field("id", &self.id)
			.field("run_after", &self.run_after)
			// .field("run_before", &self.run_before)
			.finish()			
	}
}
unsafe impl Send for ExtensionSystem {}
unsafe impl Sync for ExtensionSystem {}


#[derive(Debug, PartialEq, Eq)]
pub enum DirtyLevel {
	Clean,
	Reload, // Load .so file again
	Rebuild, // Rebuild whole project
}


#[derive(Debug, Default)]
pub struct ExtensionStorages {
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
	pub systems: Vec<ExtensionSystem>,
	pub provides: Option<ExtensionStorages>,
}
impl ExtensionEntry {
	// Reads extension from disk and compiles 
	pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		trace!("Loading {:?}", path.as_ref());

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
		
		let dylib_path = Self::stored_extension_path(name);
		trace!("Build files should be in {:?}", dylib_path);

		// Check if recompilation is needed
		// stored files exist and are not older than build files
		let needs_recompilation = dylib_path.canonicalize()
			.and_then(|p| p.metadata().unwrap().modified())
			.and_then(|t| Ok(t < Self::build_files_last_modified(path.as_ref())))
			.unwrap_or(true);
		if needs_recompilation {
			trace!("Either build does not exist or is outdated, rebuilding");

			// Look for old build files
			// If swap path is soem then unsawp after building
			let out_path = Self::build_output_path(path.as_ref(), &name)?;
			let swap_path = if out_path.exists() {
				trace!("Previous build files exist, copying to library swap");

				let storage = Self::stored_library_path(&name);
				std::fs::copy(&out_path, &storage).unwrap();

				Some(storage)
			} else {
				trace!("No previous build detected");
				None
			};

			let status = std::process::Command::new("cargo")
				.arg("build")
				.arg("-F")
				.arg("extension")
				.current_dir(path.as_ref())
				.status()
				.with_context(|| "cargo build failed")?;
			if !status.success() {
				error!("Failed to compile extension");
				panic!();
			}

			trace!("Copying output to extension storage");
			let new_files = Self::build_output_path(path.as_ref(), &name)?;
			let storage = Self::stored_extension_path(&name);
			trace!("{:?} -> {:?}", new_files, storage);
			std::fs::create_dir_all(storage.parent().unwrap()).unwrap();
			std::fs::copy(new_files, storage).unwrap();

			if let Some(storage) = swap_path {
				trace!("Restoring previous build files from library swap");
				std::fs::copy(&storage, &out_path).unwrap();
				std::fs::remove_file(&storage).unwrap();
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

		// Fetch systems
		let mut systems = Vec::new();
		let mut systems_loader = ExtensionSystemsLoader {
			systems: &mut systems,
		};
		unsafe {
			let f = library.get::<unsafe extern fn(&mut ExtensionSystemsLoader)>(b"systems")?;
			f(&mut systems_loader);
		}
		trace!("Provides {} systems", systems.len());

		Ok(Self {
			name: name.to_string(), 
			path: path.as_ref().into(), 
			library, 
			read_at: library_ts, 
			load_dependencies,
			systems,
			provides: None,
		})
	}

	/// After being built, extension files are copied here for storage. 
	/// This is becuase building one extension that depends on other extensions will rebuild its dependents becuase of the change in `no_export` flag. 
	fn stored_extension_path(extension_name: impl AsRef<str>) -> PathBuf {
		 PathBuf::from("target/extensions")
			.join(Self::stored_name(extension_name))
	}

	fn stored_library_path(extension_name: impl AsRef<str>) -> PathBuf {
		PathBuf::from("target/tmp")
		   .join(Self::stored_name(extension_name))
	}

	fn stored_name(extension_name: impl AsRef<str>) -> PathBuf {
		// File name varies by platform 
		#[cfg(target_os = "linux")]
		let dylib_path = PathBuf::from(format!("lib{}", extension_name.as_ref())).with_extension("so");
		#[cfg(target_os = "macos")]
		let dylib_path = PathBuf::from(format!("lib{}", extension_name.as_ref())).with_extension("dylib");
		#[cfg(target_os = "windows")]
		let dylib_path = PathBuf::from(format!("{}", extension_name.as_ref())).with_extension("dll");

		dylib_path
	}

	/// Where an extension will be after running `cargo build`. 
	///
	/// Retuns an error if: 
	/// - the root Cargo.toml file cannot be read 
	/// - the root Cargo.toml file cannot be parsed 
	/// - the root Cargo.toml file does not contain an array of strings in workspace.members 
	fn build_output_path(extension_path: impl AsRef<Path>, name: &str) -> anyhow::Result<PathBuf> {
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
		let dylib_output_dir = if root_workspace.contains(&"extensions/*") || root_workspace.contains(&&*format!("extensions/{}", ws_name)) {
			Path::new("./target/debug").into()
		} else {
			extension_path.as_ref().join("target/debug")
		};

		let dylib_path = dylib_output_dir.join(Self::stored_name(name));

		Ok(dylib_path)
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
		let dylib_path = Self::stored_extension_path(&self.name);
		if let Ok(p) = dylib_path.canonicalize() {
			// If modified after we last loaded
			let mod_time = p.metadata().unwrap().modified().unwrap();
			if mod_time > self.read_at {
				let d = mod_time.duration_since(self.read_at).unwrap();
				trace!("Build files are new, reload ({:?}, {:.2}s)", p, d.as_secs_f32());
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

		let mut loader = ExtensionStorageLoader {
			world,
			storages: ExtensionStorages::default(),
		};

		unsafe {
			let f = self.library.get::<unsafe extern fn(&mut ExtensionStorageLoader) -> ()>(b"load")?;
			f(&mut loader);
		}
		
		let _ = self.provides.insert(loader.storages);
		
		Ok(())
	}

	// Extensions don't need much in their unlaod functions by default
	// Systems and components and resources will be removed automatically
	// In the future maybe we should be able to choose whether to serialize the data and try to reload it 
	// This would use serde so that if the data format changed the restoration can fail 
	pub fn unload(&mut self, world: &mut World) -> anyhow::Result<()> {
		assert!(self.loaded(), "Extension is not loaded!");

		// unsafe {
		// 	let f = self.library.get::<unsafe extern fn() -> ()>(b"unload")?;
		// 	f()
		// }

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
		if self.provides.is_some() {
			warn!("Dropping extension '{}' with active provisions", self.name);
		}
		// Any references to the data in a library must be dropped before the library itself 
		self.systems.clear();
	}
}


#[derive(Default)]
pub struct NativeSystems {
	systems: Vec<ExtensionSystem>,
}
impl NativeSystems {
	pub fn system<S: SystemFunction<'static, (), Q, R> + Copy + 'static, R, Q: Queriable<'static>>(
		&mut self, 
		group: impl AsRef<str>,
		name: impl AsRef<str>, 
		function: S,
	) -> &mut ExtensionSystem {
		let i = self.systems.len();
		self.systems.push(ExtensionSystem::new::<S, R, Q>(group, name, function));
		self.systems.get_mut(i).unwrap()
	}
}


#[derive(Debug, Clone, Copy)]
enum SystemIndex {
	External((usize, usize)),
	Native(usize),
}
impl From<(usize, usize)> for SystemIndex {
	fn from(value: (usize, usize)) -> Self {
		Self::External(value)
	}
}


pub struct ExtensionRegistry {
	extensions: Vec<ExtensionEntry>,

	native_systems: NativeSystems,

	// Rebuilt when anything changes
	systems: HashMap<String, Vec<Vec<SystemIndex>>>,
}
impl ExtensionRegistry {
	pub fn new() -> Self {
		Self {
			extensions: Vec::new(),
			native_systems: NativeSystems::default(),
			systems: HashMap::new(),
		}
	}

	pub fn native_systems(&mut self) -> &mut NativeSystems {
		&mut self.native_systems
	}

	pub fn reload(&mut self, world: &mut World) -> anyhow::Result<()> {
		let mut reload_queue = Vec::new();

		trace!("Look for rebuilds");

		// Rebuild
		for i in 0..self.extensions.len() {
			match self.extensions[i].dirty_level() {
				DirtyLevel::Rebuild => {
					debug!("Rebuild extension {}", self.extensions[i].name);

					// We can't overwite the library file while it's laoded
					// Otherwise it will segfault 
					// We must do some fanangling here
					let mut ext = self.extensions.remove(i);
					ext.unload(world)?;
					let path = ext.path.clone();
					drop(ext);
					self.extensions.insert(i, ExtensionEntry::new(path)?);

					// Push dependents to reload queue
					for (j, e) in self.extensions.iter().enumerate() {
						if e.load_dependencies.contains(&self.extensions[i].name) {
							reload_queue.push(j);
						}
					}
				},
				DirtyLevel::Reload => {
					debug!("Reload extension {}", self.extensions[i].name);
					reload_queue.push(i);
				},
				DirtyLevel::Clean => {},
			}
		}

		trace!("Propagate reloads");

		// Reload propagation
		reload_queue.sort();
		reload_queue.dedup();
		trace!("Need to unload extensions {:?}", reload_queue.iter().map(|&i| &self.extensions[i].name).collect::<Vec<_>>());
		while let Some(i) = reload_queue.pop() {
			debug!("Unload extension {}", self.extensions[i].name);
			
			self.extensions[i].unload(world)?;
			for (i, e) in self.extensions.iter().enumerate() {
				if e.loaded() && e.load_dependencies.contains(&self.extensions[i].name) {
					reload_queue.push(i);
				}
			}
		}

		let did_anything = self.extensions.iter().any(|e| !e.loaded());

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

	/// Registers all folders in a path as extensions. 
	/// Should registration fail, the path is skipped. 
	pub fn register_all_in(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
		let dirs = std::fs::read_dir(path)?
			.filter_map(|d| d.ok())
			.map(|d| d.path())
			.filter(|d| d.is_dir());
		for d in dirs {
			if let Err(e) = self.register(&d) {
				error!("Failed to register {:?} - {:?}", d, e);
			}
		}

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

	pub fn run(&self, world: &mut World, group: impl AsRef<str>) -> anyhow::Result<()> {
		info!("Running '{}'", group.as_ref());
		let stages = self.systems.get(&group.as_ref().to_string())
			.with_context(|| "Failed to locate workload")?;

		for stage in stages {
			for &i in stage {
				match i {
					SystemIndex::External((ei, si)) => {
						let e = &self.extensions[ei];
						let s = &e.systems[si];
						debug!("Extension '{}' system '{}'", e.name, s.id);
						let w = world as *const World;
						(s.pointer)(w);
					},
					SystemIndex::Native(i) => {
						let s = &self.native_systems.systems[i];
						debug!("Native system '{}'", s.id);
						let w = world as *const World;
						(s.pointer)(w);
					}
				}
			}
		} 

		Ok(())
	}

	fn rebuild_systems(&mut self) -> anyhow::Result<()> {
		trace!("Collect into groups");
		let mut groups = HashMap::new();
		for (i, e) in self.extensions.iter().enumerate() {
			if !e.loaded() {
				warn!("Skipping systems for unloaded extension '{}'", e.name);
				continue
			}
			for (j, s) in e.systems.iter().enumerate() {
				let group = groups.entry(&s.group).or_insert(Vec::new());
				group.push(((i, j).into(), s));
			}
		}
		for (i, s) in self.native_systems.systems.iter().enumerate() {
			let group = groups.entry(&s.group).or_insert(Vec::new());
			group.push((SystemIndex::Native(i), s));
		}
		debug!("{} groups", groups.len());

		trace!("Collect queue dependencies");
		let groups = groups.into_iter().map(|(name, queue)| {
			let q2 = queue.clone();
			(name, queue.into_iter().enumerate().map(|(i, (index, s))| {
			let mut depends_on = s.run_after.clone();
			// If another system wants to be run before this one, then this system depends on it
			for (j, (_, o)) in q2.iter().enumerate() {
				if i == j { continue }
				if o.run_before.contains(&s.id) {
					trace!("'{}' depends on '{}'", o.id, s.id);
					depends_on.push(o.id.clone());
				}
			}
			// map to ((i, j), name, depends_on)
			(index, &s.id, depends_on)
		}).collect::<Vec<_>>())}).collect::<HashMap<_, _>>();

		trace!("Create group orders");
		let mut orders = HashMap::new();
		for (group, mut queue) in groups {
			debug!("Group '{}'", group);
			let mut stages = vec![vec![]];

			// Satisfied if in any of the PREVIOUS stages (but NOT the current stage) 
			fn satisfied(stages: &Vec<Vec<(SystemIndex, &String)>>, id: &String) -> bool {
				(&stages[0..stages.len()-1]).iter()
				.any(|stage| stage.iter()
					.map(|v| v.1)
					.fold(false, |a, v| a || v.eq(id)))
			}

			while !queue.is_empty() {
				let next = queue.iter().position(|(_, _, d)| 
					d.iter().all(|id| satisfied(&stages, id)));
				
				if let Some(i) = next {
					let (p, name, _) = queue.remove(i);
					debug!("Run '{}'", name);
					stages.last_mut().unwrap().push((p, name));
				} else {
					if stages.last().and_then(|s| Some(s.is_empty())).unwrap_or(false) {
						error!("Failing to meet some dependency!");
						error!("Stages are:");
						for (i, stage) in stages.into_iter().enumerate() {
							error!("{}", i);
							for (_, name) in stage {
								error!("\t'{}'", name);
							}
						}
						error!("Remaining items are:");
						for (_, n, d) in queue {
							error!("'{}' - {:?}", n, d);
						}
						panic!();
					}
					debug!("New stage");
					stages.push(Vec::new());
				}
			}

			let stages = stages.iter().map(|stage| 
				stage.iter().map(|&(p, _)| p).collect::<Vec<_>>()
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
