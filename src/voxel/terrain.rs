use std::path::Path;

use crate::voxel::*;
use glam::UVec3;
use noise::Perlin;
use simdnoise::FbmSettings;
use splines::{Interpolation, Key, Spline};
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
struct RawFbmSettings {
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


/// Splines are loaded from disk when calling [Self::new]. 
/// If something fails during that, the prgoram will panic.  
#[derive(Debug)]
pub struct NewTerrainGenerator {
	density_noise: RawFbmSettings,
	density_threshold: f32,
	density_spline: Spline<f32, f32>,

	height_noise: RawFbmSettings,
	height_spline: Spline<f32, f32>,
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
		}
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

		// This information can be used to know if we should skip (fill or leave empty) this chunk
		// If it's below the density = 1.0 cutoff (or the -1.0 one) then it can be filled 
		// Problem with that: it assumes that our spline ends with 1.0 and -1.0
		// We might not do that! (floating islands, caves)
		// Given the speed of my benchmarks, it should not be needed either

		// Outputs in zyx order
		let density_scale = self.density_noise.compute_scale();
		let densities = simdnoise::NoiseBuilder::fbm_3d_offset(
			x_offset as f32 + 0.5, x_extent as usize, 
			y_offset as f32 + 0.5, y_extent as usize, 
			z_offset as f32 + 0.5, z_extent as usize,
		).apply_raw_settings(self.density_noise).generate().0.into_iter()
			.map(|d| (d * density_scale + 1.0) / 2.0) // Normalize
			.collect::<Vec<_>>();

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

				let height_diff = height - world_pos.y as f32;
				let density_adjustment = self.density_spline.clamped_sample(height_diff).unwrap();
				density + density_adjustment
			})
			.map(|d| d >= self.density_threshold).collect()
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
		for x in 0..CHUNK_SIZE {
			for z in 0..CHUNK_SIZE {
				// Generate column solidity
				// The orderign of this might be wrong, just do .rev() if it is
				let solidity = self.is_solid(
					CHUNK_SIZE as i32 * chunk_position + IVec3::new(x as i32, 0, z as i32), 
					UVec3::new(1, CHUNK_SIZE + fill_depth as u32, 1),
				);

				let mut fill_to_place = 0;
				let mut last_was_empty = false;
				// Descend y
				for (y, solid) in solidity.into_iter().enumerate().rev() {
					// Never set an empty voxel
					if !solid {
						// Reset fill counter
						last_was_empty = true;
						fill_to_place = 0;
						continue
					} else {
						let in_chunk = y < CHUNK_SIZE as usize;

						// Set top if exposed on top
						if last_was_empty {
							// Begin placing fill
							fill_to_place = fill_depth;
							if in_chunk {
								// This y could be wrong
								volume.insert(UVec3::new(x as u32, y as u32, z as u32), top);
							}
						} else {
							// If not exposed and more fill to place, set fill
							if fill_to_place != 0 {
								fill_to_place -= 1;
								if in_chunk {
									// This y could be wrong
									volume.insert(UVec3::new(x as u32, y as u32, z as u32), fill);
								}
							}
						}

						last_was_empty = false;
					}
				}
			}
		}
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



#[derive(Debug)]
pub struct TerrainGenerator {
	density_noise: Perlin,
	density_noise_threshold: f64,

	max_height_perlin: Perlin,
	/// Max height noise -> max height
	max_height_spline: Spline<f64, f64>,

	/// Elevation relative to max height -> adjustment in density
	density_adjustment_spline: Spline<f64, f64>,
	/// Density adjustment noise -> density adjustment multiplier
	height_difference_multiplier_spline: Spline<f64, f64>,
	/// Essentially a "weirdness" value
	height_difference_multiplier_perlin: Perlin,

