use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};



#[derive(Debug, Serialize, Deserialize)]
pub struct BlockSpecification {
	pub name: String,
	pub materials: HashMap<String, PathBuf>,
}



pub fn load_blocks_file(
	path: &PathBuf,
) -> Vec<BlockSpecification> {
	let f = std::fs::File::open(path).expect("Failed to open file");
	let info: Vec<BlockSpecification> = ron::de::from_reader(f).expect("Failed to parse blocks ron file");
	info
}



#[derive(Debug)]
pub struct BlockData {
	name: String,
	material_id: u32,
}



#[derive(Debug)]
pub struct BlockManager {
	pub blocks: Vec<BlockData>,
	pub index_name: HashMap<String, usize>,
}



