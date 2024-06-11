use std::{collections::HashMap, path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};
use anyhow::{anyhow, Context};
use eks::{prelude::*, resource::UntypedResource, sparseset::UntypedSparseSet, system::SystemFunction, WorldEntitySpawn};
pub use eks;
pub mod prelude {
	pub use eks::prelude::*;
	pub use crate::{ExtensionRegistry, ExtensionSystemsLoader};
	pub use profiling;
	pub use ekstensions_derive::*;
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
		// TODO: can't we just get a pointer to S::run_system?
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
			.field("run_before", &self.run_before)
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
	Unloaded, // It's not loaded so we didn't test to see if it's dirty 
}


#[derive(Debug, Default)]
pub struct ExtensionStorages {
	pub components: Vec<String>,
	pub resources: Vec<String>,
}


pub struct ExtensionLibrary {
	pub library: libloading::Library,
	pub read_at: SystemTime, 
	pub load_dependencies: Vec<String>,
	pub systems: Vec<ExtensionSystem>,
	pub storages: Option<ExtensionStorages>,
}
impl ExtensionLibrary {
	// Name is needed becuase symbols for extension functions are unique (based on name)
	pub fn new(name: impl AsRef<str>, path: impl AsRef<Path>) -> anyhow::Result<Self> {
		let name = name.as_ref();
		let path = path.as_ref();

		trace!("Loading extension library '{}' from {:?}", name, path);
		let library = unsafe { libloading::Library::new(path)? };
		let library_ts = path.metadata().unwrap().modified().unwrap();
		trace!("Read success");

		// Fetch load dependencies 
		let load_dependencies = unsafe {
			let n = format!("{}_info", name);
			trace!("Fetch {:?}", n);
			let f = library.get::<unsafe extern fn() -> Vec<String>>(n.as_bytes())?;
			trace!("Call {:?}", n);
			f()
		};
		trace!("Depends on {:?}", load_dependencies);

		// Fetch systems
		let mut systems = Vec::new();
		let mut systems_loader = ExtensionSystemsLoader {
			systems: &mut systems,
		};
		unsafe {
			let n = format!("{}_systems", name);
			let f = library.get::<unsafe extern fn(&mut ExtensionSystemsLoader)>(n.as_bytes())?;
			f(&mut systems_loader);
		}
		trace!("Provides {} systems", systems.len());

		Ok(Self {
			library, 
			read_at: library_ts, 
			load_dependencies,
			systems,
			storages: None,
		})
	}

	pub fn load(&mut self, name: impl AsRef<str>, world: &mut World) -> anyhow::Result<()>  {
		trace!("Loading extension '{}' into world", name.as_ref());

		let mut loader = ExtensionStorageLoader {
			world, storages: ExtensionStorages::default(), 
		};

		unsafe {
			let n = format!("{}_load", name.as_ref());
			let f = self.library.get::<unsafe extern fn(&mut ExtensionStorageLoader)>(n.as_bytes())?;
			f(&mut loader);
		}

		self.storages = Some(loader.storages);

		Ok(())
	}

	// Extensions don't need much in their unload functions by default
	// Systems and components and resources will be removed automatically
	// In the future maybe we should be able to choose whether to serialize the data and try to reload it 
	// This would use serde so that if the data format changed the restoration can fail 
	pub fn unload(
		&mut self, world: &mut World
	) -> anyhow::Result<(
		Vec<(String, UntypedSparseSet)>,
		Vec<(String, UntypedResource)>,
	)> {
		trace!("Unloading extension 'TODO: NAME' from world");

		let provisions = self.storages.take()
			.expect("Extension was not loaded!");
		let components = provisions.components.into_iter().map(|component| {
			info!("Remove component '{}'", component);
			let s = world.unregister_component(&component).expect("Component not found!");
			(component, s)
		}).collect();

		let resources = provisions.resources.into_iter().map(|resource| {
			info!("Remove comonent '{}'", resource);
			let s = world.remove_resource(&resource).expect("Resource not found!");
			(resource, s)
		}).collect();

		Ok((components, resources))
	}
}
impl Drop for ExtensionLibrary {
	fn drop(&mut self) {
		// Any references to the data in a library must be dropped before the library itself 
		self.systems.clear();
	}
}


