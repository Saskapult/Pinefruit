use std::{path::PathBuf, collections::HashMap};
use image::DynamicImage;
use generational_arena::{Arena, Index};
use crate::util::MBPTCT;



#[derive(Debug, Clone)]
pub struct Texture {
	pub name: String,
	pub path: Option<PathBuf>,
	pub data: DynamicImage,
}
impl Texture {
	pub fn from_path(name: impl Into<String>, path: impl Into<PathBuf>) -> anyhow::Result<Self> {
		let path = path.into();
		Ok(Self {
			name: name.into(),
			path: Some(path.clone()),
			data: image::open(path.clone())?,
		})
	}

	// Idk if this is better, see corresponding test case
	pub fn from_path_rayon(name: impl Into<String>, path: impl Into<PathBuf>) -> MBPTCT<anyhow::Result<Self>> {
		let path: PathBuf = path.into();
		let name: String = name.into();
		let (mbptct, s) = MBPTCT::new();
		rayon::spawn(move || {
			s.send(Self::from_path(name, path)).unwrap();
		});

		mbptct
	}

	pub fn mean_rgba(&self) -> [f32; 4] {
		let mut r = 0.0;
		let mut g = 0.0;
		let mut b = 0.0;
		let mut a = 0.0;
		let raw = self.data.to_rgba32f().into_raw();
		raw.chunks_exact(4)
			.for_each(|p| {
				r += p[0];
				g += p[1];
				b += p[2];
				a += p[3];
			});
		
		[r, g, b, a].map(|v| v / (raw.len() / 4) as f32)
	}
}



#[derive(Debug, Default)]
pub struct TextureManager {
	textures: Arena<Texture>,
	textures_index_name: HashMap<String, Index>,
	textures_index_path: HashMap<PathBuf, Index>,
}
impl TextureManager {
	pub fn new() -> Self {
		Self {
			textures: Arena::new(),
			textures_index_name: HashMap::new(),
			textures_index_path: HashMap::new(),
		}
	}

	pub fn insert(&mut self, texture: Texture) -> Index {
		info!("New texture {} ({:?})", &texture.name, &texture.path);
		let name = texture.name.clone();
		let path = texture.path.clone();

		let idx = self.textures.insert(texture);
		self.textures_index_name.insert(name, idx);
		if let Some(path) = path {
			let canonical_path = path.canonicalize().unwrap();
			self.textures_index_path.insert(canonical_path, idx);
		}
		idx
	}

	pub fn index(&self, index: Index) -> Option<&Texture> {
		self.textures.get(index)
	}

	pub fn index_name(&self, name: &String) -> Option<Index> {
		self.textures_index_name.get(name).and_then(|&i| Some(i))
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<Index> {
		self.textures_index_path.get(path).and_then(|&i| Some(i))
	}
}



#[cfg(test)]
mod tests {
	use super::*;
	use rayon::prelude::*;
	use std::ffi::OsStr;
	use std::time::Instant;

	// I don't trust these results because I don't know if file loading can cache
	#[test]
	fn compare_texture_load_times() {
		let directory = "resources/not_for_git/InventivetalentDev/assets/minecraft/textures/blocks";

		let valid_extensions = ["png", "jpg"]
			.iter().map(|&e| OsStr::new(e)).collect::<Vec<_>>();

		let files = std::fs::read_dir(directory).unwrap()
			.filter_map(|e| e.ok())
			.filter_map(|e| {
				let p = e.path();
				if p.is_file() {
					Some(p)
				} else {
					None
				}
			})
			.filter_map(|p| {
				if let Some(e) = p.extension() {
					if valid_extensions.contains(&e) {
						return Some(p);
					}
				}
				None
			})
			.collect::<Vec<_>>();
		// println!("Files = {files:?}");
		
		let seq_st = Instant::now();
		let _textures = files.iter()
			.map(|p| Texture::from_path("g", p))
			.collect::<Result<Vec<_>,_>>()
			.unwrap();
		let seq_dur = seq_st.elapsed();
		println!(
			"Sequential duration {}ms", 
			seq_dur.as_millis(),
		);

		let par_st = Instant::now();
		let _textures = files.par_iter()
			.map(|p| Texture::from_path("g", p))
			.collect::<Result<Vec<_>,_>>()
			.unwrap();
		let par_dur = par_st.elapsed();
		println!(
			"Parallel duration {}ms ({:.2}%)", 
			par_dur.as_millis(),
			100.0 * par_dur.as_secs_f32() / seq_dur.as_secs_f32(),
		);

		let par2_st = Instant::now();
		let mut textures = files.par_iter()
			.map(|p| Texture::from_path_rayon("g", p))
			.collect::<Vec<_>>();
		loop {
			let done = textures.iter_mut().all(|t| t.poll().is_some());
			if done {
				break
			}
		}
		let par2_dur = par2_st.elapsed();
		println!(
			"Parallel2 duration {}ms ({:.2}%)", 
			par2_dur.as_millis(),
			100.0 * par2_dur.as_secs_f32() / seq_dur.as_secs_f32(),
		);
	}
}