	// Not send?!
	// cave_noise: Worley,
	// cave_noise_thresold: f64,
}
impl TerrainGenerator {
	pub fn new(seed: u32) -> Self {
		Self {
			density_noise: Perlin::new(seed),
			density_noise_threshold: 0.5,

			max_height_perlin: Perlin::new(seed+1),
			max_height_spline: Spline::from_vec(vec![
				// Ocean island
				Key::new(0.0, 10.0, Interpolation::default()),
				Key::new(0.05, 3.0, Interpolation::default()),
				// Ocean floor
				Key::new(0.1, -30.0, Interpolation::default()),
				Key::new(0.25, -30.0, Interpolation::default()),
				// Normal ground
				Key::new(0.4, 3.0, Interpolation::default()),
				Key::new(0.45, 7.0, Interpolation::default()),
				Key::new(0.5, 25.0, Interpolation::default()),
				// Mountains
				Key::new(0.70, 30.0, Interpolation::default()),
				Key::new(0.80, 90.0, Interpolation::default()),
				Key::new(1.0, 100.0, Interpolation::default()),
			]),

			density_adjustment_spline: Spline::from_vec(vec![
				Key::new(-20.0, 1.0, Interpolation::Cosine),
				Key::new(20.0, -1.0, Interpolation::Cosine),
			]),
			height_difference_multiplier_spline: Spline::from_vec(vec![
				Key::new(0.0, 1.0, Interpolation::Cosine),
				Key::new(1.0, 0.1, Interpolation::Cosine),
			]),
			height_difference_multiplier_perlin: Perlin::new(seed+2),

			// cave_noise: Worley::new(seed),
			// cave_noise_thresold: 0.5,
		}
	}

	/// Should this position be solid by default?
	#[inline]
	pub fn is_solid_default(&self, world_position: IVec3) -> bool {
		let density = crate::noise::octave_perlin_3d(
			&self.density_noise, 
			world_position.as_vec3().to_array().map(|v| (v as f64 + 0.5) / 50.0), 
			4, 
			0.5,
			2.0,
		);

		let [x_world, y_world, z_world] = world_position.to_array();

		let max_height = self.max_height_spline.clamped_sample(
			crate::noise::octave_perlin_2d(
				&self.max_height_perlin, 
				[
					(x_world as f64 + 0.5) / 300.0,
					(z_world as f64 + 0.5) / 300.0,
				], 
				1, 
				0.5, 
				2.0,
			).powf(3.0)
		).expect("Spline sampling failed!?");

		let density_adjustment = self.density_adjustment_spline.clamped_sample(
			(y_world as f64 - max_height) * self.height_difference_multiplier_spline.clamped_sample(
				crate::noise::octave_perlin_2d(
					&self.height_difference_multiplier_perlin, 
					[
						(x_world as f64 + 0.5) / 50.0,
						(z_world as f64 + 0.5) / 50.0,
					], 
					1, 
					0.5, 
					2.0,
				).powf(1.5)
			).unwrap()
		).unwrap();

		let new_density = density + density_adjustment;
		//f64::max(f64::min(density + density_adjustment, 1.0), 0.0);

		new_density >= self.density_noise_threshold
	}

	/// Creates a chunk base based on 3d noise
	pub fn chunk_base_3d(
		&self, 
		chunk_position: IVec3, 
		chunk: &mut ArrayVolume<BlockKey>,
		base: BlockKey,
	) {
		for x in 0..chunk.size[0] {
			let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
			for z in 0..chunk.size[2] {
				let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;
				for y in 0..chunk.size[1] {
					let y_world = chunk.size[1] as i32 * chunk_position[1] + y as i32;

					if self.is_solid_default(IVec3::new(x_world, y_world, z_world)) {
						chunk.insert(
							UVec3::new(x, y, z), 
							base,	
						)
					}
				}				
			}
		}
	}

