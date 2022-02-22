use std::{path::PathBuf, collections::HashMap};
use image::DynamicImage;


#[derive(Debug, Clone)]
pub struct Texture {
	pub name: String,
	pub path: Option<PathBuf>,
	pub data: DynamicImage,
}
impl Texture {
	pub fn new(name: &String, path: impl Into<PathBuf>) -> Self {
		let path = path.into();
		Self {
			name: name.clone(),
			path: Some(path.clone()),
			data: image::open(path.clone()).expect("Failed to open file"),
		}
	}
}



#[derive(Debug)]
pub struct TextureManager {
	textures: Vec<Texture>,
	textures_index_name: HashMap<String, usize>,
	textures_index_path: HashMap<PathBuf, usize>,
}
impl TextureManager {
	pub fn new() -> Self {
		Self {
			textures: Vec::new(),
			textures_index_name: HashMap::new(),
			textures_index_path: HashMap::new(),
		}
	}

	pub fn insert(&mut self, texture: Texture) -> usize {
		info!("New texture {} ({:?})", &texture.name, &texture.path);
		let idx = self.textures.len();
		self.textures_index_name.insert(texture.name.clone(), idx);
		if let Some(path) = texture.path.clone() {
			let canonical_path = path.canonicalize().unwrap();
			self.textures_index_path.insert(canonical_path, idx);
		}
		self.textures.push(texture);
		idx
	}

	pub fn index(&self, i: usize) -> &Texture {
		&self.textures[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.textures_index_name.contains_key(name) {
			Some(self.textures_index_name[name])
		} else {
			None
		}
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<usize> {
		if self.textures_index_path.contains_key(path) {
			Some(self.textures_index_path[path])
		} else {
			None
		}
	}
}