pub enum ExtensionPath {
	// Library(PathBuf), // Future stuff, watch a file and reload if changed
	Crate(PathBuf, bool), // Path, in workspace 
}
impl ExtensionPath {
	// Checks if this extension has been modified since it was last loaded
	

	
}


fn extension_build_filename(extension_name: impl AsRef<str>) -> PathBuf {
	// File name varies by platform 
	#[cfg(target_os = "linux")]
	let dylib_path = PathBuf::from(format!("lib{}", extension_name.as_ref())).with_extension("so");
	#[cfg(target_os = "macos")]
	let dylib_path = PathBuf::from(format!("lib{}", extension_name.as_ref())).with_extension("dylib");
	#[cfg(target_os = "windows")]
	let dylib_path = PathBuf::from(format!("{}", extension_name.as_ref())).with_extension("dll");

	dylib_path
}

fn src_files_last_modified(path: impl AsRef<Path>) -> SystemTime {
	// We care about Cargo.toml and everthing in the src directiory
	let src_files = walkdir::WalkDir::new(path.as_ref().join("src"))
		.into_iter().filter_map(|e| e.ok())
		.map(|d| d.into_path())
		.chain(["Cargo.toml".into()]);
	let last_modified = src_files
		.map(|p| p.metadata().unwrap().modified().unwrap())
		.max().unwrap();
	last_modified
}


pub struct ExtensionEntry {
	// Extracted from Cargo.toml or file name
	pub name: String,
	pub file_path: PathBuf, // The source file for this extension 
	pub crate_path: Option<PathBuf>, // The crate used to build this extension file 
	pub library: Option<ExtensionLibrary>,
}
impl ExtensionEntry {
	// Reads extension from disk and compiles 
	pub fn new_crate(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		trace!("Loading extension (crate) {:?}", path.as_ref());

		let cargo_toml_path = path.as_ref().join("Cargo.toml");
		let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)
			.with_context(|| "failed to read cargo.toml")?;
		let cargo_toml_table: toml::map::Map<String, toml::Value> = cargo_toml_content.parse::<toml::Table>()
			.with_context(|| "failed to parse cargo.toml")?;

		// Require dylib + rlib 
		let is_dylib = cargo_toml_table.get("lib")
			.and_then(|v| v.as_table())
			.and_then(|t| t.get("crate-type"))
			.and_then(|v| v.as_array())
			.map(|v| 
				v.contains(&toml::Value::String("dylib".to_string()))
				&&
				v.contains(&toml::Value::String("rlib".to_string()))
			).unwrap_or(false);
		if !is_dylib {
			error!("Not rlib dylib!");
			// panic!();
		}
		
		let name = cargo_toml_table
			.get("package").unwrap()
			.as_table().unwrap()
			.get("name").unwrap()
			.as_str().unwrap();

		let root_cargo_toml = std::fs::read_to_string("./Cargo.toml")
			.with_context(|| "failed to read root Cargo.toml")?
			.parse::<toml::Table>()
			.with_context(|| "failed to parse root Cargo.toml")?;
		let root_workspace_members = root_cargo_toml
			.get("workspace").and_then(|v| v.as_table())
			.expect("root Cargo.toml has no workspace")
			.get("members").and_then(|v| v.as_array())
			.expect("root Cargo.toml workspace has no members")
			.iter().map(|v| v.as_str())
			.collect::<Option<Vec<_>>>()
			.expect("failed to read root Cargo.toml workspace members");
		
		// Output path differs if in workspace or not
		let in_workspace = root_workspace_members.contains(&"extensions/*") 
			|| root_workspace_members.contains(&&*format!("extensions/{}", name));

		let file_path = if in_workspace {
			PathBuf::from("target/debug")
		} else {
			path.as_ref().join("target/debug")
		}.join(extension_build_filename(name));

