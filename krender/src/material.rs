use std::{path::{PathBuf, Path}, collections::HashMap, ffi::OsStr};
use serde::{Serialize, Deserialize};
use slotmap::SlotMap;
use crate::{ShaderKey, shader::{ShaderManager, ShaderEntry, BindGroupEntry}, texture::{TextureManager, Texture}, buffer::BufferManager, MaterialKey};



#[derive(thiserror::Error, Debug)]
pub enum MaterialError {
	#[error("This material has not been ititialized!")]
	MaterialNotInitializedError,
	
	#[error("Failed to find shader ('{0:?}') for this material!")]
	MaterialShaderNotFoundError(PathBuf),

	#[error("This material has an outdated shader!")]
	MaterialShaderOutdatedError,

	/// Used when the shader requests an id not supplied by the material. 
	#[error("Failed to find binding data ('{0}') for this material!")]
	MaterialMappingMissingError(String),

	/// Used when a resource is not found. 
	#[error("Failed to find resource '{1}' for material '{0}'!")]
	MaterialResourceMissingError(String, MaterialResourceLocation),
}


/// Is it global or context-dependent?
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MaterialResourceLocation {
	Global(GlobalResourceIdentifier), // Global resources may be accessed by path or label
	Context(String), // Context resources may only be accessed by label 
}
impl std::fmt::Display for MaterialResourceLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}


/// Identifies a resource by name or path
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GlobalResourceIdentifier {
	Label(String),
	Path(PathBuf), // This allows us to read a texture automatically! 
}


/// Must provide data for every ID in the shader specification.
/// If it is missing something, then it should throw an error. 
#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSpecification {
	pub name: String,
	pub shader: PathBuf,

	pub mappings: HashMap<String, MaterialResourceLocation>,
	pub array_mappings: HashMap<String, Vec<MaterialResourceLocation>>,
}
impl MaterialSpecification {
	pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		let path = path.as_ref().canonicalize()?;

		let f = std::fs::File::open(&path)?;
		let s = ron::de::from_reader::<std::fs::File, Self>(f)?;
		Ok(s.canonicalize(path.parent().unwrap())?)
	}

	fn canonicalize(mut self, context: impl AsRef<Path>) -> Result<Self, std::io::Error> {
		trace!("Canonicalize material {}", self.name);
		let context: &Path = context.as_ref();
		self.shader = context.join(&self.shader).canonicalize()?;

		for rl in self.mappings.values_mut() {
			if let MaterialResourceLocation::Global(GlobalResourceIdentifier::Path(p)) = rl {
				let g = context.join(&p);
				trace!("Canonicalize {g:?}");
				*p = g.canonicalize()?;
			}
		}

		for rl in self.array_mappings.values_mut().flatten() {
			if let MaterialResourceLocation::Global(GlobalResourceIdentifier::Path(p)) = rl {
				let g = context.join(&p);
				trace!("Canonicalize {g:?}");
				*p = g.canonicalize()?;
			}
		}

		Ok(self)
	}
}


#[derive(Debug)]
pub struct MaterialEntry {
	pub specification: MaterialSpecification,
	pub path: Option<PathBuf>,
	pub key: MaterialKey,

	// If it's some, then the material is in the shaders dependents list
	pub shader_key: Option<ShaderKey>,
}
impl MaterialEntry {
	fn shader_entry<'a>(&mut self, shaders: &'a ShaderManager) -> Result<&'a ShaderEntry, MaterialError> {
		if let Some(key) = self.shader_key {
			Ok(shaders.get(key).unwrap())
		} else {
			trace!("Material '{}' looks for its shader", self.specification.name);
			let key = shaders.index_from_path(&self.specification.shader)
			.ok_or(MaterialError::MaterialShaderNotFoundError(self.specification.shader.clone()))?;
			self.shader_key = Some(key);

			let s = shaders.get(key).unwrap();
			s.add_dependent_material(self.key);
			Ok(s)
		}
	}

	/// This can only be done after the shader has been loaded. 
	/// This is because we need to know if the thing is a buffer or a texture. 
	pub fn read_unknown_resources(
		&mut self, 
		shaders: &ShaderManager,
		textures: &mut TextureManager,
		_buffers: &mut BufferManager,
	) -> anyhow::Result<()> {
		let shader = self.shader_entry(shaders)?;
		for bg in shader.specification.bind_groups.values() {
			for bge in bg.values() {
				match bge {
					BindGroupEntry::Texture(id, format, _, _, _, _) => {
						let data = self.specification.mappings.get(id)
							.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
						if let MaterialResourceLocation::Global(GlobalResourceIdentifier::Path(path)) = data {
							if textures.key_by_path(path).is_none() {
								debug!("Material '{}' loads texture at path {path:?}", self.specification.name);
								trace!("extension is {:?}", path.extension());
								if path.extension().and_then(|e| Some(e.eq(OsStr::new("ron")))).unwrap_or(false) {
									trace!("That is a texture specification!");
									let texture = Texture::from_specification_path(path).unwrap();
									textures.insert(texture);

								} else {
									let name = path.file_name().unwrap().to_str().unwrap();
									let texture = Texture::from_d2_path(name, path, *format, false);
									textures.insert(texture);
								}								
							}
						}
					},
					_ => warn!("Not checking for readable resources for in {bge:?} (unimplemented)"),
				};
			}
		}
		Ok(())
	}
}


