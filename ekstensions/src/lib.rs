use std::{collections::HashMap, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
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
			panic!();
		}
		
		let name = cargo_toml_table
			.get("package").unwrap()
			.as_table().unwrap()
			.get("name").unwrap()
			.as_str().unwrap();

		let last_mod = Self::src_files_last_modified(path.as_ref());
		
		let ext_file = Self::stored_extension_file(name, last_mod);
		let ext_folder = ext_file.parent().unwrap();
		std::fs::create_dir_all(ext_folder).unwrap();
		trace!("Extension file should be {:?}", ext_file);

		// Try to find an existing extension file
		// There should only be one 
		let ext_previous = std::fs::read_dir(ext_folder).ok().and_then(|rd| rd
			.filter_map(|f| f.ok())
			.map(|f| f.path())
			.find(|f| f.extension().map(|e| e == "so").unwrap_or(false)));
		trace!("Previous extension file {:?}", ext_previous);

		// If cached file not up to date
		if ext_previous.as_ref().map(|p| !p.eq(&ext_file)).unwrap_or(true) {
			trace!("Either extension file does not exist or is outdated");

			let out_path = Self::build_output_path(path.as_ref(), &name)?;
			// If output not exists or is outdated
			if out_path.canonicalize().ok().map(|p| p.metadata().unwrap().modified().unwrap() <= last_mod).unwrap_or(true) {
				trace!("No/outdated output file, rebuilding");

				let status = std::process::Command::new("cargo")
					.arg("build")
					// .env("RUSTC_WRAPPER", "/usr/bin/sccache")
					.arg("-p")
					.arg(name)
					.status()
					.with_context(|| "cargo build failed")?;
				if !status.success() {
					error!("Failed to compile extension");
					panic!();
				}
			} else {
				trace!("We have a fresh output file :)");
			}

			// Copy to extension storage 

			// let lib_path = if out_path.exists() {
			// 	trace!("Library files exist, copying to library swap");

			// 	let lib_path = Self::stored_library_file(&name);
			// 	trace!("{:?} -> {:?}", out_path, lib_path);
			// 	std::fs::create_dir_all(lib_path.parent().unwrap()).unwrap();
			// 	std::fs::copy(&out_path, &lib_path).unwrap();
			// 	std::fs::remove_file(&out_path).unwrap();

			// 	Some(lib_path)
			// } else {
			// 	trace!("No library build detected");
			// 	None
			// };

			trace!("Moving output to extension storage");
			trace!("{:?} -> {:?}", out_path, ext_file);
			std::fs::create_dir_all(ext_file.parent().unwrap()).unwrap();
			std::fs::copy(&out_path, &ext_file).unwrap();

			if let Some(pp) = ext_previous.as_ref() {
				trace!("Deleting old extension file {:?}", pp);
				std::fs::remove_file(&pp).unwrap();
			}

			// if let Some(lib_path) = lib_path {
			// 	trace!("Restoring previous library files from library swap");
			// 	trace!("{:?} -> {:?}", lib_path, out_path);
			// 	std::fs::copy(&lib_path, &out_path).unwrap();
			// 	std::fs::remove_file(&lib_path).unwrap();
			// } 
			// else {
			// 	std::fs::remove_file(&out_path).unwrap();
			// }
		} else {
			trace!("Cached file is up to date :D");
		}

		assert!(std::fs::canonicalize(&ext_file).is_ok(), "output path is bad");
		trace!("Loading extension from {:?}", ext_file);
		let library = unsafe { libloading::Library::new(&ext_file)? };
		let library_ts = ext_file.metadata().unwrap().modified().unwrap();
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
	/// 
	/// TODO: extensions on non-linux platforms
	fn stored_extension_file(extension_name: impl AsRef<str>, last_mod: SystemTime) -> PathBuf {
		let epoch_dur = last_mod.duration_since(UNIX_EPOCH).unwrap();
		PathBuf::from("target/extensions")
			.join(extension_name.as_ref())
		   .join(format!("{}.so", epoch_dur.as_nanos()))
	}

	// fn stored_library_file(extension_name: impl AsRef<str>) -> PathBuf {
	// 	PathBuf::from("target/tmp")
	// 	   .join(Self::out_name(extension_name))
	// }

	fn out_name(extension_name: impl AsRef<str>) -> PathBuf {
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

		let dylib_path = dylib_output_dir.join(Self::out_name(name));

		Ok(dylib_path)
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
	
	// Checks if this extension has been modified since it was last loaded
	pub fn dirty_level(&self) -> DirtyLevel {	
		// If build exists, then look at the .d file to find all depenedent files
		// Rebuld if any changed after the build file (not the .d file?)

		let last_mod = Self::src_files_last_modified(&self.path);
		if last_mod > self.read_at {
			trace!("Source files modified, rebuild");
			return DirtyLevel::Rebuild;
		}

		// If newer build file exists on disk 
		let ext_path = Self::stored_extension_file(&self.name, last_mod);
		if let Ok(p) = ext_path.canonicalize() {
			// If modified after we last loaded
			let mod_time = p.metadata().unwrap().modified().unwrap();
			if mod_time > self.read_at {
				let d = mod_time.duration_since(self.read_at).unwrap();
				trace!("Extension file is newer, reload ({:?}, {:.2}s)", p, d.as_secs_f32());
				return DirtyLevel::Reload;
			} else {
				return DirtyLevel::Clean;
			}
		} else {
			trace!("Extension file does not exist, rebuild");
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
			let n = format!("{}_load", self.name);
			let f = self.library.get::<unsafe extern fn(&mut ExtensionStorageLoader) -> ()>(n.as_bytes())?;
			f(&mut loader);
		}
		
		let _ = self.provides.insert(loader.storages);
		
		Ok(())
	}

	// Extensions don't need much in their unlaod functions by default
	// Systems and components and resources will be removed automatically
	// In the future maybe we should be able to choose whether to serialize the data and try to reload it 
	// This would use serde so that if the data format changed the restoration can fail 
	pub fn unload(
		&mut self, world: &mut World
	) -> anyhow::Result<(
		Vec<(String, UntypedSparseSet)>,
		Vec<(String, UntypedResource)>,
	)> {
		assert!(self.loaded(), "Extension is not loaded!");

		// unsafe {
		// 	let f = self.library.get::<unsafe extern fn() -> ()>(b"unload")?;
		// 	f()
		// }

		let provisions = self.provides.take().unwrap();
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
impl Drop for ExtensionEntry {
	fn drop(&mut self) {
		if self.provides.is_some() {
			warn!("Dropping extension '{}' with active provisions", self.name);
		}
		// Any references to the data in a library must be dropped before the library itself 
		self.systems.clear();
		warn!("Done that");
	}
}


pub struct ExtensionRegistry {
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

	pub fn reload(&mut self, world: &mut World) -> anyhow::Result<()> {
		let mut load_queue = HashMap::new();

		trace!("Look for rebuilds");

		// Rebuild
		for i in 0..self.extensions.len() {
			if !self.extensions[i].loaded() {
				load_queue.entry(i).or_insert(false);
				continue
			}

			match self.extensions[i].dirty_level() {
				DirtyLevel::Rebuild => {
					debug!("Rebuild extension {}", self.extensions[i].name);
					load_queue.insert(i, true);
					// Push dependents to reload queue
					for (j, e) in self.extensions.iter().enumerate() {
						if e.load_dependencies.contains(&self.extensions[i].name) {
							load_queue.entry(j).or_insert(false);
						}
					}
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

		// TODO: Dependency load order
		for (i, hard) in load_queue {
			if hard {
				let mut ext = self.extensions.remove(i);
				debug!("Hard reload of {}", ext.name);
				
				// Unload from world
				trace!("Unloading from world...");
				let (c, r) = ext.unload(world)?;

				trace!("Removing storages...");
				// Rip out references to data
				let c = c.into_iter()
					.map(|(n, c)| (n, unsafe { c.into_raw() }))
					.collect::<Vec<_>>();
				let r = r.into_iter()
					.map(|(n, r)| (n, unsafe { r.into_raw() }))
					.collect::<Vec<_>>();

				// TODO: if serializable, use serialization
				// Needs untypedsparseset to finish serialization feature 

				// Reload code
				let path = ext.path.clone();
				trace!("Dropping old extension entry...");
				drop(ext);
				trace!("Loading new extension entry...");
				self.extensions.insert(i, ExtensionEntry::new(path)?);

				// Load with new code
				trace!("Loading into world...");
				self.extensions[i].load(world)?;

				// Replace storages
				trace!("Overwriting storages...");
				for (id, uss) in c {
					warn!("Replacing component storage '{}' with raw persisted data", id);
					let mut s = world.component_raw_mut(id);
					unsafe { s.load_raw(uss) };
				}
				for (id, uss) in r {
					warn!("Replacing resource storage '{}' with raw persisted data", id);
					let mut s = world.resource_raw_mut(id);
					unsafe { s.load_raw(uss) };
				}
			} else {
				debug!("Soft reload of {}", self.extensions[i].name);
				self.extensions[i].load(world)?;
			}
		}

		self.rebuild_systems()?;
		
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

	/// Returns work group name, contents, and run order. 
	/// Used for displaying visually. 
	pub fn workgroup_info(&self) -> Vec<(&String, Vec<(&String, &Vec<usize>)>, &Vec<Vec<usize>>)> {
		self.systems.iter().map(|(n, (s, o))| {
			let systems = s.iter()
				.map(|((ei, si), d)| (&self.extensions[*ei].systems[*si].id, d))
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
				e.systems.iter().enumerate()
					.filter(|(_, s)| s.group == group.as_ref())
					.map(move |(j, _)| (i, j))
			}).collect::<Vec<_>>();
		
		let deps = systems.iter().enumerate().map(|(i, &(ei, si))| {
			let s = &self.extensions[ei].systems[si];
			// Find group system index of dependencies
			let mut deps = s.run_after.iter()
				.map(|id: &String| systems.iter()
					.map(|&(ei, si)| &self.extensions[ei].systems[si])
					.position(|s| s.id.eq(id)).expect("Failed to find dependent system"))
				.collect::<Vec<_>>();
			// Add others to dependencies if they want to be run before
			for (j, &(ej, sj)) in systems.iter().enumerate() {
				if i == j { continue }
				let d = &self.extensions[ej].systems[sj];
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
							let s = &self.extensions[*ei].systems[*si];
							error!("\t'{}'", s.id);
						}
					}
					error!("Remaining items are:");
					for i in queue {
						let ((ei, si), d) = &systems_deps[i];
						let s = &self.extensions[*ei].systems[*si];
						let n = &s.id;
						let d = d.iter().copied().map(|i| {
							let ((ei, si), _d) = &systems_deps[i];
							let s = &self.extensions[*ei].systems[*si];
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
			.flat_map(|e| e.systems.iter())
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
				let s = &e.systems[*si];
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
