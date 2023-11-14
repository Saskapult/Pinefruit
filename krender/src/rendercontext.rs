use std::{collections::HashMap, sync::atomic::{AtomicBool, Ordering}};
use slotmap::{SlotMap, SecondaryMap};
use crate::{EntityIdentifier, TextureKey, BufferKey, RenderContextKey, MaterialKey, BindGroupKey, material::{MaterialManager, MaterialError, MaterialResourceLocation, GlobalResourceIdentifier}, shader::{ShaderManager, BindGroupEntry}, bindgroup::{BindGroupEntryContentDescriptor, SamplerDescriptor}, prelude::BindGroupManager, texture::TextureManager, buffer::BufferManager};



#[derive(Debug)]
pub(crate) struct MaterialBinding {
	pub bind_groups: [Option<BindGroupKey>; 4],
	pub texture_usages: Vec<(TextureKey, wgpu::TextureUsages)>,
	pub buffer_usages: Vec<(BufferKey, wgpu::BufferUsages)>,
	pub dirty: AtomicBool,
}


#[derive(Debug)]
pub struct RenderContext<T: EntityIdentifier> {
	pub name: String, // For debugging
	// Systems take this entity and extract data from it to make context data
	// Like camera buffer system, which looks for a camera component and then
	// Writes its data to the camera buffer (a context resource)
	pub entity: Option<T>,

	pub textures: HashMap<String, TextureKey>,
	pub buffers: HashMap<String, BufferKey>,

	pub(crate) material_bindings: SecondaryMap<MaterialKey, MaterialBinding>,

	pub key: RenderContextKey,
}
impl<T: EntityIdentifier> RenderContext<T> {
	pub fn new(name: impl Into<String>, key: RenderContextKey) -> Self {
		Self {
			name: name.into(),
			entity: None,
			textures: HashMap::new(),
			buffers: HashMap::new(),
			material_bindings: SecondaryMap::new(),
			key,
		}
	}

	pub fn with_entity(mut self, entity_id: T) -> Self {
		self.entity = Some(entity_id);
		self
	}

	// Todo: make this rebuild materials
	pub fn insert_texture(&mut self, label: impl Into<String>, key: TextureKey) {
		let label = label.into();
		trace!("Context insert texture '{label}'");
		if self.textures.insert(label.clone(), key).is_some() {
			warn!("Replace context texture '{}'", label);
			// Because a context resource has been changed, this could mean
			// that any material binding could be invalid
			// We should remove all the bindings when this happens
			// Need to drain the slotmap and also remove usages from each 
			// resource!
			todo!("Remove all context materials");
		}
	}

	// Todo: make this rebuild materials
	pub fn insert_buffer(&mut self, label: impl Into<String>, key: BufferKey) {
		let label = label.into();
		info!("context buffer {label}");
		if self.buffers.insert(label.clone(), key).is_some() {
			warn!("Replace context buffer '{}'", label);
			todo!("Remove all context materials");
		}
	}
	
	pub fn texture(&self, id: impl Into<String>) -> Option<TextureKey> {
		self.textures.get(&id.into()).copied()
	}

