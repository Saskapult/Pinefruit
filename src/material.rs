use std::{path::{PathBuf, Path}, collections::HashMap};
use serde::{Serialize, Deserialize};
use anyhow::*;
use crate::texture::*;
use generational_arena::{Arena, Index};




// Unlike Material, this has file-relative paths
#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSpecification {
	pub name: String,
	pub polygon_graph: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub floats: HashMap<String, Vec<f32>>,	// Alpha cutoff in here please
	pub sounds: HashMap<String, Vec<PathBuf>>,
}
impl MaterialSpecification {
	pub fn canonicalize(&mut self, context: &PathBuf) -> std::io::Result<()> {
		self.polygon_graph = context.join(&self.polygon_graph).canonicalize()?;

		for (_, textures) in self.textures.iter_mut() {
			for tex in textures.iter_mut() {
				*tex = context.join(&tex.clone()).canonicalize()?;
			}
		}
		for (_, sounds) in self.sounds.iter_mut() {
			for sound in sounds.iter_mut() {
				*sound = context.join(&sound.clone()).canonicalize()?;
			}
		}
		
		Ok(())
	}
}



// A material is just a collection of resources to be used by something (renderer, physics, sound)
#[derive(Debug)]
pub struct Material {
	pub name: String,
	pub polygon_graph: PathBuf,
	pub source_path: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub floats: HashMap<String, Vec<f32>>,
	pub sounds: HashMap<String, Vec<PathBuf>>, // step sounds, break sounds
}
impl Material {
	pub fn mean_albedo(&self, tm: &TextureManager) -> Option<[f32; 4]> {
		if let Some(g) = self.floats.get("mean_albedo") {
			let g = g.clone().try_into().unwrap();
			Some(g)
		} else {
			let g = self.textures.get("albedo")
				.and_then(|p| p.get(0))
				.and_then(|p| tm.index_path(p))
				.and_then(|i| tm.index(i))
				.and_then(|t| Some(t.mean_rgba()));
			g
		}
	}
}
impl std::fmt::Display for Material {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", &self.name)
	}
}



/// Material instance manager?
#[derive(Debug, Default)]
pub struct MaterialManager {
	materials: Arena<Material>,
	materials_index_name: HashMap<String, Index>,
}
impl MaterialManager {
	pub fn new() -> Self {
		Self {
			materials: Arena::new(),
			materials_index_name: HashMap::new(),
		}
	}

	pub fn insert(&mut self, material: Material) -> Index {
		info!("New material {}", &material.name);
		let name = material.name.clone();
		let idx = self.materials.insert(material);
		self.materials_index_name.insert(name, idx);
		idx
	}

	pub fn index(&self, i: Index) -> Option<&Material> {
		self.materials.get(i)
	}

	pub fn index_name(&self, name: &String) -> Option<Index> {
		self.materials_index_name.get(name).and_then(|&i| Some(i))
	}
}



/// Loads materials from a file along with their assets
pub fn load_materials_file(
	path: impl AsRef<Path>,
	tm: &mut TextureManager, 
	mm: &mut MaterialManager, 
) -> Result<()> {
	let path = path.as_ref();
	info!("Reading materials file {:?}", &path);

	let canonical_path = path.canonicalize()
		.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	let f = std::fs::File::open(&path)
		.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
	let material_specs: Vec<MaterialSpecification> = ron::de::from_reader(f)
		.with_context(|| format!("Failed to parse material ron file '{:?}'", &canonical_path))?;
	let folder_context = canonical_path.parent().unwrap();

	for material_spec in material_specs {
		let canonical_graph_path = folder_context.join(&material_spec.polygon_graph).canonicalize()
			.with_context(|| format!("Failed to canonicalize path '{:?}'", &material_spec.polygon_graph))?;
		
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
			let tex = Texture::from_path(t, &tps[0]).unwrap();
			tm.insert(tex);
		}

		let mat = Material {
			name: material_spec.name,
			polygon_graph: canonical_graph_path,
			source_path: canonical_path.clone(),
			textures,
			floats: HashMap::new(),
			sounds: HashMap::new(),
		};

		mm.insert(mat);
	}
	
	Ok(())
}
