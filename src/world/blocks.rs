use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use anyhow::{Result, Context};
use crate::render::*;



#[derive(Debug, Serialize, Deserialize)]
pub struct BlockSpecification {
	pub name: String,
	pub materials: HashMap<String, PathBuf>,
}



pub fn load_blocks_file(
	path: &PathBuf,
	bm: &mut BlockManager,
	tm: &mut TextureManager, 
	mm: &mut MaterialManager, 
) -> Result<()> {
	info!("Reading blocks file {:?}", path);

	let canonical_path = path.canonicalize()
		.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	let f = std::fs::File::open(path)
		.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
	let mut material_specs: Vec<MaterialSpecification> = ron::de::from_reader(f)
		.with_context(|| format!("Failed to parse blocks ron file '{:?}'", &canonical_path))?;
	let folder_context = canonical_path.parent().unwrap();

	todo!();
}



#[derive(Debug)]
pub struct Block {
	pub name: String,
	pub material_idx: u32,	// For now block faces share the same material
}



#[derive(Debug)]
pub struct BlockManager {
	blocks: Vec<Block>,
	index_name: HashMap<String, usize>,
}
impl BlockManager {
	pub fn new() -> Self {
		Self {
			blocks: Vec::new(),
			index_name: HashMap::new(),
		}
	}

	pub fn insert(&mut self, block: Block) -> usize {
		let idx = self.blocks.len();
		self.index_name.insert(block.name.clone(), idx);
		self.blocks.push(block);
		idx
	}

	pub fn insert_specification(&mut self, bspec: BlockSpecification) -> usize {
		let block = Block {
			name: bspec.name,
			material_idx: 0,		// Todo: this
		};
		self.insert(block)
	}

	pub fn index(&self, i: usize) -> &Block {
		&self.blocks[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.index_name.contains_key(name) {
			Some(self.index_name[name])
		} else {
			None
		}
	}
}