		Ok(Self {
			name: name.to_string(),
			file_path,
			crate_path: Some(path.as_ref().to_path_buf()),
			library: None,
		})
	}

	/// Loads an extension libray into memory. 
	/// If loading from a crate, this could rebuild the crate. 
	pub fn activate(&mut self) -> anyhow::Result<()> {
		assert!(!self.active(), "Cannot activate an active extension!");

		// Try to find an existing extension file
		// There should only be one file in the extension folder 
		let ext_folder = Path::new("target/extensions").join(&self.name);
		std::fs::create_dir_all(&ext_folder).unwrap();
		let ext_previous = std::fs::read_dir(&ext_folder).ok().and_then(|rd| rd
			.filter_map(|f| f.ok())
			.map(|f| f.path())
			.find(|f| f.extension().map(|e| e == "so").unwrap_or(false)));
		if let Some(p) = ext_previous.as_ref() {
			trace!("Previous extension file {:?}", p);
		}

		// Get timestamps
		let stored_ts = ext_previous.as_ref()
			.map(|v: &PathBuf| v.file_stem().unwrap().to_str().unwrap().parse::<u64>().unwrap())
			.map(|v| UNIX_EPOCH.checked_add(Duration::from_nanos(v)).unwrap());
		let build_ts = self.file_path.canonicalize().ok().map(|p| p.metadata().unwrap().modified().unwrap());
		let src_ts = self.crate_path.as_ref().map(|p| src_files_last_modified(p));

		// Find level of dirty 
		let dirty_level = if let Some(stored_ts) = stored_ts {
			if src_ts.map(|src_ts| src_ts > stored_ts).unwrap_or(false) {
				// Stored exists and less updated than src
				DirtyLevel::Rebuild
			} else if build_ts.map(|build_ts| build_ts > stored_ts).unwrap_or(false) {
				// Stored exists and less updated than build
				DirtyLevel::Reload
			} else {
				// Stored exists and most updated
				DirtyLevel::Clean
			}
		} else {
			if let Some(build_ts) = build_ts {
				if let Some(src_ts) = src_ts {
					if src_ts > build_ts {
						// Stored DNE and src most updated
						DirtyLevel::Rebuild
					} else {
						// Stored DNE and build most updated
						DirtyLevel::Reload
					}
				} else {
					// Stored DNE and src not exist
					DirtyLevel::Reload
				}
			} else {
				// Stored DNE and build DNE
				DirtyLevel::Rebuild
			}
		};

		if dirty_level == DirtyLevel::Rebuild {
			trace!("Rebuilding extension from crate");
			assert!(self.crate_path.is_some());

			// This assumes the crate is part of our workspace! 
			let status = std::process::Command::new("cargo")
				.arg("build")
				// .env("RUSTC_WRAPPER", "/usr/bin/sccache")
				.arg("-p")
				.arg(&self.name)
				.status()
				.with_context(|| "cargo build failed")?;
			if !status.success() {
				error!("Failed to compile extension");
				panic!();
			}
		}


		let epoch_dur = self.file_path.metadata().unwrap().modified().unwrap().duration_since(UNIX_EPOCH).unwrap();
		let ext_file = ext_folder.join(format!("{}.so", epoch_dur.as_nanos()));
		if dirty_level == DirtyLevel::Reload || dirty_level == DirtyLevel::Rebuild {
			trace!("Copying new extension build file to storage");
			trace!("{:?} -> {:?}", self.file_path, ext_file);
			std::fs::create_dir_all(ext_file.parent().unwrap()).unwrap();
			std::fs::copy(&self.file_path, &ext_file).unwrap();

			if let Some(pp) = ext_previous.as_ref() {
				trace!("Deleting old extension file {:?}", pp);
				std::fs::remove_file(&pp).unwrap();
			}
		}

		self.library = Some(ExtensionLibrary::new(&self.name, ext_file)?);
		Ok(())
	}

	pub fn active(&self) -> bool {
		self.library.is_some()
	}

	pub fn dirty_level(&self) -> DirtyLevel {
		if self.library.is_none() {
			return DirtyLevel::Unloaded;
		}
		let last_read = self.library.as_ref().unwrap().read_at;

		if let Some(path) = self.crate_path.as_ref() {
			// Look at source files
			let last_mod = src_files_last_modified(path);
			if last_mod > last_read {
				trace!("Source files modified, rebuild");
				return DirtyLevel::Rebuild;
			}
		}

		if let Ok(p) = self.file_path.canonicalize() {
			let mod_time = p.metadata().unwrap().modified().unwrap();
			if mod_time > last_read {
				let d = mod_time.duration_since(last_read).unwrap();
				trace!("Extension file is newer, reload ({:?}, {:.2}s)", p, d.as_secs_f32());
				return DirtyLevel::Reload;
			} else {
				return DirtyLevel::Clean;
			}
		} else {
			trace!("Extension file does not exist, rebuild");
			assert!(self.crate_path.is_some(), "No crate path, cannot rebuild!");
			return DirtyLevel::Rebuild;
		}
	}
}


