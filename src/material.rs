use std::{path::PathBuf, collections::HashMap};
use serde::{Serialize, Deserialize};
use anyhow::*;
use crate::texture::*;




// Unlike Material, this has file-relative paths
#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSpecification {
	pub name: String,
	pub graph: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub floats: HashMap<String, Vec<f32>>,	// Alpha cutoff in here or elsewhere?
	pub sounds: HashMap<String, Vec<PathBuf>>,
}



// A material is just a collection of resources to be used by something (renderer, physics, sound)
#[derive(Debug)]
pub struct Material {
	pub name: String,
	pub graph: PathBuf,
	pub source_path: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub floats: HashMap<String, Vec<f32>>,
	pub sounds: HashMap<String, Vec<PathBuf>>, // step sounds, break sounds
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



/// Loads materials from a file along with their assets
pub fn load_materials_file(
	path: PathBuf,
	tm: &mut TextureManager, 
	mm: &mut MaterialManager, 
) -> Result<()> {
	info!("Reading materials file {:?}", &path);

	let canonical_path = path.canonicalize()
		.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	let f = std::fs::File::open(&path)
		.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
	let material_specs: Vec<MaterialSpecification> = ron::de::from_reader(f)
		.with_context(|| format!("Failed to parse material ron file '{:?}'", &canonical_path))?;
	let folder_context = canonical_path.parent().unwrap();

	for material_spec in material_specs {
		let canonical_graph_path = folder_context.join(&material_spec.graph).canonicalize()
			.with_context(|| format!("Failed to canonicalize path '{:?}'", &material_spec.graph))?;
		
		// For each texture entry in material
		let mut textures = HashMap::new();
		for (entry_name, entry_textures) in material_spec.textures {
			// For each texture in the texture entry
			let mut canonical_texture_paths = Vec::new();
			for texture_path in entry_textures {
				let canonical_texture_path = folder_context.join(&texture_path).canonicalize()
					.with_context(|| format!("Failed to canonicalize path '{:?}'", &texture_path))?;
				canonical_texture_paths.push(canonical_texture_path);
			}
			textures.insert(entry_name, canonical_texture_paths);
		}

		// Load textures
		for (t, tps) in &textures {
			let tex = Texture::new(t, &tps[0]);
			tm.insert(tex);
		}

		let mat = Material {
			name: material_spec.name,
			graph: canonical_graph_path,
			source_path: canonical_path.clone(),
			textures,
			floats: HashMap::new(),
			sounds: HashMap::new(),
		};

		mm.insert(mat);
	}
	
	Ok(())
}
