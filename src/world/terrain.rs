use std::collections::HashSet;
use crate::world::*;
use noise::Perlin;
use noise::Seedable;
use noise::Worley;
use noise::NoiseFn;
use splines::{Interpolation, Key, Spline};
use thiserror::Error;




#[derive(Error, Debug)]
pub enum GenerationError {
	#[error("failed to find block entry for '{0}'")]
	BlockNotFoundError(String),
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

	cave_noise: Worley,
	cave_noise_thresold: f64,
}
impl TerrainGenerator {
	pub fn new(seed: u32) -> Self {
		Self {
			density_noise: Perlin::new().set_seed(seed),
			density_noise_threshold: 0.5,

			max_height_perlin: Perlin::new().set_seed(seed+1),
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
			height_difference_multiplier_perlin: Perlin::new().set_seed(seed+2),

			cave_noise: Worley::new().set_seed(seed),
			cave_noise_thresold: 0.5,
		}
	}

	/// Should this position be solid by default?
	#[inline]
	fn is_solid_default(&self, world_position: [i32; 3]) -> bool {
		let density = crate::noise::octave_perlin_3d(
			&self.density_noise, 
			world_position.map(|v| (v as f64 + 0.5) / 50.0), 
			4, 
			0.5,
			2.0,
		);

		let [x_world, y_world, z_world] = world_position;

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
		chunk_position: [i32; 3], 
		mut chunk: Chunk,
		base_voxel: Voxel,
	) -> Chunk {
		for x in 0..chunk.size[0] {
			let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
			for z in 0..chunk.size[2] {
				let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;
				for y in 0..chunk.size[1] {
					let y_world = chunk.size[1] as i32 * chunk_position[1] + y as i32;

					chunk.set_voxel(
						[x as i32, y as i32, z as i32], 
						if self.is_solid_default([x_world, y_world, z_world]) {
							base_voxel
						} else {
							Voxel::Empty
						},
					)
				}				
			}
		}

		chunk
	}

	/// Creates blockmods which add grass and dirt layers to a chunk.
	/// Relies upon the bare shape with no regard to world contents.
	// Todo: Stop outputting blockmods, just take and return chunk?
	pub fn cover_chunk(
		&self,
		mut chunk: Chunk,
		chunk_position: [i32; 3], 
		top_voxel: Voxel,
		fill_voxel: Voxel,
		fill_depth: i32, // n following top placement
	) -> Chunk {
		let chunk_size = chunk.size;

		for x in 0..chunk_size[0] as i32 {
			for z in 0..chunk_size[2] as i32 {
				let mut dirt_to_place = 0;
				let mut last_was_empty = false;
				// Descend y
				for y in (0..chunk_size[1] as i32 + fill_depth+1).rev() {
					let v_world_position = chunk_voxel_world(chunk_position, [x, y, z], chunk_size);
					
					// Never set an empty voxel
					if !self.is_solid_default(v_world_position) {
						// Reset dirt counter
						last_was_empty = true;
						dirt_to_place = 0;
						continue
					} else {
						let v_chunk_position = world_chunk(v_world_position, chunk_size);

						// Set grass if exposed on top
						if last_was_empty {
							dirt_to_place = 3;
							
							if v_chunk_position == chunk_position {
								chunk.set_voxel([x,y,z], top_voxel);
							}
						} else {
							// If not exposed and more dirt to place, set dirt
							if dirt_to_place != 0 {
								dirt_to_place -= 1;
								// set to dirt if within this chunk
								if v_chunk_position == chunk_position {
									chunk.set_voxel([x,y,z], fill_voxel);
								}
							}
						}

						last_was_empty = false;
					}

					
				}
			}
		}

		chunk
	}