/// A status update for extension loading. 
pub struct LoadStatus {
	pub to_load: Vec<(String, bool)>,
	pub loaded: Vec<String>,
}


pub struct ExtensionRegistry {
	// Extension entries build themselves upon being created
	// This is bad 
	// It should only know its path, and then build later if applicable (in the reload function)
	// Because we can't rely on cargo.toml, extension name should only be known after the extension is loaded
	// Probably with an external function implemented by a macro 
	// Like setting profiling on or off 
	// registration_queue: Vec<PathBuf>,

	extensions: Vec<ExtensionEntry>,

	// Rebuilt when anything changes
	systems: HashMap<String, (Vec<((usize, usize), Vec<usize>)>, Vec<Vec<usize>>)>,
}
impl ExtensionRegistry {
	pub fn new() -> Self {
		Self {
			extensions: Vec::new(),
			systems: HashMap::new(),
		}
	}

	pub fn reload(&mut self, world: &mut World, updates: impl Fn(LoadStatus)) -> anyhow::Result<()> {
		// Bool is for soft/hard reload 
		// A soft reload occurs if the extension is clean but needs to be loaded again due to depending on something else
		let mut load_queue = HashMap::new();

		// Find dirty/unloaded extensions 
		trace!("Look for rebuilds");
		for i in 0..self.extensions.len() {
			// if !self.extensions[i].active() {
			// 	load_queue.entry(i).or_insert(false);
			// 	continue
			// }

			match self.extensions[i].dirty_level() {
				DirtyLevel::Unloaded => {
					debug!("Load extension {}", self.extensions[i].name);
					load_queue.insert(i, true);
					// Push dependents to reload queue
					// for (j, e) in self.extensions.iter().enumerate() {
					// 	if e.load_dependencies.contains(&self.extensions[i].name) {
					// 		load_queue.entry(j).or_insert(false);
					// 	}
					// }
				}
				DirtyLevel::Rebuild => {
					debug!("Rebuild extension {}", self.extensions[i].name);
					load_queue.insert(i, true);
					// Push dependents to reload queue
					// for (j, e) in self.extensions.iter().enumerate() {
					// 	if e.load_dependencies.contains(&self.extensions[i].name) {
					// 		load_queue.entry(j).or_insert(false);
					// 	}
					// }
				},
				DirtyLevel::Reload => {
					debug!("Reload extension {}", self.extensions[i].name);
					load_queue.entry(i).or_insert(false);
				},
				DirtyLevel::Clean => {
					debug!("Extension {} is clean", self.extensions[i].name);
				},
			}
		}

		// Send update
		updates(LoadStatus {
			to_load: load_queue.iter()
				.map(|(&i, &h)| (self.extensions[i].name.clone(), h))
				.collect::<Vec<_>>(),
			loaded: (0..self.extensions.len())
				.filter(|i| load_queue.get(i).is_none())
				.map(|i| self.extensions[i].name.clone())
				.collect::<Vec<_>>(),
		});
		error!("Update: {} in queue", load_queue.len());

		// TODO: Dependency load order
		for (&i, &hard) in load_queue.clone().iter() {
			if hard {
				let ext = self.extensions.get_mut(i).unwrap();				
				debug!("Hard reload of {}", ext.name);

				let mut lib = ext.library.take();
				trace!("Removing storages...");
				let previous_storages = lib.as_mut().map(|lib| lib.unload(world))
					.map(|r| r.map(|(c, r)| (
						c.into_iter()
							.map(|(n, c)| (n, unsafe { c.into_raw() }))
							.collect::<Vec<_>>(),
						r.into_iter()
							.map(|(n, r)| (n, unsafe { r.into_raw() }))
							.collect::<Vec<_>>(),
					))).transpose()?;

				// TODO: if serializable, use serialization
				// Needs untypedsparseset to finish serialization feature 

				trace!("Dropping old extension entry...");
				drop(lib);
				ext.activate()?;

				// Load with new code
				trace!("Loading into world...");
				ext.library.as_mut().unwrap().load(&ext.name, world)?;

				if let Some((components, resources)) = previous_storages {
					// Replace storages
					trace!("Overwriting to restore previous storages...");
					for (id, uss) in components {
						warn!("Replacing component storage '{}' with raw persisted data", id);
						let mut s = world.component_raw_mut(id);
						unsafe { s.load_raw(uss) };
					}
					for (id, uss) in resources {
						warn!("Replacing resource storage '{}' with raw persisted data", id);
						let mut s = world.resource_raw_mut(id);
						unsafe { s.load_raw(uss) };
					}
				}
			} else {
				debug!("Soft reload of {}", self.extensions[i].name);
				let e = self.extensions.get_mut(i).unwrap();
				e.library.as_mut().unwrap().load(&e.name, world)?;
			}

			load_queue.remove(&i);

			// Send update
			updates(LoadStatus {
				to_load: load_queue.iter()
					.map(|(&i, &h)| (self.extensions[i].name.clone(), h))
					.collect::<Vec<_>>(),
				loaded: (0..self.extensions.len())
					.filter(|i| load_queue.get(i).is_none())
					.map(|i| self.extensions[i].name.clone())
					.collect::<Vec<_>>(),
			});
			error!("Update: {} in queue", load_queue.len());
		}

		self.rebuild_systems()?;
		
		Ok(())
	}

