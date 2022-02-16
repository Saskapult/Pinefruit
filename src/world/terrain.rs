use crate::world::*;
use noise::{NoiseFn, Perlin};



pub struct TerrainGenerator {
	perlin: Perlin,
}
impl TerrainGenerator {
	pub fn new() -> Self {
		Self {
			perlin: Perlin::new(),
		}
	}

	pub fn generate_chunk(
		&self, 
		chunk_position: [i32; 3], 
		mut chunk: Chunk,
		bm: &BlockManager,
	) -> Chunk {
		println!("{:#?}", &bm);
		let grass_idx = bm.index_name(&"grass".to_string()).unwrap();
		let dirt_idx = bm.index_name(&"dirt".to_string()).unwrap();

		for x in 0..chunk.size[0] {
			let x_world = chunk.size[0] as i32 * chunk_position[0] + x as i32;
			for z in 0..chunk.size[2] {
				let z_world = chunk.size[2] as i32 * chunk_position[2] + z as i32;
				
				// let noisy = self.perlin.get([
				// 	x_world as f64 / 10 as f64, 
				// 	z_world as f64 / 10 as f64,
				// ]);

				let noisy = crate::noise::octave_perlin_2d(
					&self.perlin, 
					[
						x_world as f64 / 10 as f64, 
						z_world as f64 / 10 as f64,
					], 
					4, 
					1.0,
				);

				let y_level = (noisy * 4.0) as i32;

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

	pub fn decorate_chunk(&self, chunk: Chunk) -> Chunk {
		chunk
	}
}