	/// Creates blockmods which add grass and dirt layers to a chunk.
	/// Relies upon the bare shape with no regard to world contents.
	// Todo: Stop outputting blockmods, just take and return chunk?
	pub fn cover_chunk(
		&self,
		chunk: &mut ArrayVolume<BlockKey>,
		chunk_position: IVec3, 
		top: BlockKey,
		fill: BlockKey,
		fill_depth: i32, // n following top placement
	) {
		let chunk_size = chunk.size;

		for x in 0..chunk_size[0] as i32 {
			for z in 0..chunk_size[2] as i32 {
				let mut dirt_to_place = 0;
				let mut last_was_empty = false;
				// Descend y
				for y in (0..chunk_size[1] as i32 + fill_depth+1).rev() {
					let v_world_position = chunk_position * CHUNK_SIZE as i32 + IVec3::new(x, y, z);
					
					// Never set an empty voxel
					if !self.is_solid_default(v_world_position) {
						// Reset dirt counter
						last_was_empty = true;
						dirt_to_place = 0;
						continue
					} else {
						let v_chunk_position = chunk_of_voxel(v_world_position);

						// Set grass if exposed on top
						if last_was_empty {
							dirt_to_place = 3;
							
							if v_chunk_position == chunk_position {
								chunk.insert(UVec3::new(x as u32, y as u32, z as u32), top);
							}
						} else {
							// If not exposed and more dirt to place, set dirt
							if dirt_to_place != 0 {
								dirt_to_place -= 1;
								// set to dirt if within this chunk
								if v_chunk_position == chunk_position {
									chunk.insert(UVec3::new(x as u32, y as u32, z as u32), fill);
								}
							}
						}

						last_was_empty = false;
					}

					
				}
			}
		}
	}

	// If a voxel is below sea level and is empty, fill with water
	pub fn flood_chunk(
		&self, 
		mut chunk: ArrayVolume<BlockKey>,
		flood: BlockKey,
	) -> ArrayVolume<BlockKey> {
		// for x in 0..chunk.size[0] {
		// 	for z in 0..chunk.size[2] {
		// 		for y in 0..chunk.size[1] {
		// 			if chunk.get_voxel([x as i32, y as i32, z as i32]).is_empty() {
		// 				chunk.set_voxel([x as i32, y as i32, z as i32], flood_voxel);
		// 			}
		// 		}				
		// 	}
		// }

		chunk
	}

	/// Tests if a point in a chunk is a tree generation candidate based on the chunk's noise and an r value
	// I like this very much
	// Todo: move to world coordinates
	fn is_tree_candidate_2d(
		&self,
		position: [i32; 2], 
		chunk_size_xz: [u32; 2],
		chunk_noise_2d: Vec<f32>,
		r: u32,
	) -> bool {
		let [x, z] = position;
		
		// Should be offset so that two chunks cannot generate trees beside each other
		if x == 0 || z == 0  {
			// Set top and left false
			return false
		}
		
		let r = r as i32;
		let x = x + r;
		let z = z + r;
		let max_z = chunk_size_xz[1] as i32;		
		
		let here = chunk_noise_2d[((x*max_z) + z) as usize];
		for sx in (x-r)..=(x+r) {
			for sz in (z-r)..=(z+r) {
				let sample = chunk_noise_2d[((sx*max_z) + sz) as usize];
				if sample > here {
					return false
				}
			}
		}
		true
	}

	// /// Tests if it would be reasonable to put a tree here.
	// /// Affected chunks must be at least Bare.
	// // Todo: Make it generate ON TOP of the specified block
	// //  Will let only the containing chunk be grassified
	// fn could_generate_tree(
	// 	&self,
	// 	world_position: [i32; 3], 
	// 	generates_on: &HashSet<Voxel>,
	// ) -> bool {

	// 	let [x, y, z] = world_position;
	// 	let base_voxel = map.get_voxel_world([x, y, z]).unwrap();
		
	// 	if !generates_on.contains(&base_voxel) {
	// 		return false;
	// 	}

	// 	// Must have ten blocks above
	// 	for i in 1..=10 {
	// 		if !self.is_solid_default([x, y+i, z]) {
	// 			return false;
	// 		}
	// 	}

	// 	return true
	// }