	pub fn register(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
		let e = ExtensionEntry::new_crate(path)?;
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
		if let Some(i) = self.extensions.iter().position(|e| e.file_path.eq(path.as_ref())) {
			let e = self.extensions.remove(i);
			if let Some(mut lib) = e.library {
				lib.unload(world);
			}
		} else {
			return Err(anyhow!("Extension not found"));
		}
		self.reload(world, |_s| {})?;

		Ok(())
	}

	/// Returns work group name, contents, and run order. 
	/// Used for displaying visually. 
	pub fn workgroup_info(&self) -> Vec<(&String, Vec<(&String, &Vec<usize>)>, &Vec<Vec<usize>>)> {
		self.systems.iter().map(|(n, (s, o))| {
			let systems = s.iter()
				.map(|((ei, si), d)| (&self.extensions[*ei].library.as_ref().unwrap().systems[*si].id, d))
				.collect::<Vec<_>>();
			(n, systems, o)
		}).collect::<Vec<_>>()
	}

	/// Creates a list of systems and their dependencies. 
	// (Vec<(usize, usize)>, Vec<Vec<usize>>)
	fn get_systems_and_deps(&self, group: impl AsRef<str>) -> Vec<((usize, usize), Vec<usize>)> {
		// Vec of (extension index, system index in extension)
		let systems = self.extensions.iter().enumerate()
			.flat_map(|(i, e)| {
				e.library.as_ref().unwrap().systems.iter().enumerate()
					.filter(|(_, s)| s.group == group.as_ref())
					.map(move |(j, _)| (i, j))
			}).collect::<Vec<_>>();
		
		let deps = systems.iter().enumerate().map(|(i, &(ei, si))| {
			let s = &self.extensions[ei].library.as_ref().unwrap().systems[si];
			// Find group system index of dependencies
			let mut deps = s.run_after.iter()
				.map(|id: &String| systems.iter()
					.map(|&(ei, si)| &self.extensions[ei].library.as_ref().unwrap().systems[si])
					.position(|s| s.id.eq(id)).expect("Failed to find dependent system"))
				.collect::<Vec<_>>();
			// Add others to dependencies if they want to be run before
			for (j, &(ej, sj)) in systems.iter().enumerate() {
				if i == j { continue }
				let d = &self.extensions[ej].library.as_ref().unwrap().systems[sj];
				if d.run_before.contains(&s.id) {
					trace!("'{}' runs before '{}' so '{}' depends on '{}'", d.id, s.id, s.id, d.id);
					deps.push(j);
				}
			}
			deps
		}).collect::<Vec<_>>();

		systems.into_iter().zip(deps.into_iter()).collect()
	}

