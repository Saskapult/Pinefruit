use std::path::Path;
use chunks::{array_volume::ArrayVolume, blocks::BlockKey, cube_iterator_xyz_uvec, CHUNK_SIZE};
use glam::{IVec2, IVec3, UVec2, UVec3};
use simdnoise::FbmSettings;
use splines::Spline;
use thiserror::Error;




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
trait ConfigureRawFbm {
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


#[inline]
fn lerp(x: f32, x1: f32, x2: f32, q00: f32, q01: f32) -> f32 {
	((x2 - x) / (x2 - x1)) * q00 + ((x - x1) / (x2 - x1)) * q01
}
// #[inline]
// fn lerp2(
// 	x: f32, y: f32, 
// 	q11: f32, q12: f32, q21: f32, q22: f32, 
// 	x1: f32, x2: f32, y1: f32, y2: f32,
// ) -> f32 {
// 	let r1 = lerp(x, x1, x2, q11, q21);
// 	let r2 = lerp(x, x1, x2, q12, q22);
// 	lerp(y, y1, y2, r1, r2)
// }
#[inline]
fn lerp3(
	x: f32, y: f32, z: f32, 
	q000: f32, q001: f32, q010: f32, q011: f32, q100: f32, q101: f32, q110: f32, q111: f32, 
	x1: f32, x2: f32, y1: f32, y2: f32, z1: f32, z2: f32, 
) -> f32 {
	let x00 = lerp(x, x1, x2, q000, q100);
	let x10 = lerp(x, x1, x2, q010, q110);
	let x01 = lerp(x, x1, x2, q001, q101);
	let x11 = lerp(x, x1, x2, q011, q111);

	let r0 = lerp(y, y1, y2, x00, x01);
	let r1 = lerp(y, y1, y2, x10, x11);
   
	lerp(z, z1, z2, r0, r1)
}


pub struct InteroplatedGeneratorNoise {
	scale: UVec3, // One sample every scale voxels 
	samples: Vec<f32>, 
	samples_extent: UVec3, // Extent of noise in sample space 
	samples_origin: IVec3, // Origin of noise in sample space 
}
impl InteroplatedGeneratorNoise {
	pub fn generate(
		settings: RawFbmSettings,
		x_offset: i32, x_extent: u32, x_scale: u32,
		y_offset: i32, y_extent: u32, y_scale: u32,
		z_offset: i32, z_extent: u32, z_scale: u32,
	) -> Vec<f32> {
		let scale = UVec3::new(x_scale, y_scale, z_scale);
		let world_offset = IVec3::new(x_offset, y_offset, z_offset);
		let world_extent = UVec3::new(x_extent, y_extent, z_extent);
		let samples_extent = (world_extent / scale) + UVec3::ONE;	
		let samples_origin = world_offset.div_euclid(scale.as_ivec3());

		let [x_offset_f, y_offset_f, z_offset_f] = samples_origin.as_vec3().to_array();
		let [width, height, depth] = samples_extent.to_array();
		// println!("Noise {}x{}x{}", width, height, depth);
		let samples = simdnoise::NoiseBuilder::fbm_3d_offset(
			x_offset_f + 0.5, width as usize,
			y_offset_f + 0.5, height as usize,
			z_offset_f + 0.5, depth as usize,
		).apply_raw_settings(settings).generate().0;
		assert_eq!(width * height * depth, samples.len() as u32);
	
		let interp = Self {
			scale,
			samples,
			samples_extent,
			samples_origin,
		};
	
		let output = cube_iterator_xyz_uvec(world_extent)
			.map(|p| interp.get(world_offset + p.as_ivec3()))
			.collect::<Vec<_>>();
		assert_eq!(x_extent * y_extent * z_extent, output.len() as u32);

		// let smax = interp.samples.iter().copied().reduce(|a, v| f32::max(a, v)).unwrap();
		// let omax = output.iter().copied().reduce(|a, v| f32::max(a, v)).unwrap();
		// assert!(smax >= omax, "{smax} >= {omax}");

		output
	}

	#[inline]
	fn index_of(&self, pos: UVec3) -> usize {
		let [x, y, z] = pos.to_array();
		(z * self.samples_extent.y * self.samples_extent.x + y * self.samples_extent.x + x) as usize
	}

