pub mod blocks;
pub mod chunk;
pub mod map;
pub mod terrain;

pub use blocks::*;
pub use chunk::*;
pub use map::*;
pub use terrain::*;



#[cfg(test)]
mod tests {
    use super::*;
	use rand::prelude::*;

	fn randomize_chunk(mut chunk: Chunk, brange: f32) -> Chunk {
		let mut rng = thread_rng();
		for i in 0..chunk.size[0] {
			for j in 0..chunk.size[1] {
				for k in 0..chunk.size[2] {
					let rn = (rng.gen::<f32>() * brange) as usize;
					let voxel = match rn == 0 {
						true => Voxel::Empty,
						false => Voxel::Block(rn -1),
					};
					chunk.set_voxel([i as i32, j as i32, k as i32], voxel)
				}
			}
		}
		chunk
	}

    #[test]
    fn test_chunk_encode_decode() {
		const CHUNKSIZE: [u32; 3] = [16, 16, 16];

        let chunk1 = randomize_chunk(Chunk::new(CHUNKSIZE), 8 as f32);
		let rle = chunk1.rle();
		let chunk2 = Chunk::new(CHUNKSIZE).rld(&rle);

        assert_eq!(chunk1, chunk2);
    }

	#[test]
    fn test_chunk_rle_map() {
		const CHUNKSIZE: [u32; 3] = [2, 2, 2];

		let mut bm = BlockManager::new();
		(0..=8).for_each(|i| {
			bm.insert(Block::new(
				&format!("block {}", i)
			));
		});

        let chunk1 = randomize_chunk(Chunk::new(CHUNKSIZE), 8 as f32);
		
		let rle = chunk1.rle();
		let (name_idx_map, name_map) = bm.encoding_maps(&rle);
		
		let mapped_rle = rle.iter().map(|&(e_id, len)| {
			if e_id == 0 {
				// If it is empty don't map it
				(e_id, len)
			} else {
				// Otherwise find its index in uniques
				println!("map {} ({}) -> {}", e_id, &bm.index(e_id-1).name, name_idx_map[&e_id]);
				let name_idx = name_idx_map[&e_id];
				// Don't map to zero
				(name_idx + 1, len)
			}
		}).collect::<Vec<_>>();
		
		let unmapped_rle = mapped_rle.iter().map(|&(name_idx, len)| {
			if name_idx == 0 {
				// If idx is 0 then it was always 0 and represents empty
				(name_idx, len)
			} else {
				// Unmap from not mapping to zero
				let corrected_name_idx = name_idx - 1;
				let name = &name_map[corrected_name_idx];
				let e_id = bm.index_name(name).unwrap() + 1;

				println!("unmap {} -> {} -> {}", corrected_name_idx, name, e_id);
				(e_id, len)
			}
		}).collect::<Vec<_>>();

        assert_eq!(rle, unmapped_rle);
    }

	#[test]
    fn test_chunk_serde() {
		const CHUNKSIZE: [u32; 3] = [16, 16, 16];

        let mut bm = BlockManager::new();
		(0..=8).for_each(|i| {
			bm.insert(Block::new(
				&format!("block {}", i)
			));
		});

        let chunk1 = randomize_chunk(Chunk::new(CHUNKSIZE), 8 as f32);
		
		let rle = chunk1.rle();
		let (name_idx_map, name_map) = bm.encoding_maps(&rle);
		
		let mapped_rle = rle.iter().map(|&(e_id, len)| {
			if e_id == 0 {
				// If it is empty don't map it
				(e_id, len)
			} else {
				// Otherwise find its index in uniques
				// println!("map {} ({}) -> {}", e_id, &bm.index(e_id-1).name, name_idx_map[&e_id]);
				let name_idx = name_idx_map[&e_id];
				// Don't map to zero
				(name_idx + 1, len)
			}
		}).collect::<Vec<_>>();
		
		let save_path = "/tmp/chunktest.ron";
		{ // Save
			
			let f = std::fs::File::create(&save_path).unwrap();
			ron::ser::to_writer(f, &(name_map, mapped_rle)).unwrap();
		}
		// Read
		let f = std::fs::File::open(&save_path).unwrap();
		let (read_name_map, read_mapped_rle): (Vec<String>, Vec<(usize, u32)>) = ron::de::from_reader(f).unwrap();

		let unmapped_rle = read_mapped_rle.iter().map(|&(name_idx, len)| {
			if name_idx == 0 {
				// If idx is 0 then it was always 0 and represents empty
				(name_idx, len)
			} else {
				// Unmap from not mapping to zero
				let corrected_name_idx = name_idx - 1;
				let name = &read_name_map[corrected_name_idx];
				let e_id = bm.index_name(name).unwrap() + 1;

				// println!("unmap {} -> {} -> {}", corrected_name_idx, name, e_id);
				(e_id, len)
			}
		}).collect::<Vec<_>>();
		let chunk2 = Chunk::new(CHUNKSIZE).rld(&unmapped_rle);

        assert_eq!(chunk1, chunk2);
    }
}
