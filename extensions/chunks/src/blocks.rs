use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use krender::prelude::*;
use krender::MaterialKey;
use parking_lot::RwLock;
use render::MaterialResource;
use serde::{Serialize, Deserialize};
use slotmap::SlotMap;
use slotmap::new_key_type;
use eeks::prelude::*;



#[derive(Debug, Resource, Default)]
pub struct BlockResource {
	pub blocks: Arc<RwLock<BlockManager>>,
}
impl std::ops::Deref for BlockResource {
	type Target = Arc<RwLock<BlockManager>>;
	fn deref(&self) -> &Self::Target {
		&self.blocks
	}
}
impl std::ops::DerefMut for BlockResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.blocks
	}
}


new_key_type! {
	pub struct BlockKey;
}


#[derive(Debug, Serialize, Deserialize)]
pub struct BlockSpecification {
	pub name: String,
	// pub script: PathBuf, 
	// wasm module?
	// Should be able to take and allocate data
	// A chunk can store Option<BlockKey> and also a vec of instance data by position
	// It just needs some way to allocate (or not allocate) that data
	
	pub render_type: BlockSpecificationRenderType,
	
	pub floats: HashMap<String, Vec<f32>>,
	pub sounds: HashMap<String, PathBuf>,
}
impl BlockSpecification {
	pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		let path = path.as_ref().canonicalize()?;
		let f = std::fs::read(&path)?;
		let mut specification: BlockSpecification = ron::de::from_bytes(f.as_slice())?;
		specification.canonicalize(path.parent().unwrap())?;
		Ok(specification)
	}

	pub fn read_many(path: impl AsRef<Path>) -> anyhow::Result<Vec<Self>> {
		let path = path.as_ref();
		let contents = std::fs::read(&path)?;
		let mut specifications: Vec<BlockSpecification> = ron::de::from_bytes(contents.as_slice())?;
		let context = path.parent().unwrap();
		for s in specifications.iter_mut() {
			s.canonicalize(&context)?;
		};
		Ok(specifications)
	}

	pub fn canonicalize(&mut self, context: impl AsRef<Path>) -> anyhow::Result<()> {
		let context = context.as_ref();
		for sound in self.sounds.values_mut() {
			*sound = context.join(&sound).canonicalize()?;
		}

		match &mut self.render_type {
			BlockSpecificationRenderType::Colour => {},
			BlockSpecificationRenderType::Cube { xp, xn, yp, yn, zp, zn } => {
				xp.canonicalize(context)?;
				xn.canonicalize(context)?;
				yp.canonicalize(context)?;
				yn.canonicalize(context)?;
				zp.canonicalize(context)?;
				zn.canonicalize(context)?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PathOrLabel {
	Path(PathBuf),
	Label(String),
}
impl PathOrLabel {
	pub fn canonicalize(&mut self, context: impl AsRef<Path>) -> anyhow::Result<()> {
		match self {
			Self::Label(_) => {},
			Self::Path(p) => {
				*p = context.as_ref().join(p.clone()).canonicalize()?;
			}
		}
		Ok(())
	}
}


#[derive(Debug, Serialize, Deserialize)]
pub enum BlockSpecificationRenderType {
	Colour,
	Cube {
		xp: PathOrLabel,
		xn: PathOrLabel,
		yp: PathOrLabel,
		yn: PathOrLabel,
		zp: PathOrLabel,
		zn: PathOrLabel,
	}, // path or name
	// ScriptedCube,
	// Model,
	// ScriptedModel,
}


#[derive(Debug)]
pub enum BlockRenderType {
	// make sure floats hs colour value? Or else hash the name? 
	Colour, 
	// follows octree xp yp zp ordering
	// This does need acces to the material manager in order to pull keys
	// Maybe it should be Option<[MaterialKey; 6]> and then we call a thing to pull unpulled keys
	Cube([MaterialKey; 6]), 
	// ScriptedCube,
	// Model,
	// ScriptedModel,
}


#[derive(Debug)]
pub struct BlockEntry {
	pub specification: BlockSpecification,
	pub path: PathBuf,
	// This could have been Option<BlockRenderType> for more parallelization
	// I very much doubt, however, that it's worth the effort
	pub render_type: BlockRenderType, 
	pub covering: bool, // transparent or is model
}
impl BlockEntry {
	pub fn read(path: impl AsRef<Path>, materials: &mut MaterialManager) -> anyhow::Result<Self> {
		let path = path.as_ref();
		let specification = BlockSpecification::read(path)?;

		Self::from_specification(specification, path, materials)
	}

	pub fn from_specification(
		specification: BlockSpecification, 
		path: impl AsRef<Path>,
		materials: &mut MaterialManager,
	) -> anyhow::Result<Self> {

		fn get_thing(pol: &PathOrLabel, materials: &mut MaterialManager) -> MaterialKey {
			match pol {
				PathOrLabel::Label(l) => materials.key_by_name(l).unwrap(),
				PathOrLabel::Path(p) => materials.key_by_path(p).unwrap_or_else(|| materials.read(p)),
			}
		}

		let render_type = match &specification.render_type {
			BlockSpecificationRenderType::Colour => BlockRenderType::Colour,
			BlockSpecificationRenderType::Cube { xp, xn, yp, yn, zp, zn } => BlockRenderType::Cube([
				get_thing(xp, materials),
				get_thing(xn, materials),
				get_thing(yp, materials),
				get_thing(yn, materials),
				get_thing(zp, materials),
				get_thing(zn, materials),
			]),
		};

		Ok(Self {
			specification,
			path: path.as_ref().canonicalize()?,
			render_type,
			covering: true,
		})
	}

	// Reads colour or hashes name
	pub fn colour(&self) -> [f32; 4] {
		self.specification.floats.get("colour")
			.and_then(|v| {
				let c: Result<[f32; 4], _> = v.as_slice().try_into();
				c.ok()
			})
			.unwrap_or_else(|| {
				let mut h = DefaultHasher::new();
				self.specification.name.hash(&mut h);
				let v = h.finish();
				let r = ((v & 0xFFFF000000000000) >> 48) as f32 / u16::MAX as f32;
				let g = ((v & 0x0000FFFF00000000) >> 32) as f32 / u16::MAX as f32;
				let b = ((v & 0x00000000FFFF0000) >> 16) as f32 / u16::MAX as f32;
				let _ = ((v & 0x000000000000FFFF) >> 0) as f32 / u16::MAX as f32;
				[r, g, b, 1.0]
			})
	}
}


#[derive(Debug, Default)]
pub struct BlockManager {
	// Might want a rwlock so we can put this in an Arc and give to meshing threads
	pub blocks: SlotMap<BlockKey, BlockEntry>,
	key_by_name: HashMap<String, BlockKey>,
}
impl BlockManager {
	pub fn new() -> Self {
		Self {
			blocks: SlotMap::with_key(),
			key_by_name: HashMap::new(),
		}
	}

	pub fn insert(&mut self, block: BlockEntry) -> BlockKey {
		self.blocks.insert_with_key(|key| {
			self.key_by_name.insert(block.specification.name.clone(), key);
			block
		})
	}

	pub fn get(&self, key: BlockKey) -> Option<&BlockEntry> {
		self.blocks.get(key)
	}

	pub fn key_by_name(&self, name: &String) -> Option<BlockKey> {
		self.key_by_name.get(name).copied()
	}

	// /// Creates an encoding map for a run-length encoding.
	// /// 
	// /// encoding id -> unique index (for encoding).
	// /// Empty is not included
	// /// 
	// /// unique index -> block name (for decoding)
	// pub fn encoding_maps(&self, rle: &Vec<(usize, u32)>) -> (HashMap<usize, usize>, Vec<String>) {
		
	// 	// Find unique encoding ids which are not zero
	// 	let mut uniques = rle.iter().filter_map(|&(e_id, _)| {
	// 		if e_id > 0 {
	// 			Some(e_id)
	// 		} else {
	// 			None
	// 		}
	// 	}).collect::<Vec<_>>();
	// 	uniques.sort();
	// 	uniques.dedup();

	// 	// Create a mapping to find their index in this sorted list
	// 	// encoding id -> unique index
	// 	let uidx_map = uniques.iter().enumerate().map(|(uidx, &e_id)| {
	// 		(e_id, uidx)
	// 	}).collect::<HashMap<_,_>>();

	// 	// Map each unique non-zero encoding id to its block name
	// 	// unique index -> block name
	// 	let name_map = uniques.iter().map(|&e_id| {
	// 		self.blocks[e_id-1].name.clone()
	// 	}).collect::<Vec<_>>();

	// 	(uidx_map, name_map)
	// }
}


pub fn load_all_blocks_in_directory(
	blocks: &mut BlockManager,
	directory: impl AsRef<Path>,
	materials: &mut MaterialManager,
) -> anyhow::Result<()> {
	let directory = directory.as_ref().canonicalize()?;
	assert!(directory.is_dir());

	let files = directory
		.read_dir()?
		.map(|f| f.unwrap().path())
		.filter(|p| p.extension() == Some(OsStr::new("ron")));

	for file in files {
		blocks.insert(BlockEntry::read(file, materials)?);
	}

	Ok(())
}


pub fn load_all_blocks_in_file(
	blocks: &mut BlockManager,
	file: impl AsRef<Path>,
	materials: &mut MaterialManager,
) -> anyhow::Result<()> {
	let path = file.as_ref();
	let specifications = BlockSpecification::read_many(path)?;

	for specification in specifications {
		blocks.insert(BlockEntry::from_specification(specification, path, materials)?);
	}

	Ok(())
}

pub fn load_blocks(
	br: Res<BlockResource>,
	mut materials: ResMut<MaterialResource>,
) {
	info!("Loading blocks from file");
	let mut blocks = br.write();
	load_all_blocks_in_file(&mut blocks, "resources/kblocks.ron", &mut materials).unwrap();
}