	#[inline]
	pub fn get(&self, pos: IVec3) -> f32 {
		let world_origin = self.samples_origin * self.scale.as_ivec3();
		let world_extent = self.samples_extent.as_ivec3() * self.scale.as_ivec3();
		assert!(pos.cmpge(world_origin).all());
		assert!(pos.cmplt(world_origin + world_extent).all());

		let samples_pos = pos.div_euclid(self.scale.as_ivec3());
		let base_cell = (samples_pos - self.samples_origin).as_uvec3();

		let q000 = self.samples[self.index_of(base_cell)];
		let q001 = self.samples[self.index_of(base_cell + UVec3::X)];
		let q010 = self.samples[self.index_of(base_cell + UVec3::Y)];
		let q011 = self.samples[self.index_of(base_cell + UVec3::X + UVec3::Y)];
		let q100 = self.samples[self.index_of(base_cell + UVec3::Z)];
		let q101 = self.samples[self.index_of(base_cell + UVec3::Z + UVec3::X)];
		let q110 = self.samples[self.index_of(base_cell + UVec3::Z + UVec3::Y)];
		let q111 = self.samples[self.index_of(base_cell + UVec3::Z + UVec3::Y + UVec3::X)];

		let pos_q000 = (samples_pos * self.scale.as_ivec3()).as_vec3();
		let pos_q111 = ((samples_pos + IVec3::ONE) * self.scale.as_ivec3()).as_vec3();
		let [x, y, z] = pos.as_vec3().to_array();
		let [x1, y1, z1] = pos_q000.to_array();
		let [x2, y2, z2] = pos_q111.to_array();

		let v = lerp3(x, y, z, q000, q001, q010, q011, q100, q101, q110, q111, x1, x2, y1, y2, z1, z2);

		if pos == samples_pos * self.scale.as_ivec3() {
			assert!((v - q000).abs() < 0.0001, "reconstructed {v} != original {q000} in same position");
		}
		
		// if [q000,q001,q010,q011,q100,q101,q110,q111].into_iter().all(|a| a < v) {
		// 	panic!("In interp output {v} > all octants");
		// }
		// let qmax = [q000,q001,q010,q011,q100,q101,q110,q111].into_iter().reduce(|a, v| f32::max(a, v)).unwrap();
		// assert!(v <= qmax, "output {v} > qmax {qmax}");
		// assert!(v <= 1.0 && v >= 0.0, "bad range on v {v}");

		v
	}
}


/// Splines are loaded from disk when calling [Self::new]. 
/// If something fails during that, the prgoram will panic.  
#[derive(Debug)]
pub struct NewTerrainGenerator {
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
impl NewTerrainGenerator {
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

		// Outputs in zyx order
		let density_scale = self.density_noise.compute_scale();
		// let densities = 
		// simdnoise::NoiseBuilder::fbm_3d_offset(
		// 	x_offset as f32 + 0.5, x_extent as usize, 
		// 	y_offset as f32 + 0.5, y_extent as usize, 
		// 	z_offset as f32 + 0.5, z_extent as usize,
		// ).apply_raw_settings(self.density_noise).generate().0
		// // vec![0.0; 32768]
		// .into_iter()
		// 	.map(|d| (d * density_scale + 1.0) / 2.0) // Normalize
		// 	.collect::<Vec<_>>();
		let densities = 
		InteroplatedGeneratorNoise::generate(
			self.density_noise, 
			x_offset, x_extent, 4, 
			y_offset, y_extent, 8, 
			z_offset, z_extent, 4,
		)
		// vec![0.0; 32768]
		.into_iter()
			.map(|d| (d * density_scale + 1.0) / 2.0) // Normalize
			.collect::<Vec<_>>();
		for d in densities.iter().copied() {
			// assert!(d <= 1.0, "a density value {d} > 1.0 ({})", ((d * 2.0) - 1.0) / density_scale);
			if d > 1.0 {
				println!("a density value {d} > 1.0 ({})", ((d * 2.0) - 1.0) / density_scale);
				break
			}
			// assert!(d >= 0.0, "a density value {d} < 0.0 ({})", ((d * 2.0) - 1.0) / density_scale);
			if d < 0.0 {
				println!("a density value {d} < 0.0 ({})", ((d * 2.0) - 1.0) / density_scale);
				break
			}
		}

