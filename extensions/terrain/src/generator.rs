use std::{path::Path, sync::Arc};
use chunks::{blocks::{BlockKey, BlockManager}, chunks::ChunkKey, cube_iterator_xyz_uvec, CHUNK_SIZE};
use glam::{IVec2, IVec3, UVec2, UVec3};
use pinecore::pollthread::PollThread;
use simdnoise::FbmSettings;
use slotmap::SecondaryMap;
use splines::Spline;
use thiserror::Error;

use crate::{modification::VoxelModification, noise_lerp3::fbm_scaled_linear, terrain::TerrainContents};




#[derive(Error, Debug)]
pub enum GenerationError {
	#[error("failed to find block entry for '{0}'")]
	BlockNotFoundError(String),
}


fn load_spline(path: impl AsRef<Path>) -> anyhow::Result<Spline<f32, f32>> {
	let p = path.as_ref();
	let b = std::fs::read(p)?;
	let s = ron::de::from_bytes(b.as_slice())?;
	Ok(s)
}


/// This structure is used because [simdnoise::FbmSettings] does not implment [std::fmt::Debug] and also stores volume data. 
#[derive(Debug, Clone, Copy)]
pub struct RawFbmSettings {
	pub seed: i32,
	pub freq: f32,
    pub lacunarity: f32,
    pub gain: f32,
    pub octaves: u8,
}
impl RawFbmSettings {
	/// Multiply by this to map the noise to [-1.0, 1.0]
	pub fn compute_scale(&self) -> f32 {
		// Magic number derived from tests, is the analytical maximum output of one-octave noise
		let mut amp = 0.027125815;
		let mut scale = amp;
		for _ in 1..self.octaves {
			amp *= self.gain;
			scale += amp
		}
		1.0 / scale
	}
}
pub trait ConfigureRawFbm {
	fn apply_raw_settings(&mut self, settings: RawFbmSettings) -> &mut Self;
}
impl ConfigureRawFbm for FbmSettings {
	fn apply_raw_settings(&mut self, settings: RawFbmSettings) -> &mut Self {
		self
			.with_freq(settings.freq)
			.with_gain(settings.gain)
			.with_lacunarity(settings.lacunarity)
			.with_octaves(settings.octaves)
			.with_seed(settings.seed)
	}
}


#[derive(Debug)]
enum ThisOrThat<A, B> {
	This(A),
	That(B),
}


#[derive(Debug)]
enum TerrainGeneratorChunkStatus {
	// This job generates desnity data 
	// It might also need to make biome data 
	// Then it decides what's solid or not (and discards the density data)
	Shaping(PollThread<TerrainContents>),
	CoverWait, // Waits for the chunk above it
	Covering(PollThread<TerrainContents>),
	DecorationWait(Vec<IVec3>), // Waits for covering of chunks which affect potential stuctures
	Decorating(PollThread<ThisOrThat<TerrainContents, Vec<IVec3>>>), // Can halt and revert to wait
}

// Want castle here 
// Must flatten terrain 
// Oh wait maybe jsut place castles on flat areas (high flat -> scan area for max)
// Adjust height for all near it? 
// Prevent tree genration? 
// Maybe decorations have added influence over the world 
// But how could they blend without density access? 


// So it's always polling the next 
// And the next is variable 
// Next is A becuase of spiral iterator (up from bottom pls)
// A is shaped but not covered, so we should do b 
// Then The adjacents for decoration? 
// Basically we just do whatever this chunk is waiting for 
// Or maybe we jsut follow the spiral iterator until something is not blocked


// We want terrain generation passes 
// Covering replaces voxels (and depends on the above chunk)
// Carving... idk
// Flooding adds liquid voxels 
// Decoration adds trees (and depends on many other chunks)
// Finally it is ready for simulation 
// 
// Maybe things just produce modifications? Run in parallel 

// Issue: Want trees not intersect rock
// Solution: Know solidity of other chunk 
// Also after carving so no float
// Also after covering so not on stone 


#[derive(Debug)]
pub struct TerrainGenerator {
	// Some will be done (check the map)
	// Others will be in progress
	// Oh fuck these are interdependent 
	// We will need to look at other non-done entries
	// Fuck!!
	// Expand out before adding to the simulation 
	// Data will be either here or elsewhere 
	// Check world first and then check this? 
	// Ughh
	// stati: SecondaryMap<ChunkKey, TerrainGeneratorChunkStatus>,
	// Shared with the jobs
	settings: Arc<TerrainGeneratorSettings>,
}
impl TerrainGenerator {
	pub fn new(seed: u32) -> Self {
		Self {
			// stati: SecondaryMap::new(),
			settings: Arc::new(TerrainGeneratorSettings::new(seed))
		}
	}

