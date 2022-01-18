use crate::render::*;
use std::collections::HashMap;



/*
Materials need to be compiled against a shader
Meshes need to be compiled against a shader
A model lets us link these two things
It's debatably bad but it's very flexible

This will probably need to be changed if we ever add bones
*/



pub struct Model {
	pub name: String,
	pub mesh_idx: usize,	// Must match material's shader vertex input
	pub material_idx: usize,
}



// Information needed to render something
pub struct ModelInstance {
	pub model_idx: usize,
	// Matches shader and all that
	pub instance_properties: Vec<InstanceProperty>,
	pub instance_properties_data: Vec<u8>,
}



pub struct ModelManager {
	models: Vec<Model>,
	index_name: HashMap<String, usize>,
}
impl ModelManager {
	pub fn new() -> Self {
		Self {
			models: Vec::new(),
			index_name: HashMap::new(),
		}
	}

	pub fn index(&self, i: usize) -> &Model {
		&self.models[i]
	}

	pub fn index_name(&self, name: &String) -> usize {
		self.index_name[name]
	}

	pub fn insert(&mut self, model: Model) -> usize {
		let idx = self.models.len();
		self.index_name.insert(model.name.clone(), idx);
		self.models.push(model);
		idx
	}
}
