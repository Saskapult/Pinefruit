use std::{path::{PathBuf, Path}, collections::HashMap, sync::atomic::{AtomicBool, Ordering}};
use serde::{Serialize, Deserialize};
use slotmap::{SlotMap, SecondaryMap, SparseSecondaryMap};

use crate::{ShaderKey, BindGroupKey, shader::{ShaderManager, ShaderEntry, BindGroupEntry, ShaderBase}, texture::{TextureManager, Texture}, buffer::BufferManager, prelude::BindGroupManager, bindgroup::{BindGroupEntryContentDescriptor, SamplerDescriptor}, MaterialKey, TextureKey, BufferKey, rendertarget::RenderTarget, RenderContextKey, rendercontext::RenderContextManager, EntityIdentifier};



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
	#[error("Failed to find resource '{0}' for this material!")]
	MaterialResourceMissingError(ResourceLocation),
}


/// Is it global or context-dependent?
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResourceLocation {
	Global(GlobalResourceIdentifier), // Global resources may be accessed by path or label
	Context(String), // Context resources may only be accessed by label 
}
impl std::fmt::Display for ResourceLocation {
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

	pub mappings: HashMap<String, ResourceLocation>,
	pub array_mappings: HashMap<String, Vec<ResourceLocation>>,
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
			if let ResourceLocation::Global(GlobalResourceIdentifier::Path(p)) = rl {
				let g = context.join(&p);
				trace!("Canonicalize {g:?}");
				*p = g.canonicalize()?;
			}
		}

		for rl in self.array_mappings.values_mut().flatten() {
			if let ResourceLocation::Global(GlobalResourceIdentifier::Path(p)) = rl {
				let g = context.join(&p);
				trace!("Canonicalize {g:?}");
				*p = g.canonicalize()?;
			}
		}

		Ok(self)
	}
}


#[derive(Debug)]
pub struct MaterialBinding {
	pub render_target: Option<RenderTarget>,
	pub bind_groups: [Option<BindGroupKey>; 4],
	pub context: RenderContextKey,
	pub texture_usages: SparseSecondaryMap<TextureKey, wgpu::TextureUsages>,
	pub buffer_usages: SparseSecondaryMap<BufferKey, wgpu::BufferUsages>,
}
impl MaterialBinding {
	pub fn polygon_stuff(&self) -> (&RenderTarget, [Option<BindGroupKey>; 4]) {
		(self.render_target.as_ref().unwrap(), self.bind_groups)
	}
}


#[derive(Debug)]
pub struct MaterialEntry {
	pub specification: MaterialSpecification,
	pub path: Option<PathBuf>,
	pub key: MaterialKey,

	// If it's some, then the material is in the shaders dependents list
	pub shader_key: Option<ShaderKey>,
	bindings: SecondaryMap<RenderContextKey, (AtomicBool, MaterialBinding)>,
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

	pub fn shader(&self) -> Option<ShaderKey> {
		self.shader_key
	}

	pub fn binding(&self, context: RenderContextKey) -> Option<&MaterialBinding> {
		self.bindings.get(context).and_then(|(_, b)| Some(b))
	}