	// I've chosen to do this entirely independently of other chunks
	// Benefits: No interdependence
	// Drawbacks: future plans, more noise generation
	// Rationale: We're never gonna get there anyway
	// Should return block mods and not bool but whatevs
	pub fn generate_this(&self, chunk_position: IVec3, blocks: &BlockManager) -> PollThread<(TerrainContents, Vec<VoxelModification>)> {
		// let grass = blocks.key_by_name(&"grass".into()).unwrap();
		// let dirt = blocks.key_by_name(&"dirt".into()).unwrap();
		let stone = blocks.key_by_name(&"stone".into()).unwrap();

		let settings = self.settings.clone();
		PollThread::new(move || {
			let mut contents = TerrainContents::new();
			settings.base(chunk_position, &mut contents, stone);
			
			(contents, vec![])
		})
	}
}



// An Arc of this is shared between generation jobs, so I've given it the generation code 
#[derive(Debug)]
struct TerrainGeneratorSettings {
	// The noise used to determine the base density of a voxel
	density_noise: RawFbmSettings,
	// Threshold for somethign to be solid or empty
	// Could be useful or useless, idk
	density_threshold: f32,

	// The noise used to determine the intended height of the world
	height_noise: RawFbmSettings,
	// Maps raw height noise [0, 1] -> intended world terrain height [a, b]
	height_spline: Spline<f32, f32>,

	// Height variability, decides where -1 and 1 is on the adjustment spline 
	height_variability_noise: RawFbmSettings,
	height_variability_spline: Spline<f32, f32>,

	// Density adjustment, adjusts density based on value [-1, 1]
	density_adjustment_spline: Spline<f32, f32>,
}
impl TerrainGeneratorSettings {
	pub fn new(seed: u32) -> Self {
		let seed = i32::from_ne_bytes(seed.to_ne_bytes());
		Self {
			density_noise: RawFbmSettings {
				seed,
				freq: 1.0 / 50.0,
				lacunarity: 2.0,
				gain: 0.5,
				octaves: 3,
			},
			density_threshold: 0.5,
			height_noise: RawFbmSettings {
				seed: seed + 1,
				freq: 1.0 / 1000.0,
				lacunarity: 2.0,
				gain: 0.5,
				octaves: 1,
			},
			height_spline: load_spline("resources/height_spline.ron").unwrap(),
			height_variability_noise: RawFbmSettings {
				seed: seed + 2,
				freq: 1.0 / 100.0,
				lacunarity: 2.0,
				gain: 0.5, 
				octaves: 1,
			},
			height_variability_spline: load_spline("resources/difference_spline.ron").unwrap(),
			density_adjustment_spline: load_spline("resources/density_spline.ron").unwrap(),
		}
	}

	// Takes raw noise values [0, 1]
	// Decides if a voxel is solid
	#[inline]
	pub fn is_solid(&self, pos: f32, density: f32, height: f32, variability: f32) -> bool {
		let height = self.height_spline.clamped_sample(height).unwrap();
		let variability = self.height_variability_spline.clamped_sample(variability).unwrap();

		// Above intended height is positive 
		let dh = pos - height; 
		// But our spline does not reflect this so invert it here
		let dh = -dh;
		let height_frac = dh / variability;
		let adjustment = self.density_adjustment_spline.clamped_sample(height_frac).unwrap();

		let density = density + adjustment;
		density >= self.density_threshold
	}

	// Generates the base solid blocks for a chunk
	pub fn base(
		&self, 
		chunk_position: IVec3, 
		volume: &mut TerrainContents,
		base: BlockKey,
	) {
		let density = fbm_scaled_linear(
			self.density_noise, 
			chunk_position * CHUNK_SIZE as i32, 
			UVec3::splat(32), 
			UVec3::splat(8),
		);

		// We could map and insert directly into the array volume, 
		// but that would require knowing the indexing implementation 
		// and I don't want to make that assumption
		for p in cube_iterator_xyz_uvec(UVec3::splat(CHUNK_SIZE)) {
			let density = density[(
				p.z * CHUNK_SIZE * CHUNK_SIZE +
				p.y * CHUNK_SIZE +
				p.x
			) as usize];

			let world_pos = chunk_position * CHUNK_SIZE as i32 + p.as_ivec3();
			let solid = self.is_solid(world_pos.y as f32, density, 0.5, 0.5);

			if solid {
				volume.insert(p, base);
			}
		}
	}
}


