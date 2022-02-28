use crate::world::*;
use noise::Perlin;
use noise::Seedable;
use noise::Worley;
use noise::NoiseFn;
use std::path::Path;
use anyhow::*;
use splines::*;




pub trait BaseGenerator {
	fn chunk_base(&self, chunk_position: [i32; 3], chunk: Chunk, bm: &BlockManager) -> Chunk;
}



pub struct TerrainGenerator {
	perlin: Perlin,
}
impl TerrainGenerator {
	pub fn new(seed: u32) -> Self {
		Self {
			perlin: Perlin::new().set_seed(seed),
		}
	}

	/// Creates a chunk base based on a heightmap
	fn chunk_base_hm(
		&self, 
		chunk_position: [i32; 3], 
		mut chunk: Chunk,
		bm: &BlockManager,
	) -> Chunk {
		let grass_idx = bm.index_name(&"grass".to_string()).unwrap();
		let dirt_idx = bm.index_name(&"dirt".to_string()).unwrap();

		for x in 0..chunk.size[0] {
			let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
			for z in 0..chunk.size[2] {
				let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;

				let noisy = crate::noise::octave_perlin_2d(
					&self.perlin, 
					[
						x_world as f64 / 25.0, 
						z_world as f64 / 25.0,
					], 
					4, 
					0.5,
					2.0,
				).powf(2.28);

				let y_level = (noisy * 16.0 - 8.0) as i32;

				for y in 0..chunk.size[1] {
					let y_world = chunk.size[1] as i32 * chunk_position[1] + y as i32;
					let voxel = {
						if y_world > y_level {
							Voxel::Empty
						} else if y_world == y_level {
							Voxel::Block(grass_idx)
						} else {
							Voxel::Block(dirt_idx)
						}
					};
					chunk.set_voxel([x as i32, y as i32, z as i32], voxel)
				}
			}
		}

		chunk
	}

	/// Creates a chunk base based on 3d noise
	fn chunk_base_3d(
		&self, 
		chunk_position: [i32; 3], 
		mut chunk: Chunk,
		bm: &BlockManager,
	) -> Chunk {
		let stone_idx = bm.index_name(&"stone".to_string()).unwrap();

		for x in 0..chunk.size[0] {
			let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
			for z in 0..chunk.size[2] {
				let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;
				for y in 0..chunk.size[1] {
					let y_world = chunk.size[1] as i32 * chunk_position[1] + y as i32;

					let density = crate::noise::octave_perlin_3d(
						&self.perlin, 
						[
							x_world as f64 / 25.0, 
							y_world as f64 / 25.0,
							z_world as f64 / 25.0,
						], 
						4, 
						0.5,
						2.0,
					);

					let squashpart = if y_world >= 0 {
						// No blocks after y=20
						if y_world > 20 {
							0.0
						} else {
							1.0 - (y_world as f64 / 20.0)
						}
					} else {
						y_world.abs() as f64 / 20.0 + 1.0
					};

					let voxel = {
						if density * squashpart >= 0.5 {
							Voxel::Block(stone_idx)
						} else {
							Voxel::Empty
						}
					};
					chunk.set_voxel([x as i32, y as i32, z as i32], voxel)
				}				
			}
		}

		chunk
	}