	pub fn update_bindings<T: EntityIdentifier>(
		&mut self, 
		contexts: &RenderContextManager<T>,
		shaders: &ShaderManager,
		textures: &TextureManager,
		buffers: &BufferManager,
		bind_groups: &mut BindGroupManager,
	) -> Result<(), MaterialError> {
		let shader = self.shader_entry(shaders)?;

		for (context_key, context) in contexts.render_contexts.iter() {
			if let Some((d, _)) = self.bindings.get(context_key) {
				if !d.load(Ordering::Relaxed) {
					continue;
				}
			}
			trace!("Binding material '{}' for render context '{}'", self.specification.name, context.name);

			// Usages will be collected in the following stages
			// At the end, we will apply them
			let mut texture_usages = SparseSecondaryMap::new();
			let mut buffer_usages = SparseSecondaryMap::new();
			let mut add_texture_usages = |key, usages| {
				if let Some(u) = texture_usages.get_mut(key) {
					*u = *u | usages;
				} else {
					texture_usages.insert(key, usages);
				}
			};
			let mut add_buffer_usages = |key, usages| {
				trace!("Add buffer usages");
				if let Some(u) = buffer_usages.get_mut(key) {
					*u = *u | usages;
				} else {
					buffer_usages.insert(key, usages);
				}
				assert_ne!(0, buffer_usages.len());
			};			

			trace!("Bind group stuff");
			// Bind groups
			let mut bindings = [None; 4];
			for (&i, group) in shader.specification.bind_groups.iter() {
				let mut binding_config = [None, None, None, None];
				for (&j, entry) in group.iter() {
					let content = match entry {
						BindGroupEntry::UniformBuffer(id, _) => {
							let data = self.specification.mappings.get(id)
								.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
							let key = match data {
								ResourceLocation::Global(id) => match id {
									GlobalResourceIdentifier::Label(label) => buffers.key(label),
									GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
								},
								ResourceLocation::Context(label) => context.buffers.get(label).cloned(),
							}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
							trace!("Adding uniform usages to buffer {:?} ({:?})", data, key);
							add_buffer_usages(key, wgpu::BufferUsages::UNIFORM);
							BindGroupEntryContentDescriptor::Buffer(key)
						},
						BindGroupEntry::UniformBuffers(ids, _) => {
							let keys = ids.iter()
								.map(|id| self.specification.mappings.get(id).ok_or(MaterialError::MaterialMappingMissingError(id.clone())))
								.collect::<Result<Vec<_>, MaterialError>>()?
								.iter()
								.map(|&data| match data {
									ResourceLocation::Global(id) => match id {
										GlobalResourceIdentifier::Label(label) => buffers.key(label),
										GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
									},
									ResourceLocation::Context(label) => context.buffers.get(label).cloned(),
								}.ok_or(MaterialError::MaterialResourceMissingError(data.clone())))
								.collect::<Result<Vec<_>, MaterialError>>()?;
							keys.iter().for_each(|&key| add_buffer_usages(key, wgpu::BufferUsages::UNIFORM));
							BindGroupEntryContentDescriptor::Buffers(keys)
						},
						BindGroupEntry::UniformBufferArray(id, _, _) => {
							let ids = self.specification.array_mappings.get(id)
								.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
							let keys = ids.iter()
								.map(|data| match data {
									ResourceLocation::Global(id) => match id {
										GlobalResourceIdentifier::Label(label) => buffers.key(label),
										GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
									},
									ResourceLocation::Context(label) => context.buffers.get(label).cloned(),
								}.ok_or(MaterialError::MaterialResourceMissingError(data.clone())))
								.collect::<Result<Vec<_>, MaterialError>>()?;
							keys.iter().for_each(|&key| add_buffer_usages(key, wgpu::BufferUsages::UNIFORM));
							BindGroupEntryContentDescriptor::Buffers(keys)
						},
						BindGroupEntry::Texture(id, _, _, _, _, _) => {
							let data = self.specification.mappings.get(id)
								.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
							let key = match data {
								ResourceLocation::Global(id) => match id {
									GlobalResourceIdentifier::Label(label) => textures.key_by_name(label),
									GlobalResourceIdentifier::Path(path) => textures.key_by_path(path),
								},
								ResourceLocation::Context(label) => context.textures.get(label).copied(),
							}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
							add_texture_usages(key, wgpu::TextureUsages::TEXTURE_BINDING);
							BindGroupEntryContentDescriptor::Texture(key)
						},
						BindGroupEntry::Sampler(_, address, mag_filter, min_filter, mipmap_filter, lod_min_clamp, lod_max_clamp, _) => {
							let key = bind_groups.make_or_fetch_sampler(SamplerDescriptor {
								address: (*address).into(),
								mag_filter: (*mag_filter).into(),
								min_filter: (*min_filter).into(),
								mipmap_filter: (*mipmap_filter).into(),
								lod_min_clamp: *lod_min_clamp,
								lod_max_clamp: *lod_max_clamp,
							});
		
							BindGroupEntryContentDescriptor::Sampler(key)
						},
						// I just copied this from uniform buffer
						BindGroupEntry::StorageBuffer(id, _, _) => {
							let data = self.specification.mappings.get(id)
								.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
							let key = match data {
								ResourceLocation::Global(id) => match id {
									GlobalResourceIdentifier::Label(label) => buffers.key(label),
									GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
								},
								ResourceLocation::Context(label) => context.buffers.get(label).cloned(),
							}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
							trace!("Adding uniform usages to buffer {:?} ({:?})", data, key);
							add_buffer_usages(key, wgpu::BufferUsages::STORAGE);
							BindGroupEntryContentDescriptor::Buffer(key)
						}
						_ => todo!(),
					};
					binding_config[j as usize] = Some(content);
				}
				// Could make layout here or when shader is registered or smnk
				let layout_key = shader.bind_group_layout_keys.unwrap()[i as usize].unwrap();
				let k = bind_groups.i_need_a_bind_group(binding_config, layout_key, textures, buffers);
				bindings[i as usize] = Some(k);
			}
			
			trace!("Render target stuff");
			// Render target
			let render_target = if let ShaderBase::Polygonal(base) = &shader.specification.base {
				let mut render_target = RenderTarget::new();

				for attachment in base.attachments.iter() {
					let data = self.specification.mappings.get(&attachment.source)
						.ok_or(MaterialError::MaterialMappingMissingError(attachment.source.clone()))?;
					let key = match data {
						ResourceLocation::Global(id) => match id {
							GlobalResourceIdentifier::Label(label) => textures.key_by_name(label),
							GlobalResourceIdentifier::Path(_) => todo!("Global path identifiers cannot be used in render attachments!"),
						},
						ResourceLocation::Context(label) => context.textures.get(label).copied(),
					}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
					add_texture_usages(key, wgpu::TextureUsages::RENDER_ATTACHMENT);
					render_target = render_target.with_colour(key, None);
				}

				if let Some(depth) = base.depth.as_ref() {
					let data = self.specification.mappings.get(&depth.source)
						.ok_or(MaterialError::MaterialMappingMissingError(depth.source.clone()))?;
					let key = match data {
						ResourceLocation::Global(id) => match id {
							GlobalResourceIdentifier::Label(label) => textures.key_by_name(label),
							GlobalResourceIdentifier::Path(_) => panic!("Global path identifiers cannot be used in render attachments!"),
						},
						ResourceLocation::Context(label) => context.textures.get(label).copied(),
					}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;					
					add_texture_usages(key, wgpu::TextureUsages::RENDER_ATTACHMENT);
					render_target = render_target.with_depth(key);
				}

				Some(render_target)
			} else {
				None
			};

			trace!("Usage application");
			// Apply resource usages
			for (key, &usages) in texture_usages.iter() {
				textures.add_dependent_material(key, self.key, context_key, usages);
			}
			for (key, &usages) in buffer_usages.iter() {
				buffers.add_dependent_material(key, self.key, context_key, usages);
			}

			trace!("Done!");

			self.bindings.insert(context_key, (AtomicBool::new(false), MaterialBinding { 
				render_target, 
				bind_groups: bindings, 
				context: context_key, 
				texture_usages, 
				buffer_usages,
			}));
		}

		Ok(())
	}