/// Splines are loaded from disk when calling [Self::new]. 
/// If something fails during that, the prgoram will panic.  
#[derive(Debug)]
pub struct TerrainGenerator2 {
	// The noise used to determine the base density of a voxel
	density_noise: RawFbmSettings,
	density_threshold: f32,
	// Density adjustment, difference from intended height -> density adjustment
	density_spline: Spline<f32, f32>,

	// The noise used to determine the intended height of the world
	height_noise: RawFbmSettings,
	// Maps raw height noise -> intended world terrain height
	height_spline: Spline<f32, f32>,

	// The noise used to create a multiplier for the difference from intended height
	// Think of this as a "weirdness" value
	height_difference_noise: RawFbmSettings,
	// Maps raw height difference noise -> height difference multiplier
	height_difference_spline: Spline<f32, f32>,
}
impl TerrainGenerator2 {
	pub fn new(seed: i32) -> Self {
		Self {
			density_noise: RawFbmSettings {
				seed,
				freq: 1.0 / 50.0,
				lacunarity: 2.0,
				gain: 0.5,
				octaves: 3,
			},
			density_threshold: 0.5,
			density_spline: load_spline("resources/density_spline.ron").unwrap(),
			height_noise: RawFbmSettings {
				seed: seed + 1,
				freq: 1.0 / 1000.0,
				lacunarity: 2.0,
				gain: 0.5,
				octaves: 1,
			},
			height_spline: load_spline("resources/height_spline.ron").unwrap(),
			height_difference_noise: RawFbmSettings {
				seed: seed + 2,
				freq: 1.0 / 100.0,
				lacunarity: 2.0,
				gain: 0.5, 
				octaves: 1,
			},
			height_difference_spline: load_spline("resources/difference_spline.ron").unwrap(),
		}
	}

	pub fn max_height(_world_position: IVec2, _extent: UVec2) -> Option<Vec<i32>> {
		// None if the max x key's y value in density adjustment is not 1
		todo!("Max height")
	}

	/// A lookahead method for knowing if a block will be solid. 
	/// Can generate single positions, columns, or whole chunks worth of solidity data! 
	fn is_solid(
		&self, 
		world_position: IVec3,
		extent: UVec3,
	) -> Vec<bool> {
		let [x_offset, y_offset, z_offset] = world_position.to_array();
		let [x_extent, y_extent, z_extent] = extent.to_array();

		// Sample height (2d fbm -> height spline)
		// Outputs in yx order
		let height_scale = self.height_noise.compute_scale();
		let heights = simdnoise::NoiseBuilder::fbm_2d_offset(
			x_offset as f32 + 0.5, x_extent as usize, 
			z_offset as f32 + 0.5, z_extent as usize,
		).apply_raw_settings(self.height_noise).generate().0.into_iter()
			.map(|d| (d * height_scale + 1.0) / 2.0) // Normalize
			.map(|height_noise| {
				self.height_spline.clamped_sample(height_noise).unwrap()
			})
			.collect::<Vec<_>>();

		let height_difference_scale = self.height_difference_noise.compute_scale();
		let height_differences = simdnoise::NoiseBuilder::fbm_2d_offset(
			x_offset as f32 + 0.5, x_extent as usize, 
			z_offset as f32 + 0.5, z_extent as usize,
		).apply_raw_settings(self.height_difference_noise).generate().0.into_iter()
			.map(|d| (d * height_difference_scale + 1.0) / 2.0) // Normalize
			.map(|noise| {
				self.height_difference_spline.clamped_sample(noise).unwrap()
			})
			.collect::<Vec<_>>();

		// This information can be used to know if we should skip (fill or leave empty) this chunk
		// If it's below the density = 1.0 cutoff (or the -1.0 one) then it can be filled 
		// Problem with that: it assumes that our spline ends with 1.0 and -1.0
		// We might not do that! (floating islands, caves)
		// Given the speed of my benchmarks, it should not be needed either

		let densities = fbm_scaled_linear(self.density_noise, world_position, extent, UVec3::splat(8));
		for d in densities.iter().copied() {
			const ERR: f32 = 0.05;
			// assert!(d <= 1.0, "a density value {d} > 1.0 ({})", ((d * 2.0) - 1.0) / density_scale);
			if d > 1.0 + ERR {
				println!("a density value {d} > 1.0");
				break
			}
			// assert!(d >= 0.0, "a density value {d} < 0.0 ({})", ((d * 2.0) - 1.0) / density_scale);
			if d < 0.0 - ERR {
				println!("a density value {d} < 0.0");
				break
			}
		}

		// Because simd_noise outputs in zyx/yx order, we can't just zip() here
		cube_iterator_xyz_uvec(extent)
			.map(|p| (p, p.as_ivec3() + world_position))
			.map(|(p, world_pos)| {
				let density = densities[(
					p.z * y_extent * x_extent +
					p.y * x_extent +
					p.x
				) as usize];
				let height = heights[(
					p.z * x_extent +
					p.x
				) as usize];
				let height_difference = height_differences[(
					p.z * x_extent +
					p.x
				) as usize];

				let height_diff = (height - world_pos.y as f32) * height_difference;
				let density_adjustment = self.density_spline.clamped_sample(height_diff).unwrap();
				density + density_adjustment
			})
			.map(|d| d >= self.density_threshold).collect()

		// cube_iterator_xyz_uvec(extent).map(|p| {
		// 	(p.as_ivec3() + world_position).y < 0
		// }).collect()
	}