	/// Creates blcokmods which add grass and dirt layers to a map based on a chunk
	// This code is a little bad
	// Try caching the above/below chunk
	pub fn grassify_3d(
		&self,
		chunk_position: [i32; 3], 
		map: &Map,
		bm: &BlockManager,	// Assumed to be the same as the world's bm
	) -> BlockMods {
		let mut block_mods = BlockMods::new();

		let chunk = map.chunk(chunk_position).unwrap();

		let grass_idx = bm.index_name(&"grass".to_string()).unwrap();
		let dirt_idx = bm.index_name(&"dirt".to_string()).unwrap();

		for x in 0..chunk.size[0] as i32 {
			for z in 0..chunk.size[2] as i32 {
				for y in 0..chunk.size[1] as i32 {

					let set_to_grass = 
					if !chunk.get_voxel([x, y, z]).is_empty() {
						// Not empty
						if chunk.is_in_bounds(x, y+1, z) {
							// Above is in chunk
							chunk.get_voxel([x, y+1, z]).is_empty()
							// Above is empty
						} else {
							// Above is not in chunk
							match map.chunk([chunk_position[0], chunk_position[1]+1, chunk_position[2]]) {
								// Above is empty
								Some(c) => c.get_voxel([x, 0, z]).is_empty(),
								// Above does not exist
								None => true, 
							}
						}
					} else {
						// Empty
						false
					};	

					if set_to_grass {
						blockmods_insert(
							&mut block_mods,
							chunk_position,
							ChunkBlockMod {
								voxel_chunk_position: [x, y, z],
								reason: BlockModReason::WorldGenSet(Voxel::Block(grass_idx)),
							},
						);

						// Set below it to dirt
						for i in 1..=2 {
							
							if chunk.is_in_bounds(x, y-i, z) {
								// In bounds

								// Not empty
								let set_to_dirt= !chunk.get_voxel([x, y-i, z]).is_empty();

								if set_to_dirt {
									blockmods_insert(
										&mut block_mods,
										chunk_position,
										ChunkBlockMod {
											voxel_chunk_position: [x, y-i, z],
											reason: BlockModReason::WorldGenSet(Voxel::Block(dirt_idx)),
										},
									);
								}
							} else {
								// Below bounds
								let set_to_dirt = 
								match map.chunk([chunk_position[0], chunk_position[1]-1, chunk_position[2]]) {
									// Not empty
									Some(c) => !c.get_voxel([x, chunk.size[1] as i32 + y-i, z]).is_empty(),
									// Does not exist
									None => false,
								};

								if set_to_dirt {
									assert!(chunk.is_in_bounds(x, chunk.size[1] as i32 + y-i, z), "{}", y-i);
									blockmods_insert(
										&mut block_mods,
										[chunk_position[0], chunk_position[1]-1, chunk_position[2]],
										ChunkBlockMod {
											voxel_chunk_position: [x, chunk.size[1] as i32 + y-i, z],
											reason: BlockModReason::WorldGenSet(Voxel::Block(dirt_idx)),
										},
									);
								}
							}

							
						}
					}
					
				}
			}
		}

		block_mods
	}
}
impl BaseGenerator for TerrainGenerator {
	fn chunk_base(
		&self, 
		chunk_position: [i32; 3], 
		chunk: Chunk,
		bm: &BlockManager,
	) -> Chunk {
		self.chunk_base_3d(chunk_position, chunk, bm)
	}
}



pub trait Carver {
	fn carve_chunk(&self, chunk_position: [i32; 3], chunk: Chunk) -> Chunk;
}



pub struct WorleyCarver {
	worley: Worley,
}
impl WorleyCarver {
	pub fn new(seed: u32) -> Self {
		Self {
			worley: Worley::new().set_seed(seed),
		}
	}
}
impl Carver for WorleyCarver {
	fn carve_chunk(
		&self, 
		chunk_position: [i32; 3], 
		mut chunk: Chunk,
	) -> Chunk {
		for x in 0..chunk.size[0] {
			let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
			for z in 0..chunk.size[2] {
				let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;
				for y in 0..chunk.size[1] {
					let y_world = chunk.size[1] as i32 * chunk_position[1] + y as i32;

					let density = self.worley.get([
						x_world as f64 / 5.0, 
						y_world as f64 / 5.0,
						z_world as f64 / 5.0,
					]) / 2.0 + 0.5;

					if density >= 0.8 {
						chunk.set_voxel([x as i32, y as i32, z as i32], Voxel::Empty)
					}
				}
			}
		}

		chunk
	}
}


pub fn save_spline(
	spline: &Spline<f64, f64>, 
	path: impl AsRef<Path>,
) -> Result<()> {
	let path = path.as_ref();
	// let canonical_path = path.canonicalize()
	// 	.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	let canonical_path = path;

	let f = std::fs::File::create(&path)
		.with_context(|| format!("Failed to write file path '{:?}'", &canonical_path))?;
	ron::ser::to_writer(f, spline)
		.with_context(|| format!("Failed to parse spline ron file '{:?}'", &canonical_path))?;
	
	Ok(())
}
pub fn load_spline(path: impl AsRef<Path>) -> Result<Spline<f64, f64>> {
	let path = path.as_ref();
	let canonical_path = path.canonicalize()
		.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	let f = std::fs::File::open(&path)
		.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
	let spline: Spline<f64, f64> = ron::de::from_reader(f)
		.with_context(|| format!("Failed to parse spline ron file '{:?}'", &canonical_path))?;
	Ok(spline)
}



#[cfg(test)]
mod tests {
	use super::*;

	fn make_test_spline() -> Spline<f64, f64> {
		let k1 = Key::new(0.0, 0.0, Interpolation::Linear);
		let k2 = Key::new(1.0, 1.0, Interpolation::Linear);
		let spline = Spline::from_vec(vec![k1, k2]);
		spline
	}

    #[test]
    fn test_spline_serde() {
		let spline1 = make_test_spline();

		let spline_path = "/tmp/splinetime.ron";
		save_spline(&spline1, spline_path).unwrap();
		let spline2 = load_spline(spline_path).unwrap();

        assert_eq!(spline1.keys(), spline2.keys());
    }
}