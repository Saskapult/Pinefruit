
use std::{
	collections::HashMap,
	path::PathBuf,
	sync::{Arc, RwLock},
};
use crate::render::*;
use serde::{Serialize, Deserialize};



// Unlike Material, this has file-relative paths
#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSpecification {
	pub name: String,
	pub shader: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub floats: HashMap<String, Vec<f32>>,
	// pub sounds: HashMap<String, Vec<PathBuf>>, // step sounds, break sounds
}



pub fn read_materials_file(
	path: &PathBuf,
) -> Vec<Material> {
	info!("Reading materials file {:?}", path);
	let f = std::fs::File::open(path).expect("Failed to open file");
	let mut info: Vec<MaterialSpecification> = ron::de::from_reader(f).expect("Failed to read materials ron file");
	let info = info.drain(..).map(|ms| Material::from_specification(ms, &path)).collect::<Vec<_>>();
	info
}



// A material is just a collection of resources to be used by something (renderer, physics, sound)
#[derive(Debug)]
pub struct Material {
	pub name: String,
	pub shader: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub floats: HashMap<String, Vec<f32>>,
	// pub sounds: HashMap<String, Vec<PathBuf>>, // step sounds, break sounds
}
impl Material {
	pub fn from_specification(specification: MaterialSpecification, source_path: &PathBuf) -> Self {
		let folder_context = source_path.parent().unwrap();
		let shader_path = folder_context.join(specification.shader);
		let textures = specification.textures.iter().map(|(name, ppaths)| {
			let paths = ppaths.iter().map(|p| folder_context.join(p)).collect::<Vec<_>>();
			(name.clone(), paths)
		}).collect::<HashMap<_,_>>();
		Material {
			name: specification.name,
			shader: shader_path,
			textures,
			floats: HashMap::new(),
		}
	}
}
impl std::fmt::Display for Material {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", &self.name)
	}
}



#[derive(Debug)]
pub struct MaterialManager {
	materials: Vec<Material>,
	materials_index_name: HashMap<String, usize>,
}
impl MaterialManager {
	pub fn new() -> Self {
		Self {
			materials: Vec::new(),
			materials_index_name: HashMap::new(),
		}
	}

	pub fn insert(&mut self, material: Material) -> usize {
		info!("New material {}", &material.name);
		let idx = self.materials.len();
		self.materials_index_name.insert(material.name.clone(), idx);
		self.materials.push(material);
		idx
	}

	pub fn index(&self, i: usize) -> &Material {
		&self.materials[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.materials_index_name.contains_key(name) {
			Some(self.materials_index_name[name])
		} else {
			None
		}
	}
}



// A material bound with a certain bind group input layout
// Note to self: bind group could hold texture array, array texture, buffer, whatever
#[derive(Debug)]
pub struct BoundMaterial {
	pub name: String,
	pub shader_idx: usize,	// The name of the shader to be used
	pub bind_group_format: BindGroupFormat,
	pub bind_group: wgpu::BindGroup,
}
impl std::fmt::Display for BoundMaterial {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "BoundMaterial {} '{}'", &self.name, &self.bind_group_format)
	}
}


