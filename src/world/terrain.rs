use crate::world::*;
use noise::Perlin;
use noise::Seedable;
use noise::Worley;
use noise::NoiseFn;
use thiserror::Error;




pub trait SurfaceGenerator {
	// Only stones
	fn chunk_surface(&self, chunk_position: [i32; 3], chunk: Chunk, bm: &BlockManager) -> Chunk;
	// Todo: top and filler block adder (max depth, depth noise)
}
pub trait Carver {
	// Makes holes
	fn carve_chunk(&self, chunk_position: [i32; 3], chunk: Chunk) -> Chunk;
}



#[derive(Error, Debug)]
pub enum GenerationError {
	#[error("a chunk ({chunk_position:?}) containing data requisite for generation was not loaded to the minimum stage of {stage_min:?}")]
	ChunkStageLtError {
		chunk_position: [i32; 3], 
		stage_min: GenerationStage,
	},
	#[error("a chunk ({chunk_position:?}) should be of stage {stage_desired:?} but is of stage {stage_current:?}")]
	ChunkStageNeError {
		chunk_position: [i32; 3], 
		stage_current: GenerationStage,
		stage_desired: GenerationStage,
	},
	#[error("lol idk")]
	GenericError,
}



#[derive(Debug)]
pub struct TerrainGenerator {
	perlin: Perlin,
	base_noise_threshold: f64,
}
impl TerrainGenerator {
	pub fn new(seed: u32) -> Self {
		Self {
			perlin: Perlin::new().set_seed(seed),
			base_noise_threshold: 0.5,
		}
	}

	/// Should this block be solid by default?
	#[inline]
	fn is_solid_default(&self, world_position: [i32; 3]) -> bool {
		let density = crate::noise::octave_perlin_3d(
			&self.perlin, 
			world_position.map(|v| (v as f64 + 0.5) / 25.0), 
			4, 
			0.5,
			2.0,
		);

		let [_, y_world, _] = world_position;
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

		density * squashpart >= self.base_noise_threshold
	}

	/// Creates a chunk base based on 3d noise
	pub fn chunk_base_3d(
		&self, 
		chunk_position: [i32; 3], 
		mut chunk: Chunk,
		base_idx: usize
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
							Voxel::Block(base_idx)
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
		chunk_position: [i32; 3], 
		chunk_size: [u32; 3],
		top_idx: usize,
		fill_idx: usize,
		fill_depth: i32, // n following top placement
	) -> ChunkBlockMods {
		let mut block_mods = ChunkBlockMods::new();

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

						// Set dirt if needed
						// Know voxel is not exposed because it would be preceeded by empty,
						//  meaning dirt_to_place would be zero

						// Set grass if exposed on top
						if last_was_empty {
							dirt_to_place = 3;
							
							if v_chunk_position == chunk_position {
								insert_chunkblockmods(
									&mut block_mods,
									BlockMod {
										position: VoxelPosition::from_chunk_voxel(chunk_position, [x,y,z]),
										reason: BlockModReason::WorldGenSet(Voxel::Block(top_idx)),
									},
									chunk_size,
								);
							}
						} else {
							// If not exposed and more dirt to place, set dirt
							if dirt_to_place != 0 {
								dirt_to_place -= 1;
								// set to dirt if within this chunk
								if v_chunk_position == chunk_position {
									insert_chunkblockmods(
										&mut block_mods,
										BlockMod {
											position: VoxelPosition::from_chunk_voxel(chunk_position, [x,y,z]),
											reason: BlockModReason::WorldGenSet(Voxel::Block(fill_idx)),
										},
										chunk_size,
									);
								}
							}
						}

						last_was_empty = false;
					}

					
				}
			}
		}

		block_mods
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
		bm: &BlockManager,
	) -> Result<bool, GenerationError> {
		const STAGE_NEEDED: GenerationStage = GenerationStage::Covered;

		let [x, y, z] = world_position;

		// The chunk needs to be at least Covered
		let this_chunk = world_chunk(world_position, map.chunk_size);
		if map.chunk_stage(this_chunk) < STAGE_NEEDED {
			return Err(GenerationError::ChunkStageLtError {
				chunk_position: this_chunk, 
				stage_min: STAGE_NEEDED,
			});
		}

		// Must have voxel as base
		let below_chunk = world_chunk([x, y-1, z], map.chunk_size);
		if below_chunk != this_chunk {
			if map.chunk_stage(below_chunk) < STAGE_NEEDED {
				return Err(GenerationError::ChunkStageLtError {
					chunk_position: below_chunk, 
					stage_min: STAGE_NEEDED,
				});
			}
		}
		let base_voxel = map.get_voxel_world([x, y-1, z]).unwrap();
		
		// No generating on air
		if base_voxel.is_empty() {
			return Ok(false);
		}

		// Must have grass below
		if !(bm.index(base_voxel.unwrap_id()).name == "grass") {
			return Ok(false);
		}

		// Must have ten blocks above
		for i in 0..10 {
			let space_chunk = world_chunk([x, y+i, z], map.chunk_size);
			if space_chunk != this_chunk {
				if map.chunk_stage(space_chunk) < STAGE_NEEDED {
					return Err(GenerationError::ChunkStageLtError {
						chunk_position: space_chunk, 
						stage_min: STAGE_NEEDED,
					});
				}
			}

			if !map.get_voxel_world([x, y+i, z]).unwrap().is_empty() {
				return Ok(false)
			}
		}

		return Ok(true)
	}

	// Todo: only let leaves replace certain voxel values
	// Because each chunk has its won seed this won't generate trees on top of each other unless the chunk size is big enough for that
	pub fn treeify_3d(
		&self,
		chunk_position: [i32; 3], 
		map: &Map,
		bm: &BlockManager,	// Assumed to be the same as the world's bm
		r: u32
	) -> Result<ChunkBlockMods, GenerationError> {
		let mut block_mods = ChunkBlockMods::new();

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
						if self.could_generate_tree([bx + x, by + y, bz + z], map, bm)? {
							let bms = self.place_tree([bx + x, by + y, bz + z], map.chunk_size, bm);
							append_chunkblockmods(&mut block_mods, bms);
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
		bm: &BlockManager,
	) -> ChunkBlockMods {
		let log_idx = bm.index_name(&"log".to_string()).unwrap();
		let leaves_idx = bm.index_name(&"leaves".to_string()).unwrap();

		let mut block_mods = ChunkBlockMods::new();
		let [x, y, z] = world_pos;

		// Trunk
		for i in 0..=5 {
			insert_chunkblockmods(
				&mut block_mods,
				BlockMod {
					position: VoxelPosition::WorldRelative([x, y+i, z]),
					reason: BlockModReason::WorldGenSet(Voxel::Block(log_idx)),
				},
				chunk_size,
			);
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
			let ly = ly as i32;
			for (lx, z_slice) in y_slice.chunks_exact(5).enumerate() {
				let lx = lx as i32 - 2;
				for (lz, &bleaves) in z_slice.iter().enumerate() {
					let lz = lz as i32 - 2;
					if bleaves {
						insert_chunkblockmods(
							&mut block_mods,
							BlockMod {
								position: VoxelPosition::WorldRelative([x+lx, y+ly, z+lz]),
								reason: BlockModReason::WorldGenSet(Voxel::Block(leaves_idx)),
							},
							chunk_size,
						);
					
					}
				}
			}
		}

		block_mods
	}
}



// Helper
fn world_position_stage(world_position: [i32; 3], map: &Map) -> GenerationStage {
	map.chunk_stage(world_chunk(world_position, map.chunk_size))
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