	pub fn bind_materials(
		&mut self,
		materials: &MaterialManager,
		shaders: &ShaderManager,
		textures: &TextureManager,
		buffers: &BufferManager,
		bind_groups: &mut BindGroupManager,
	) -> Result<(), MaterialError> {
		info!("Binding materials for render context '{}'", self.name);
		for (material_key, material) in materials.materials.iter() {
			// If dirty or DNE
			if match self.material_bindings.get(material_key) {
				Some(binding) => binding.dirty.load(Ordering::Relaxed),
				None => true,
			} {
				// Remove old usages if old binding exists
				if let Some(binding) = self.material_bindings.remove(material_key) {
					debug!("(Re)Binding material '{}'", material.specification.name);

					trace!("Removing usages for old binding");
					for (texture, _) in binding.texture_usages {
						textures.remove_dependent_material(texture, material_key, self.key);
					}
					for (buffer, _) in binding.buffer_usages {
						buffers.remove_usages(buffer, material_key, self.key);
					}

					warn!("Todo: Decrement bind groups' usage counters");
				} else {
					debug!("Binding material '{}'", material.specification.name);
				}
				
				let shader_key = material.shader_key
					.expect("Material has no shader key!");
				let shader = shaders.get(shader_key).unwrap();

				// Use sparse secondary slotmap?
				let mut texture_usages = Vec::new();
				let mut buffer_usages = Vec::new();

				let mut bindings = [None; 4];
				for (&i, group) in shader.specification.bind_groups.iter() {
					trace!("Bind group {i}");
					let mut binding_config = [None, None, None, None];
					for (&j, entry) in group.iter() {
						let content = match entry {
							BindGroupEntry::UniformBuffer(id, _) => {
								let data = material.specification.mappings.get(id)
									.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
								let key = match data {
									MaterialResourceLocation::Global(id) => match id {
										GlobalResourceIdentifier::Label(label) => buffers.key(label),
										GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
									},
									MaterialResourceLocation::Context(label) => self.buffers.get(label).cloned(),
								}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
								trace!("Adding uniform usages to buffer {:?} ({:?})", data, key);
								buffer_usages.push((key, wgpu::BufferUsages::UNIFORM));
								BindGroupEntryContentDescriptor::Buffer(key)
							},
							BindGroupEntry::UniformBuffers(ids, _) => {
								let keys = ids.iter()
									.map(|id| material.specification.mappings.get(id).ok_or(MaterialError::MaterialMappingMissingError(id.clone())))
									.collect::<Result<Vec<_>, MaterialError>>()?
									.iter()
									.map(|&data| match data {
										MaterialResourceLocation::Global(id) => match id {
											GlobalResourceIdentifier::Label(label) => buffers.key(label),
											GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
										},
										MaterialResourceLocation::Context(label) => self.buffers.get(label).cloned(),
									}.ok_or(MaterialError::MaterialResourceMissingError(data.clone())))
									.collect::<Result<Vec<_>, MaterialError>>()?;
								keys.iter().for_each(|&key| buffer_usages.push((key, wgpu::BufferUsages::UNIFORM)));
								BindGroupEntryContentDescriptor::Buffers(keys)
							},
							BindGroupEntry::UniformBufferArray(id, _, _) => {
								let ids = material.specification.array_mappings.get(id)
									.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
								let keys = ids.iter()
									.map(|data| match data {
										MaterialResourceLocation::Global(id) => match id {
											GlobalResourceIdentifier::Label(label) => buffers.key(label),
											GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
										},
										MaterialResourceLocation::Context(label) => self.buffers.get(label).cloned(),
									}.ok_or(MaterialError::MaterialResourceMissingError(data.clone())))
									.collect::<Result<Vec<_>, MaterialError>>()?;
								keys.iter().for_each(|&key| buffer_usages.push((key, wgpu::BufferUsages::UNIFORM)));
								BindGroupEntryContentDescriptor::Buffers(keys)
							},
							BindGroupEntry::Texture(id, _, _, _, _, _) => {
								let data = material.specification.mappings.get(id)
									.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
								let key = match data {
									MaterialResourceLocation::Global(id) => match id {
										GlobalResourceIdentifier::Label(label) => textures.key_by_name(label),
										GlobalResourceIdentifier::Path(path) => textures.key_by_path(path),
									},
									MaterialResourceLocation::Context(label) => self.textures.get(label).copied(),
								}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
								texture_usages.push((key, wgpu::TextureUsages::TEXTURE_BINDING));
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
								let data = material.specification.mappings.get(id)
									.ok_or(MaterialError::MaterialMappingMissingError(id.clone()))?;
								let key = match data {
									MaterialResourceLocation::Global(id) => match id {
										GlobalResourceIdentifier::Label(label) => buffers.key(label),
										GlobalResourceIdentifier::Path(_) => todo!("Buffers currently cannot be read! You must decide what format to use for this feature!"),
									},
									MaterialResourceLocation::Context(label) => self.buffers.get(label).cloned(),
								}.ok_or(MaterialError::MaterialResourceMissingError(data.clone()))?;
								trace!("Adding uniform usages to buffer {:?} ({:?})", data, key);
								buffer_usages.push((key, wgpu::BufferUsages::STORAGE));
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

				trace!("Applying usages to resources");
				for &(key, usages) in texture_usages.iter() {
					textures.add_dependent_material(key, material_key, self.key, usages);
				}
				for &(key, usages) in buffer_usages.iter() {
					buffers.add_usages(key, material_key, self.key, usages);
				}

				self.material_bindings.insert(material_key, MaterialBinding { 
					bind_groups: bindings, 
					texture_usages, 
					buffer_usages, 
					dirty: AtomicBool::new(false),
				});
			}
		}
		Ok(())
	}
}


#[derive(Debug, Default)]
pub struct RenderContextManager<T: EntityIdentifier> {
	pub render_contexts: SlotMap<RenderContextKey, RenderContext<T>>,
}
impl<T: EntityIdentifier> RenderContextManager<T> {
	pub fn new() -> Self {
		Self {
			render_contexts: SlotMap::with_key(),
		}
	}

	pub fn new_context(&mut self, name: impl Into<String>) -> (RenderContextKey, &mut RenderContext<T>) {
		if self.render_contexts.len() != 0 {
			panic!("tell me why");
		}
		let k = self.render_contexts.insert_with_key(|k| RenderContext::new(name, k));

		(k, self.render_contexts.get_mut(k).unwrap())
	}

	pub fn get(&self, key: RenderContextKey) -> Option<&RenderContext<T>> {
		self.render_contexts.get(key)
	}

	pub fn get_mut(&mut self, key: RenderContextKey) -> Option<&mut RenderContext<T>> {
		self.render_contexts.get_mut(key)
	}

	pub fn bind_materials(
		&mut self,
		materials: &MaterialManager,
		shaders: &ShaderManager,
		textures: &TextureManager,
		buffers: &BufferManager,
		bind_groups: &mut BindGroupManager,
	) -> Result<(), MaterialError> {
		for rc in self.render_contexts.values_mut() {
			rc.bind_materials(materials, shaders, textures, buffers, bind_groups)?;
		}
		Ok(())
	}


}
