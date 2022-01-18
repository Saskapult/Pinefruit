use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};



#[derive(Debug, Serialize, Deserialize)]
pub struct BlockSpecification {
	pub name: String,
	pub materials: HashMap<String, PathBuf>,
}



pub fn read_blocks_file(
	path: &PathBuf,
) -> Vec<BlockSpecification> {
	info!("Reading blocks file {:?}", path);
	let f = std::fs::File::open(path).expect("Failed to open file");
	let info: Vec<BlockSpecification> = ron::de::from_reader(f).expect("Failed to parse blocks ron file");
	info
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