	/// Constructs a run order from a list of systems and their dependencies. 
	fn construct_run_order(&self, systems_deps: &Vec<((usize, usize), Vec<usize>)>) -> Vec<Vec<usize>> {
		// let systems_deps = self.get_systems_and_deps(group.as_ref());
		let mut queue = (0..systems_deps.len()).collect::<Vec<_>>();

		let mut stages = vec![vec![]];

		// Satisfied if in any of the PREVIOUS stages (but NOT the current stage) 
		fn satisfied(stages: &Vec<Vec<usize>>, i: usize) -> bool {
			(&stages[0..stages.len()-1]).iter().any(|systems| systems.contains(&i))
		}

		while !queue.is_empty() {
			let next = queue.iter().copied()
				.map(|i| &systems_deps[i])
				.position(|(_, deps)| deps.iter().copied().all(|i| satisfied(&stages, i)));
			if let Some(qi) = next {
				let i = queue.remove(qi);
				// debug!("Run '{}'", i);
				stages.last_mut().unwrap().push(i);
			} else {
				if stages.last().and_then(|s| Some(s.is_empty())).unwrap_or(false) {
					error!("Failing to meet some dependency!");
					error!("Stages are:");
					for (i, stage) in stages.into_iter().enumerate() {
						error!("{}:", i);
						for j in stage {
							let ((ei, si), _d) = &systems_deps[j];
							let s = &self.extensions[*ei].library.as_ref().unwrap().systems[*si];
							error!("\t'{}'", s.id);
						}
					}
					error!("Remaining items are:");
					for i in queue {
						let ((ei, si), d) = &systems_deps[i];
						let s = &self.extensions[*ei].library.as_ref().unwrap().systems[*si];
						let n = &s.id;
						let d = d.iter().copied().map(|i| {
							let ((ei, si), _d) = &systems_deps[i];
							let s = &self.extensions[*ei].library.as_ref().unwrap().systems[*si];
							&s.id
						}).collect::<Vec<_>>();
						error!("'{}' - {:?}", n, d);
					}
					panic!();
				}
				debug!("New stage");
				stages.push(Vec::new());
			}
		}

		stages
	}

	fn rebuild_systems(&mut self) -> anyhow::Result<()> {
		info!("Rebuilding workloads");

		let mut groups2 = HashMap::new();

		let mut groups = self.extensions.iter()
			.flat_map(|e| e.library.as_ref().unwrap().systems.iter())
			.map(|s| &s.group)
			.collect::<Vec<_>>();
		groups.sort_unstable();
		groups.dedup();

		debug!("There are {} workloads to build ({:?})", groups.len(), groups);

		for group in groups {
			debug!("Collect systems for group '{}'", group);
			let systems_deps = self.get_systems_and_deps(group);
			debug!("{} systems are found", systems_deps.len());

			debug!("Construct run order for group '{}'", group);
			let run_order = self.construct_run_order(&systems_deps);
			debug!("Run in {} stages", run_order.len());

			groups2.insert(group.clone(), (systems_deps, run_order));
		}

		self.systems = groups2;

		Ok(())
	}

	pub fn run(&self, world: &mut World, group: impl AsRef<str>) -> anyhow::Result<()> {
		debug!("Running '{}'", group.as_ref());
		let (systems_deps, run_order) = self.systems.get(&group.as_ref().to_string())
			.with_context(|| "Failed to locate workload")?;

		for stage in run_order {
			for &i in stage {
				let ((ei, si), _) = &systems_deps[i];
				let e = &self.extensions[*ei];
				let s = &e.library.as_ref().unwrap().systems[*si];
				trace!("Extension '{}' system '{}'", e.name, s.id);
				profiling::scope!(format!("{}::{}", e.name, s.id));
				let w = world as *const World;
				(s.pointer)(w);
			}
		} 

		Ok(())
	}
}
// impl Drop for ExtensionRegistry {
// 	fn drop(&mut self) {
// 		// Any references to the data in a library must be dropped before the library itself 
// 		self.systems.clear();
// 	}
// }