	/// This can only be done after the shader has been loaded. 
	/// This is because we need to know if the thing is a buffer or a texture. 
	pub fn read_unknown_resources(
		&mut self, 
		shaders: &ShaderManager,
		textures: &mut TextureManager,
		_buffers: &mut BufferManager
	) -> anyhow::Result<()> {
		let shader = self.shader_entry(shaders)?;
		for bg in shader.specification.bind_groups.values() {
			for bge in bg.values() {
				match bge {
					BindGroupEntry::Texture(id, format, _, _, _, _) => {
						let data = self.specification.mappings.get(id)
							.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
						if let ResourceLocation::Global(GlobalResourceIdentifier::Path(path)) = data {
							if textures.key_by_path(path).is_none() {
								debug!("Material '{}' loads texture at path {path:?}", self.specification.name);
								let name = path.file_name().unwrap().to_str().unwrap();
								let texture = Texture::new_from_path(name, path, *format, false);
								textures.insert(texture);
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
	materials: SlotMap<MaterialKey, MaterialEntry>,
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
			bindings: SecondaryMap::new(),
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
			bindings: SecondaryMap::new(),
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

	pub fn mark_dirty(&self, key: MaterialKey, context: RenderContextKey) {
		if let Some(material) = self.materials.get(key) {
			if let Some((d, _)) = material.bindings.get(context) {
				d.store(true, Ordering::Relaxed);
			} else {
				warn!("Tried to mark a nonexistent context binding as dirty");
			}
		} else {
			warn!("Tried to mark a material as dirty");
		}
	}

	/// Register shaders found in the material specification
	pub fn read_unknown_shaders(
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

	pub fn read_unknown_resources(
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

	/// Register resources usages and declare bind groups
	pub fn update<T: EntityIdentifier>(
		&mut self,
		shaders: &ShaderManager,
		textures: &TextureManager,
		buffers: &BufferManager,
		bind_groups: &mut BindGroupManager,
		contexts: &RenderContextManager<T>,
	) {
		self.materials.values_mut()
			.for_each(|m| {
				m.update_bindings(contexts, shaders, textures, buffers, bind_groups).unwrap();
			});
	}
}