	// If a voxel is below sea level and is empty, fill with water
	pub fn flood_chunk(
		&self, 
		mut chunk: Chunk,
		flood_voxel: Voxel,
	) -> Chunk {
		for x in 0..chunk.size[0] {
			for z in 0..chunk.size[2] {
				for y in 0..chunk.size[1] {
					if chunk.get_voxel([x as i32, y as i32, z as i32]).is_empty() {
						chunk.set_voxel([x as i32, y as i32, z as i32], flood_voxel);
					}
				}				
			}
		}

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

	/// Tests if it would be reasonable to put a tree here.
	/// Affected chunks must be at least Bare.
	// Todo: Make it generate ON TOP of the specified block
	//  Will let only the containing chunk be grassified
	fn could_generate_tree(
		&self,
		world_position: [i32; 3], 
		map: &Map,
		generates_on: &HashSet<Voxel>,
	) -> bool {

		let [x, y, z] = world_position;
		let base_voxel = map.get_voxel_world([x, y, z]).unwrap();
		
		if !generates_on.contains(&base_voxel) {
			return false;
		}

		// Must have ten blocks above
		for i in 1..=10 {
			if !self.is_solid_default([x, y+i, z]) {
				return false;
			}
		}

		return true
	}

	// Todo: only let leaves replace certain voxel values
	// Because each chunk has its won seed this won't generate trees on top of each other unless the chunk size is big enough for that
	pub fn treeify_chunk(
		&self,
		chunk_position: [i32; 3], 
		map: &Map,
		r: u32,
		generates_on: HashSet<Voxel>,
		leaves_voxel: Voxel,
		trunk_voxel: Voxel,
	) -> Result<ChunkBlockMods, GenerationError> {
		let mut block_mods = ChunkBlockMods::new(map.chunk_size);

		let chunk = map.chunk(chunk_position).unwrap();
		
		let [bx, by, bz] = [
			chunk_position[0] * map.chunk_size[0] as i32,
			chunk_position[1] * map.chunk_size[1] as i32,
			chunk_position[2] * map.chunk_size[2] as i32,
		];

		for x in 0..chunk.size[0] as i32 {
			for z in 0..chunk.size[2] as i32 {
				for y in 0..chunk.size[1] as i32 {
					// If it's a tree candidate
					if self.is_tree_candidate_2d(
						[x,z], 
						[chunk.size[0], chunk.size[2]], 
						crate::noise::xoshiro_2d(
							crate::world::chunk_seed(chunk_position, 0), 
							chunk.size[0] + r*2, 
							chunk.size[2] + r*2,
						).iter().map(|&f| f as f32).collect::<Vec<_>>(), 
						r,
					) {
						// And we can generate a tree here
						if self.could_generate_tree([bx + x, by + y, bz + z], map, &generates_on) {
							let bms = self.place_tree([bx + x, by + y, bz + z], map.chunk_size, leaves_voxel, trunk_voxel);
							block_mods += bms;
							// Done with trees for this chunk column
							break
						}
					}
					
				}
			}
		}

		Ok(block_mods)
	}

	/// Generates blockmods to put a tree world_pos
	pub fn place_tree(
		&self,
		world_pos: [i32; 3],
		chunk_size: [u32; 3],
		leaves_voxel: Voxel,
		trunk_voxel: Voxel,
	) -> ChunkBlockMods {
		let mut block_mods = ChunkBlockMods::new(chunk_size);
		let [x, y, z] = world_pos;

		// Trunk
		for i in 1..=5 {
			block_mods += BlockMod {
				position: VoxelPosition::WorldRelative([x, y+i, z]),
				reason: BlockModReason::WorldGenSet(trunk_voxel),
			};
		}
		
		let leaflayers = [
			[0; 25],
			[0; 25],
			[0; 25],
			[
				1, 1, 1, 1, 1,
				1, 1, 1, 1, 1,
				1, 1, 0, 1, 1,
				1, 1, 1, 1, 1,
				1, 1, 1, 1, 1,
			],
			[
				1, 1, 1, 1, 1,
				1, 1, 1, 1, 1,
				1, 1, 0, 1, 1,
				1, 1, 1, 1, 1,
				1, 1, 1, 1, 1,
			],
			[
				0, 0, 0, 0, 0, 
				0, 1, 1, 1, 0, 
				0, 1, 1, 1, 0, 
				0, 1, 1, 1, 0, 
				0, 0, 0, 0, 0, 
			],
			[
				0, 0, 0, 0, 0, 
				0, 0, 1, 0, 0, 
				0, 1, 1, 1, 0, 
				0, 0, 1, 0, 0, 
				0, 0, 0, 0, 0, 
			],
		].map(|i| i.map(|i| i == 1));

		for (ly, y_slice) in leaflayers.iter().enumerate() {
			let ly = ly as i32 + 1;
			for (lx, z_slice) in y_slice.chunks_exact(5).enumerate() {
				let lx = lx as i32 - 2;
				for (lz, &bleaves) in z_slice.iter().enumerate() {
					let lz = lz as i32 - 2;
					if bleaves {
						block_mods += BlockMod {
							position: VoxelPosition::WorldRelative([x+lx, y+ly, z+lz]),
							reason: BlockModReason::WorldGenSet(leaves_voxel),
						};
					}
				}
			}
		}

		block_mods
	}

	pub fn carve_chunk(
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

					let density = self.cave_noise.get([
						x_world as f64 / 5.0, 
						y_world as f64 / 5.0,
						z_world as f64 / 5.0,
					]) / 2.0 + 0.5;

					if density >= self.cave_noise_thresold {
						chunk.set_voxel([x as i32, y as i32, z as i32], Voxel::Empty)
					}
				}
			}
		}

		chunk
	}
}



#[cfg(test)]
mod tests {
	use super::*;
	use rayon::prelude::*;

	#[test]
    fn spt() {
		let s = Spline::from_vec(vec![
			// x, y
			Key::new(0.0, 0.0, Interpolation::default()),
			Key::new(1.0, 10.0, Interpolation::default()),
		]);
		let vals = vec![
			0.0,
			0.5,
			1.0,
			-1.0,
			2.0,
		];
		println!("Default:");
		vals.iter().for_each(|&v| println!("s({}) = {:?}", v, s.sample(v)));

		println!("Clamped:");
		vals.iter().for_each(|&v| println!("s({}) = {:?}", v, s.clamped_sample(v)));
	}



    #[test]
    fn show_terrain_slice() {

		const WIDTH: u32 = 1024;
		const HEIGHT: u32 = 256;

		let tgen = TerrainGenerator::new(0);

		// 0,0 -> 1,0
		// |
		// v
		// 0,1
		let output = (0..WIDTH*HEIGHT).into_par_iter().map(|v| {
			let x = (v % WIDTH) as i32;
			let y = HEIGHT as i32 - (v / WIDTH) as i32;
			let xc = x - (WIDTH / 2) as i32;
			let yc = y - (HEIGHT / 2) as i32;
			tgen.is_solid_default([
				xc as i32, yc as i32, 0,
			])
		}).collect::<Vec<_>>();

		let img = image::DynamicImage::ImageRgb8(
			image::ImageBuffer::from_vec(WIDTH, HEIGHT, output.par_iter().flat_map(|&solid| {
				if solid {
					[u8::MAX; 3]
				} else {
					[u8::MIN; 3]
				}
			}).collect::<Vec<_>>()).unwrap()
		);

		crate::util::show_image(img).unwrap();

        assert_eq!(2 + 2, 4);
    }
}