	// // Todo: only let leaves replace certain voxel values
	// // Because each chunk has its won seed this won't generate trees on top of each other unless the chunk size is big enough for that
	// pub fn treeify_chunk(
	// 	&self,
	// 	chunk_position: [i32; 3], 
	// 	map: &Map,
	// 	r: u32,
	// 	generates_on: HashSet<Voxel>,
	// 	leaves_voxel: Voxel,
	// 	trunk_voxel: Voxel,
	// ) -> Result<ChunkBlockMods, GenerationError> {
	// 	let mut block_mods = ChunkBlockMods::new(map.chunk_size);

	// 	let chunk = map.chunk(chunk_position).unwrap();
		
	// 	let [bx, by, bz] = [
	// 		chunk_position[0] * map.chunk_size[0] as i32,
	// 		chunk_position[1] * map.chunk_size[1] as i32,
	// 		chunk_position[2] * map.chunk_size[2] as i32,
	// 	];

	// 	for x in 0..chunk.size[0] as i32 {
	// 		for z in 0..chunk.size[2] as i32 {
	// 			for y in 0..chunk.size[1] as i32 {
	// 				// If it's a tree candidate
	// 				if self.is_tree_candidate_2d(
	// 					[x,z], 
	// 					[chunk.size[0], chunk.size[2]], 
	// 					crate::noise::xoshiro_2d(
	// 						crate::voxel::chunk_seed(chunk_position, 0), 
	// 						chunk.size[0] + r*2, 
	// 						chunk.size[2] + r*2,
	// 					).iter().map(|&f| f as f32).collect::<Vec<_>>(), 
	// 					r,
	// 				) {
	// 					// And we can generate a tree here
	// 					if self.could_generate_tree([bx + x, by + y, bz + z], map, &generates_on) {
	// 						let bms = self.place_tree([bx + x, by + y, bz + z], map.chunk_size, leaves_voxel, trunk_voxel);
	// 						block_mods += bms;
	// 						// Done with trees for this chunk column
	// 						break
	// 					}
	// 				}
					
	// 			}
	// 		}
	// 	}

	// 	Ok(block_mods)
	// }

	// /// Generates blockmods to put a tree world_pos
	// pub fn place_tree(
	// 	&self,
	// 	world_pos: [i32; 3],
	// 	chunk_size: [u32; 3],
	// 	leaves_voxel: Voxel,
	// 	trunk_voxel: Voxel,
	// ) -> ChunkBlockMods {
	// 	let mut block_mods = ChunkBlockMods::new(chunk_size);
	// 	let [x, y, z] = world_pos;

	// 	// Trunk
	// 	for i in 1..=5 {
	// 		block_mods += VoxelModification {
	// 			position: VoxelPosition::WorldRelative([x, y+i, z]),
	// 			reason: BlockModReason::WorldGenSet(trunk_voxel),
	// 		};
	// 	}
		
	// 	let leaflayers = [
	// 		[0; 25],
	// 		[0; 25],
	// 		[0; 25],
	// 		[
	// 			1, 1, 1, 1, 1,
	// 			1, 1, 1, 1, 1,
	// 			1, 1, 0, 1, 1,
	// 			1, 1, 1, 1, 1,
	// 			1, 1, 1, 1, 1,
	// 		],
	// 		[
	// 			1, 1, 1, 1, 1,
	// 			1, 1, 1, 1, 1,
	// 			1, 1, 0, 1, 1,
	// 			1, 1, 1, 1, 1,
	// 			1, 1, 1, 1, 1,
	// 		],
	// 		[
	// 			0, 0, 0, 0, 0, 
	// 			0, 1, 1, 1, 0, 
	// 			0, 1, 1, 1, 0, 
	// 			0, 1, 1, 1, 0, 
	// 			0, 0, 0, 0, 0, 
	// 		],
	// 		[
	// 			0, 0, 0, 0, 0, 
	// 			0, 0, 1, 0, 0, 
	// 			0, 1, 1, 1, 0, 
	// 			0, 0, 1, 0, 0, 
	// 			0, 0, 0, 0, 0, 
	// 		],
	// 	].map(|i| i.map(|i| i == 1));