// A compiled material is based on a material and shader bind group (layout bit)
// The material can be identified using its name
// The shader bind group can be identified throuch comparison
// Therefore a compiled material can be identified by a material name and a shader bind group
#[derive(Debug)]
pub struct BoundMaterialManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	materials: Vec<BoundMaterial>,
	materials_index_name_format: HashMap<(String, BindGroupFormat), usize>,
	material_manager: Arc<RwLock<MaterialManager>>,
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
			materials: Vec::new(),
			materials_index_name_format: HashMap::new(), 
			material_manager: material_manager.clone(),
		}
	}

	pub fn insert(&mut self, bound_material: BoundMaterial) -> usize {
		let idx = self.materials.len();
		self.materials_index_name_format.insert((bound_material.name.clone(), bound_material.bind_group_format.clone()), idx);
		self.materials.push(bound_material);
		idx
	}

	pub fn index(&self, i: usize) -> &BoundMaterial {
		&self.materials[i]
	}

	pub fn index_name_format(&self, name: &String, format: &BindGroupFormat) -> Option<usize> {
		let key = (name.clone(), format.clone());
		if self.materials_index_name_format.contains_key(&key) {
			Some(self.materials_index_name_format[&key])
		} else {
			None
		}
	}

	pub fn index_name_format_bind(&mut self, _name: &String, _format: &BindGroupFormat) -> Option<usize> {
		todo!();
	}

	// Attempts to compile a material to be accepted into a bind group
	pub fn bind_material(
		&self,
		material: &Material,
		shaders: &mut ShaderManager,
		textures: &mut BoundTextureManager,
	) -> BoundMaterial {
		info!("Binding material '{}'", &material);
		// Find/load the shader
		let shader_idx = match shaders.index_path(&material.shader) {
			Some(index) => index,
			None => shaders.register_path(&material.shader),
		};
		let shader = shaders.index(shader_idx);
		let bind_group_format = shader.bind_groups[&1].format.clone();

		// Collect resource info
		let mut texture_view_index_collections = Vec::new();
		let mut samplers = Vec::new();
		let mut binding_templates = Vec::new(); // (type, binding position, index)
		for binding in &bind_group_format.binding_specifications {
			let j = binding.layout.binding;
			match binding.binding_type {
				BindingType::Texture => {
					let texture_usage = &binding.resource_usage;
					// If there is no such resource then panic or something
					if material.textures.contains_key(texture_usage) {
						let texture_path = &material.textures[texture_usage][0];
						let texture_idx = textures.index_path_bind(texture_path).expect("Missing texture data!");
						binding_templates.push((BindingType::Texture, j as u32, texture_idx));
					} else {
						panic!("This material is missing a field for its shader")
					}
				},
				BindingType::TextureArray => {
					let texture_usage = &binding.resource_usage;
					if material.textures.contains_key(texture_usage) {
						let texture_paths = &material.textures[texture_usage];
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
						binding_templates.push((BindingType::TextureArray, j as u32, tvi_idx));
					} else {
						panic!("This material is missing a field for its shader")
					}
				},
				BindingType::Sampler => {
					// Todo: Let the material specify its samplers
					let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor::default());
					let i = samplers.len();
					samplers.push(sampler);

					binding_templates.push((BindingType::Sampler, j as u32, i));
				},
				BindingType::ArrayTexture => {
					// Make/get array texture
					todo!("Array texture not done please forgive");
				},
				_ => panic!("This shader binding type is not (yet?) supported!"),
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
				BindingType::ArrayTexture => {
					todo!("Array texture still not done please forgive again");
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
				_ => panic!("how did you reach this?"),
			}
		}
		
		let name = format!("{} with format {}", &material.name, &bind_group_format);

		let layout = match shaders.bind_group_layout_index_bind_group_format(&bind_group_format) {
			Some(bgli) => shaders.bind_group_layout_index(bgli),
			None => {
				let idx = shaders.bind_group_layout_create(&bind_group_format);
				shaders.bind_group_layout_index(idx)
			}
		};

		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &bindings[..],
			layout,
			label: Some(&*format!("bind group of {}", &name)),
		});

		BoundMaterial {
			name, shader_idx, bind_group_format, bind_group,
		}
	}
}



#[cfg(test)]
mod tests {
	use super::*;

	fn create_example_material() -> MaterialSpecification {
		let mut textures = HashMap::new();
		let albedo = ["g.png", "f.png"].iter().map(|s| PathBuf::from(&s)).collect::<Vec<_>>();
		textures.insert("albedo".to_string(), albedo);
		MaterialSpecification {
			name: "example material".into(),
			shader: "exap_shader.ron".into(),
			floats: HashMap::new(),
			textures,
		}
	}

	#[test]
	fn test_serialize() {
		let data = vec![create_example_material()];
		let pretty = ron::ser::PrettyConfig::new()
			.depth_limit(3)
			.separate_tuple_members(true)
			.enumerate_arrays(false);
		let s = ron::ser::to_string_pretty(&data, pretty).expect("Serialization failed");
		println!("{}", s);
		assert!(true);
	}
}