	// Generates the base solid blocks for a chunk
	pub fn base(
		&self, 
		chunk_position: IVec3, 
		volume: &mut TerrainContents,
		base: BlockKey,
	) {

		let solidity = self.is_solid(chunk_position * CHUNK_SIZE as i32, UVec3::splat(CHUNK_SIZE));

		// We could map and insert directly into the array volume, 
		// but that would require knowing the indexing implementation 
		// and I don't want to make that assumption
		for (pos, solid) in cube_iterator_xyz_uvec(UVec3::splat(CHUNK_SIZE)).zip(solidity) {
			if solid {
				volume.insert(pos, base);
			}
		}
	}
}


// #[cfg(test)]
// pub mod tests {
// 	use super::*;
// 	use test::Bencher;

// 	/// Tests that my magic scaling number is still working 
// 	#[test]
// 	fn test_noise_normalization() {
// 		let settings = RawFbmSettings {
// 			seed: 0,
// 			freq: 1.0,
// 			lacunarity: 1.0,
// 			gain: 2.5,
// 			octaves: 6,
// 		};

// 		let extent = 256;
// 		let (noise, _, _) = simdnoise::NoiseBuilder::fbm_3d(extent, extent, extent).apply_raw_settings(settings).generate();
		
// 		let min = noise.iter().min_by(|a, b| a.total_cmp(b)).unwrap();
// 		let max = noise.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
// 		println!("Max {max}, Min {min}");

// 		let scale = settings.compute_scale();
// 		println!("Scale {scale}");
// 		let normed = noise.into_iter().map(|v| (v * scale + 1.0) / 2.0).collect::<Vec<_>>();

// 		let min = normed.iter().min_by(|a, b| a.total_cmp(b)).unwrap();
// 		let max = normed.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
// 		println!("Max {max}, Min {min}");

// 		assert!(normed.iter().copied().all(|v| v <= 1.0));
// 		assert!(normed.iter().copied().all(|v| v >= 0.0));
// 	}

// 	#[bench]
// 	fn bench_interpolated_noise(b: &mut Bencher) {
// 		let scale: UVec3 = UVec3::splat(4);
// 		let extent: UVec3 = UVec3::splat(32);
// 		let settings = RawFbmSettings {
// 			seed: 42,
// 			freq: 1.0 / 50.0,
// 			lacunarity: 2.0,
// 			gain: 0.5,
// 			octaves: 3,
// 		};

// 		b.iter(|| {
// 			let world_pos = rand::random::<IVec3>();
// 			let [x_offset, y_offset, z_offset] = world_pos.to_array();
// 			let [x_extent, y_extent, z_extent] = extent.to_array();
// 			let [x_scale, y_scale, z_scale] = scale.to_array();
			
// 			InteroplatedGeneratorNoise::generate(
// 				settings, 
// 				x_offset, x_extent, x_scale, 
// 				y_offset, y_extent, y_scale, 
// 				z_offset, z_extent, z_scale,
// 			)
// 		});
// 	}