	// 	for (ly, y_slice) in leaflayers.iter().enumerate() {
	// 		let ly = ly as i32 + 1;
	// 		for (lx, z_slice) in y_slice.chunks_exact(5).enumerate() {
	// 			let lx = lx as i32 - 2;
	// 			for (lz, &bleaves) in z_slice.iter().enumerate() {
	// 				let lz = lz as i32 - 2;
	// 				if bleaves {
	// 					block_mods += VoxelModification {
	// 						position: VoxelPosition::WorldRelative([x+lx, y+ly, z+lz]),
	// 						reason: BlockModReason::WorldGenSet(leaves_voxel),
	// 					};
	// 				}
	// 			}
	// 		}
	// 	}

	// 	block_mods
	// }

	pub fn carve_chunk(
		&self, 
		chunk_position: [i32; 3], 
		mut chunk: ArrayVolume<BlockKey>,
	) -> ArrayVolume<BlockKey> {
		// for x in 0..chunk.size[0] {
		// 	let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
		// 	for z in 0..chunk.size[2] {
		// 		let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;
		// 		for y in 0..chunk.size[1] {
		// 			let y_world = chunk.size[1] as i32 * chunk_position[1] + y as i32;

		// 			let density = self.cave_noise.get([
		// 				x_world as f64 / 5.0, 
		// 				y_world as f64 / 5.0,
		// 				z_world as f64 / 5.0,
		// 			]) / 2.0 + 0.5;

		// 			if density >= self.cave_noise_thresold {
		// 				chunk.set_voxel([x as i32, y as i32, z as i32], Voxel::Empty)
		// 			}
		// 		}
		// 	}
		// }

		chunk
	}
}


// #[cfg(Test)]
pub mod tests {
    use glam::{IVec3, UVec3};
    use crate::voxel::{ArrayVolume, CHUNK_SIZE, BlockKey, terrain::{ConfigureRawFbm, RawFbmSettings}};
    use super::NewTerrainGenerator;

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


	/// Generates chunks until one is fully solid and another is fully empty
	#[test]
	fn test_density_falloff() {
		let base = 0;
		let x = 0;
		let z = 0;
		let mut y_min = None;
		let mut y_max = None;
		let max_look_length = 10; // Look five chunks up or down

		let generator = NewTerrainGenerator::new(0);

		println!("Looking up...");
		for y in base..=base+max_look_length {
			let chunk_position = IVec3::new(x, y, z);
			let mut volume = ArrayVolume::new(UVec3::splat(CHUNK_SIZE));
			generator.base(chunk_position, &mut volume, BlockKey::default());

			let n_solid = volume.contents.iter().filter(|v| v.is_some()).count();
			println!("y={y} is {:.2}% solid ({} / {})", n_solid as f32 / CHUNK_SIZE.pow(3) as f32 * 100.0, n_solid, CHUNK_SIZE.pow(3));

			if volume.contents.iter().all(|v| v.is_none()) {
				println!("y={y} is fully empty");
				y_max = Some(y);
				break
			}
		}
		assert!(y_max.is_some(), "No fully empty chunk found");

		println!("Looking down...");
		for y in (base-max_look_length..=base).rev() {
			let chunk_position = IVec3::new(x, y, z);
			let mut volume = ArrayVolume::new(UVec3::splat(CHUNK_SIZE));
			generator.base(chunk_position, &mut volume, BlockKey::default());

			let n_solid = volume.contents.iter().filter(|v| v.is_some()).count();
			println!("y={y} is {:.2}% solid ({} / {})", n_solid as f32 / CHUNK_SIZE.pow(3) as f32 * 100.0, n_solid, CHUNK_SIZE.pow(3));

			if volume.contents.iter().all(|v| v.is_some()) {
				println!("y={y} is fully solid");
				y_min = Some(y);
				break
			}
		}
		assert!(y_min.is_some(), "No fully solid chunk found");
	}

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

	// #[bench]
	// fn bench_old_generator(b: &mut Bencher) {
		
	// 	b.iter(|| {
			
	// 	});
	// }

	// #[bench]
	// fn bench_new_generator(b: &mut Bencher) {
		
	// 	b.iter(|| {
			
	// 	});
	// }
}