#[derive(Debug, Default)]
pub struct MaterialManager {
	pub(crate) materials: SlotMap<MaterialKey, MaterialEntry>,
	materials_by_name: HashMap<String, MaterialKey>,
	materials_by_path: HashMap<PathBuf, MaterialKey>,
}
impl MaterialManager {
	pub fn new() -> Self {
		Self {
			materials: SlotMap::with_key(),
			materials_by_name: HashMap::new(),
			materials_by_path: HashMap::new(),
		}
	}

	pub fn insert(&mut self, specification: MaterialSpecification) -> MaterialKey {
		let name = specification.name.clone();
		let k = self.materials.insert_with_key(|key| MaterialEntry {
			specification,
			path: None,
			key,
			shader_key: None,
		});
		self.materials_by_name.insert(name, k);
		k
	}

	pub fn read(&mut self, path: impl Into<PathBuf>) -> MaterialKey {
		let path = path.into();
		let path = path.canonicalize().expect(&*format!("not find {path:?}"));
		
		if let Some(k) = self.key_by_path(&path) {
			return k;
		}

		let specification = MaterialSpecification::read(&path).unwrap();
		
		let name = specification.name.clone();
		let k = self.materials.insert_with_key(|key| MaterialEntry {
			specification,
			path: Some(path.clone()),
			key,
			shader_key: None,
		});
		self.materials_by_name.insert(name, k);
		self.materials_by_path.insert(path, k);
		k
	}

	pub fn get(&self, key: MaterialKey) -> Option<&MaterialEntry> {
		self.materials.get(key)
	}

	pub fn key_by_name(&self, name: impl Into<String>) -> Option<MaterialKey> {
		self.materials_by_name.get(&name.into()).cloned()
	}

	pub fn key_by_path(&self, path: impl Into<PathBuf>) -> Option<MaterialKey> {
		self.materials_by_path.get(&path.into()).cloned()
	}

	pub fn insert_direct(&mut self, material: MaterialEntry) -> MaterialKey {
		let name = material.specification.name.clone();
		let key = self.materials.insert(material);
		self.materials_by_name.insert(name, key);
		key
	}

	pub fn remove(&mut self, key: MaterialKey) {
		if let Some(material) = self.materials.remove(key) {
			self.materials_by_name.remove(&material.specification.name);
			todo!("Decrement bind group counters (BindGrouPManager::decrement_counter()")
		}
	}

	/// Register shaders found in the material specification. 
	/// Adds the material as a dependent. 
	/// Fetches their keys. 
	#[profiling::function]
	pub(crate) fn read_shaders_and_fetch_keys(
		&self, 
		shaders: &mut ShaderManager,
	) {
		self.materials.values()
			.for_each(|m| {
				if shaders.index_from_path(&m.specification.shader).is_none() {
					shaders.read(&m.specification.shader);
				}
			});
	}

	/// Reads specified resources in the material files. 
	/// 
	/// Todo: Track why a resource is loaded (manually or by material) so that it mgiht be unloaded.
	#[profiling::function] 
	pub(crate) fn read_specified_resources(
		&mut self, 
		shaders: & ShaderManager,
		textures: &mut TextureManager,
		buffers: &mut BufferManager
	) -> anyhow::Result<()> {
		for m in self.materials.values_mut() {
			m.read_unknown_resources(shaders, textures, buffers)?;
		}
		Ok(())
	}
}

