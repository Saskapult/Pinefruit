use std::{
	collections::HashMap,
	path::PathBuf,
	sync::{Arc, RwLock},
};
use crate::render::*;
use crate::material::*;




/// A material bound to a bind group format
#[derive(Debug)]
pub struct BoundMaterial {
	pub name: String,
	pub graph: PathBuf,
	pub material_idx: usize,
	pub bind_group_format: BindGroupFormat,
	pub bind_group: wgpu::BindGroup,
}
impl std::fmt::Display for BoundMaterial {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "BoundMaterial {} '{}'", &self.name, &self.bind_group_format)
	}
}



// A compiled material is based on a material and bind group format
// The material can be identified using its name
// The shader bind group can be identified throuch comparison
// A compiled material can be identified by material name and bind group format
#[derive(Debug)]
pub struct BoundMaterialManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	bound_materials: Vec<BoundMaterial>,
	materials_index_from_index_format: HashMap<(usize, BindGroupFormat), usize>,
	materials_index_from_name_format: HashMap<(String, BindGroupFormat), usize>,
	pub material_manager: Arc<RwLock<MaterialManager>>,
}
impl BoundMaterialManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		material_manager: &Arc<RwLock<MaterialManager>>,
	) -> Self {
		Self { 
			device: device.clone(), 
			queue: queue.clone(), 
			bound_materials: Vec::new(),
			materials_index_from_index_format: HashMap::new(), 
			materials_index_from_name_format: HashMap::new(), 
			material_manager: material_manager.clone(),
		}
	}

	pub fn insert(&mut self, bound_material: BoundMaterial) -> usize {
		let idx = self.bound_materials.len();
		let index_key = (bound_material.material_idx, bound_material.bind_group_format.clone());
		self.materials_index_from_index_format.insert(index_key, idx);
		let name_key = (bound_material.name.clone(), bound_material.bind_group_format.clone());
		self.materials_index_from_name_format.insert(name_key, idx);
		self.bound_materials.push(bound_material);
		idx
	}

	pub fn index(&self, i: usize) -> &BoundMaterial {
		&self.bound_materials[i]
	}

	pub fn index_from_name_format(&self, name: &String, format: &BindGroupFormat) -> Option<usize> {
		let key = (name.clone(), format.clone());
		if self.materials_index_from_name_format.contains_key(&key) {
			Some(self.materials_index_from_name_format[&key])
		} else {
			None
		}
	}

	pub fn index_from_index_format(&self, material_idx: usize, format: &BindGroupFormat) -> Option<usize> {
		let key = (material_idx, format.clone());
		if self.materials_index_from_index_format.contains_key(&key) {
			Some(self.materials_index_from_index_format[&key])
		} else {
			None
		}
	}

	pub fn index_from_index_format_bind(
		&mut self, 
		material_idx: usize, 
		format: &BindGroupFormat,
		shaders: &mut ShaderManager,
		textures: &mut BoundTextureManager,
	) -> usize {
		let key = (material_idx, format.clone());
		if self.materials_index_from_index_format.contains_key(&key) {
			self.materials_index_from_index_format[&key]
		} else {
			self.bind_by_index(material_idx, format, shaders, textures)
		}
	}

	/// Attempts to bind a material to be accepted into a bind group format
	pub fn bind_by_index(
		&mut self,
		material_idx: usize,
		bind_group_format: &BindGroupFormat,
		shaders: &mut ShaderManager,
		textures: &mut BoundTextureManager,
	) -> usize {
		let mm = self.material_manager.read().unwrap();
		let material = mm.index(material_idx);

		info!("Binding material '{}' with format '{}'", material, bind_group_format);

		// Collect resource info
		let mut texture_view_index_collections = Vec::new();
		let mut samplers = Vec::new();
		let mut binding_templates = Vec::new(); // (type, binding position, index)
		for (i, bind_group_entry_format) in &bind_group_format.entry_formats {
			let resource_usage = &bind_group_entry_format.resource_usage;
			match bind_group_entry_format.binding_type {
				BindingType::Texture => {
					if material.textures.contains_key(resource_usage) {
						let texture_path = &material.textures[resource_usage][0];
						let texture_idx = textures.index_path_bind(texture_path).expect("Missing texture data!");
						binding_templates.push((BindingType::Texture, *i, texture_idx));
					} else {
						panic!("This material is missing a field for this format!")
					}
				},
				BindingType::TextureArray => {
					if material.textures.contains_key(resource_usage) {
						let texture_paths = &material.textures[resource_usage];
						// Collect indices
						let mut texture_indices = Vec::new();
						for texture_path in texture_paths {
							let idx = textures.index_path_bind(texture_path).expect("Missing texture data!");
							texture_indices.push(idx);
						}
						// A texture array is built from a slice of memory containing references to texture views
						// Pushing to a vec might cause the contents to be reallocated
						// Any existing slices would become invalid when this occurs
						// In solution we defer slice access until after all texture array data has been allocated
						let tvi_idx = texture_view_index_collections.len();
						texture_view_index_collections.push(texture_indices);
						binding_templates.push((BindingType::TextureArray, *i, tvi_idx));
					} else {
						panic!("This material is missing a field for this format!")
					}
				},
				BindingType::Sampler => {
					// Todo: Let the material specify its samplers
					let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor::default());
					let j = samplers.len();
					samplers.push(sampler);

					binding_templates.push((BindingType::Sampler, *i, j));
				},
				_ => todo!("This material binding type is not (yet?) supported!"),
			}
		}

		// Affore mentioned texture array shenanigans
		let mut texture_view_collections = Vec::new();
		for index_collection in texture_view_index_collections {
			let mut texture_views = Vec::new();
			for i in index_collection {
				let view = &textures.index(i).view;
				texture_views.push(view);
			}
			texture_view_collections.push(texture_views);
		}
		
		// Create the bind group from now-created resources
		let mut bindings = Vec::new(); // If empty then no material data was used
		for (binding_type, position, ridx) in binding_templates {
			match binding_type {
				BindingType::Texture => {
					let texture_view = &textures.index(ridx).view;
					bindings.push(wgpu::BindGroupEntry {
						binding: position,
						resource: wgpu::BindingResource::TextureView(texture_view),
					});
				},
				BindingType::TextureArray => {
					bindings.push(wgpu::BindGroupEntry {
						binding: position,
						resource: wgpu::BindingResource::TextureViewArray(&texture_view_collections[ridx][..]),
					});
				},
				BindingType::Sampler => {
					let sr = &samplers[ridx];
					bindings.push(wgpu::BindGroupEntry {
						binding: position,
						resource: wgpu::BindingResource::Sampler(&sr),
					});
				},
				_ => todo!("how did you reach this?"),
			}
		}
		
		let layout = match shaders.bind_group_layout_index_from_bind_group_format(&bind_group_format) {
			Some(bgli) => shaders.bind_group_layout_index(bgli),
			None => {
				let idx = shaders.bind_group_layout_create(&bind_group_format);
				shaders.bind_group_layout_index(idx)
			}
		};

		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &bindings[..],
			layout,
			label: Some(&*format!("bind group of {}", &material.name)),
		});

		let bound_material = BoundMaterial {
			name: material.name.clone(),
			graph: material.graph.clone(),
			material_idx,
			bind_group_format: bind_group_format.clone(),
			bind_group,
		};

		drop(mm);
		self.insert(bound_material)
	}
}