		// Because simd_noise outputs in zyx/yx order, we can't just zip() here
		cube_iterator_xyz_uvec(extent)
			.map(|p| (p, p.as_ivec3() + world_position))
			.map(|(p, world_pos)| {
				// let density = densities[(
				// 	p.z * y_extent * x_extent +
				// 	p.y * x_extent +
				// 	p.x
				// ) as usize];
				let density = densities[(
					p.x * y_extent * x_extent +
					p.y * x_extent +
					p.z
				) as usize];
				// let height = heights[(
				// 	p.z * x_extent +
				// 	p.x
				// ) as usize];
				let height = 0.0;
				// let height_difference = height_differences[(
				// 	p.z * x_extent +
				// 	p.x
				// ) as usize];
				let height_difference = 1.0;

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
		volume: &mut ArrayVolume<BlockKey>,
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

	// Carve should be split into cheese, spaghetti, and noodles
	#[deprecated]
	pub fn carve(
		&self, 
		_chunk_position: IVec3, 
		_volume: &mut ArrayVolume<BlockKey>,
	) {
		todo!()
	}

	// Uses [Self::is_solid] lookahead to place covering blocks
	// This does re-generate all of the solidity data in order to do that
	// It would be better to share the solidity data
	// But it's much easier to just do this
	pub fn cover(
		&self,
		chunk_position: IVec3, 
		volume: &mut ArrayVolume<BlockKey>,
		top: BlockKey,
		fill: BlockKey,
		fill_depth: i32, // n following top placement
	) {
		// for x in 0..CHUNK_SIZE {
		// 	for z in 0..CHUNK_SIZE {
		// 		// Generate column solidity
		// 		// The orderign of this might be wrong, just do .rev() if it is
		// 		let solidity = self.is_solid(
		// 			CHUNK_SIZE as i32 * chunk_position + IVec3::new(x as i32, 0, z as i32), 
		// 			UVec3::new(1, CHUNK_SIZE + fill_depth as u32, 1),
		// 		);

		// 		let mut fill_to_place = 0;
		// 		let mut last_was_empty = false;
		// 		// Descend y
		// 		for (y, solid) in solidity.into_iter().enumerate().rev() {
		// 			// Never set an empty voxel
		// 			if !solid {
		// 				// Reset fill counter
		// 				last_was_empty = true;
		// 				fill_to_place = 0;
		// 				continue
		// 			} else {
		// 				let in_chunk = y < CHUNK_SIZE as usize;

		// 				// Set top if exposed on top
		// 				if last_was_empty {
		// 					// Begin placing fill
		// 					fill_to_place = fill_depth;
		// 					if in_chunk {
		// 						// This y could be wrong
		// 						volume.insert(UVec3::new(x as u32, y as u32, z as u32), top);
		// 					}
		// 				} else {
		// 					// If not exposed and more fill to place, set fill
		// 					if fill_to_place != 0 {
		// 						fill_to_place -= 1;
		// 						if in_chunk {
		// 							// This y could be wrong
		// 							volume.insert(UVec3::new(x as u32, y as u32, z as u32), fill);
		// 						}
		// 					}
		// 				}

		// 				last_was_empty = false;
		// 			}
		// 		}
		// 	}
		// }
	}

	#[deprecated]
	pub fn treeify(
		&self, 
		_chunk_position: IVec3, 
		_volume: &ArrayVolume<BlockKey>,
	) -> bool { // Should return block modifications
		todo!("Tree generation should be extended into a structure generation script")
	}
}


#[cfg(test)]
pub mod tests {
	use super::*;
	use test::Bencher;

	/// Tests that my magic scaling number is still working 
	#[test]
	fn test_noise_normalization() {
		let settings = RawFbmSettings {
			seed: 0,
			freq: 1.0,
			lacunarity: 1.0,
			gain: 2.5,
			octaves: 6,
		};

		let extent = 256;
		let (noise, _, _) = simdnoise::NoiseBuilder::fbm_3d(extent, extent, extent).apply_raw_settings(settings).generate();
		
		let min = noise.iter().min_by(|a, b| a.total_cmp(b)).unwrap();
		let max = noise.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
		println!("Max {max}, Min {min}");

		let scale = settings.compute_scale();
		println!("Scale {scale}");
		let normed = noise.into_iter().map(|v| (v * scale + 1.0) / 2.0).collect::<Vec<_>>();

		let min = normed.iter().min_by(|a, b| a.total_cmp(b)).unwrap();
		let max = normed.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
		println!("Max {max}, Min {min}");

		assert!(normed.iter().copied().all(|v| v <= 1.0));
		assert!(normed.iter().copied().all(|v| v >= 0.0));
	}

	#[bench]
	fn bench_interpolated_noise(b: &mut Bencher) {
		let scale: UVec3 = UVec3::splat(4);
		let extent: UVec3 = UVec3::splat(32);
		let settings = RawFbmSettings {
			seed: 42,
			freq: 1.0 / 50.0,
			lacunarity: 2.0,
			gain: 0.5,
			octaves: 3,
		};

		b.iter(|| {
			let world_pos = rand::random::<IVec3>();
			let [x_offset, y_offset, z_offset] = world_pos.to_array();
			let [x_extent, y_extent, z_extent] = extent.to_array();
			let [x_scale, y_scale, z_scale] = scale.to_array();
			
			InteroplatedGeneratorNoise::generate(
				settings, 
				x_offset, x_extent, x_scale, 
				y_offset, y_extent, y_scale, 
				z_offset, z_extent, z_scale,
			)
		});
	}

	#[bench]
	fn bench_uninterpolated_noise(b: &mut Bencher) {
		let extent: usize = 32;

		let settings = RawFbmSettings {
			seed: 42,
			freq: 1.0 / 50.0,
			lacunarity: 2.0,
			gain: 0.5,
			octaves: 3,
		};

		b.iter(|| {
			let world_pos = rand::random::<IVec3>();
			let st = world_pos / extent as i32 * extent as i32;
			let [x_offset, y_offset, z_offset] = st.as_vec3().to_array();

			let data = simdnoise::NoiseBuilder::fbm_3d_offset(
				x_offset as f32 + 0.5, extent, 
				y_offset as f32 + 0.5, extent, 
				z_offset as f32 + 0.5, extent,
			).apply_raw_settings(settings).generate().0;

			data
		});
	}

	// /// Generates chunks until one is fully solid and another is fully empty
	// #[test]
	// fn test_density_falloff() {
	// 	let base = 0;
	// 	let x = 0;
	// 	let z = 0;
	// 	let mut y_min = None;
	// 	let mut y_max = None;
	// 	let max_look_length = 10; // Look five chunks up or down

	// 	let generator = NewTerrainGenerator::new(0);

	// 	println!("Looking up...");
	// 	for y in base..=base+max_look_length {
	// 		let chunk_position = IVec3::new(x, y, z);
	// 		let mut volume = ArrayVolume::new(UVec3::splat(CHUNK_SIZE));
	// 		generator.base(chunk_position, &mut volume, BlockKey::default());

	// 		let n_solid = volume.contents.iter().filter(|v| v.is_some()).count();
	// 		println!("y={y} is {:.2}% solid ({} / {})", n_solid as f32 / CHUNK_SIZE.pow(3) as f32 * 100.0, n_solid, CHUNK_SIZE.pow(3));

	// 		if volume.contents.iter().all(|v| v.is_none()) {
	// 			println!("y={y} is fully empty");
	// 			y_max = Some(y);
	// 			break
	// 		}
	// 	}
	// 	assert!(y_max.is_some(), "No fully empty chunk found");

	// 	println!("Looking down...");
	// 	for y in (base-max_look_length..=base).rev() {
	// 		let chunk_position = IVec3::new(x, y, z);
	// 		let mut volume = ArrayVolume::new(UVec3::splat(CHUNK_SIZE));
	// 		generator.base(chunk_position, &mut volume, BlockKey::default());

	// 		let n_solid = volume.contents.iter().filter(|v| v.is_some()).count();
	// 		println!("y={y} is {:.2}% solid ({} / {})", n_solid as f32 / CHUNK_SIZE.pow(3) as f32 * 100.0, n_solid, CHUNK_SIZE.pow(3));

	// 		if volume.contents.iter().all(|v| v.is_some()) {
	// 			println!("y={y} is fully solid");
	// 			y_min = Some(y);
	// 			break
	// 		}
	// 	}
	// 	assert!(y_min.is_some(), "No fully solid chunk found");
	// }

	#[test]
	fn test_3d_fbm_index() {
		let settings = RawFbmSettings {
			seed: 0,
			freq: 0.05,
			lacunarity: 1.0,
			gain: 2.5,
			octaves: 6,
		};

		let distance = 15;

		let (noise, _, _) = simdnoise::NoiseBuilder::fbm_3d_offset(
			0.0, distance, 
			0.25, 1, 
			0.25, 1,
		).apply_raw_settings(settings).generate();
		let a = noise[distance-1];
		println!("{noise:?}");
		dbg!(a);

		let (noise, _, _) = simdnoise::NoiseBuilder::fbm_3d_offset(
			(distance-2) as f32, 3, 
			0.25, 1, 
			0.25, 1,
		).apply_raw_settings(settings).generate();
		let b = noise[1];
		println!("{noise:?}");
		dbg!(b);

		assert!(a - b <= f32::EPSILON);
	}
}