// 	#[bench]
// 	fn bench_uninterpolated_noise(b: &mut Bencher) {
// 		let extent: usize = 32;

// 		let settings = RawFbmSettings {
// 			seed: 42,
// 			freq: 1.0 / 50.0,
// 			lacunarity: 2.0,
// 			gain: 0.5,
// 			octaves: 3,
// 		};

// 		b.iter(|| {
// 			let world_pos = rand::random::<IVec3>();
// 			let st = world_pos / extent as i32 * extent as i32;
// 			let [x_offset, y_offset, z_offset] = st.as_vec3().to_array();

// 			let data = simdnoise::NoiseBuilder::fbm_3d_offset(
// 				x_offset as f32 + 0.5, extent, 
// 				y_offset as f32 + 0.5, extent, 
// 				z_offset as f32 + 0.5, extent,
// 			).apply_raw_settings(settings).generate().0;

// 			data
// 		});
// 	}

// 	// /// Generates chunks until one is fully solid and another is fully empty
// 	// #[test]
// 	// fn test_density_falloff() {
// 	// 	let base = 0;
// 	// 	let x = 0;
// 	// 	let z = 0;
// 	// 	let mut y_min = None;
// 	// 	let mut y_max = None;
// 	// 	let max_look_length = 10; // Look five chunks up or down

// 	// 	let generator = NewTerrainGenerator::new(0);

// 	// 	println!("Looking up...");
// 	// 	for y in base..=base+max_look_length {
// 	// 		let chunk_position = IVec3::new(x, y, z);
// 	// 		let mut volume = ArrayVolume::new(UVec3::splat(CHUNK_SIZE));
// 	// 		generator.base(chunk_position, &mut volume, BlockKey::default());

// 	// 		let n_solid = volume.contents.iter().filter(|v| v.is_some()).count();
// 	// 		println!("y={y} is {:.2}% solid ({} / {})", n_solid as f32 / CHUNK_SIZE.pow(3) as f32 * 100.0, n_solid, CHUNK_SIZE.pow(3));

// 	// 		if volume.contents.iter().all(|v| v.is_none()) {
// 	// 			println!("y={y} is fully empty");
// 	// 			y_max = Some(y);
// 	// 			break
// 	// 		}
// 	// 	}
// 	// 	assert!(y_max.is_some(), "No fully empty chunk found");

// 	// 	println!("Looking down...");
// 	// 	for y in (base-max_look_length..=base).rev() {
// 	// 		let chunk_position = IVec3::new(x, y, z);
// 	// 		let mut volume = ArrayVolume::new(UVec3::splat(CHUNK_SIZE));
// 	// 		generator.base(chunk_position, &mut volume, BlockKey::default());

// 	// 		let n_solid = volume.contents.iter().filter(|v| v.is_some()).count();
// 	// 		println!("y={y} is {:.2}% solid ({} / {})", n_solid as f32 / CHUNK_SIZE.pow(3) as f32 * 100.0, n_solid, CHUNK_SIZE.pow(3));

// 	// 		if volume.contents.iter().all(|v| v.is_some()) {
// 	// 			println!("y={y} is fully solid");
// 	// 			y_min = Some(y);
// 	// 			break
// 	// 		}
// 	// 	}
// 	// 	assert!(y_min.is_some(), "No fully solid chunk found");
// 	// }

// 	#[test]
// 	fn test_3d_fbm_index() {
// 		let settings = RawFbmSettings {
// 			seed: 0,
// 			freq: 0.05,
// 			lacunarity: 1.0,
// 			gain: 2.5,
// 			octaves: 6,
// 		};

// 		let distance = 15;

// 		let (noise, _, _) = simdnoise::NoiseBuilder::fbm_3d_offset(
// 			0.0, distance, 
// 			0.25, 1, 
// 			0.25, 1,
// 		).apply_raw_settings(settings).generate();
// 		let a = noise[distance-1];
// 		println!("{noise:?}");
// 		dbg!(a);

// 		let (noise, _, _) = simdnoise::NoiseBuilder::fbm_3d_offset(
// 			(distance-2) as f32, 3, 
// 			0.25, 1, 
// 			0.25, 1,
// 		).apply_raw_settings(settings).generate();
// 		let b = noise[1];
// 		println!("{noise:?}");
// 		dbg!(b);

// 		assert!(a - b <= f32::EPSILON);
// 	}
// }
