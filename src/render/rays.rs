use rayon::prelude::*;
use image::DynamicImage;
use crate::world::Map;
use crate::world::VoxelRayHit;
use nalgebra::*;




pub fn map_trace(
	map: &Map, 
	position: Vector3<f32>, 
	rotation: UnitQuaternion<f32>,
	width: u32, 
	height: u32, 
	fovy: f32,
) -> DynamicImage {
	let coords = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).collect::<Vec<_>>();
	// coords.chunks(width as usize).for_each(|row| println!("{:?}", row));

	let near = 1.0 / (fovy.to_radians() / 2.0).tan();
	println!("near is {near}");
	let directions = coords.iter().map(|&(x, y)| {
		rotation * Vector3::new(
			(((x as f32 + 0.5) / width as f32) - 0.5) * 2.0,
			-(((y as f32 + 0.5) / height as f32) - 0.5) * 2.0,
			near,
		).normalize()
	}).collect::<Vec<_>>();
	// directions.chunks(width as usize).for_each(|row| 
	// 	println!("{}",
	// 	row.iter().map(|v| format!("[{:>4.1}, {:>4.1}, {:>4.1}]", v[0], v[1], v[2]))
	// 		.collect::<Vec<_>>()
	// 		.join(", ")
	// 	)
	// );

	let albedo_map = |voxel_name: &str| {
		match voxel_name {
			"stone" => [0.5, 0.5, 0.5].map(|v| (v * u8::MAX as f32) as u8),
			"grass" => [65, 125, 55],
			"dirt" => [61, 47, 40],
			_ => [u8::MAX; 3],
		}
	};

	let bm = map.blocks.read().unwrap();
	let albedo = directions.par_iter().map(|&direction| {
		match map_ray(map, position, direction, 100.0) {
			Some(hit) => {
				let block = map.get_voxel_world(hit.coords).unwrap();
				let name = &*bm.index(block.unwrap_id()).name;
				albedo_map(name)
				// [u8::MAX; 3]
			},
			None => [0; 3]
		}
	}).flatten().collect::<Vec<_>>();
	
	let imb = image::ImageBuffer::from_vec(width, height, albedo).unwrap();
	image::DynamicImage::ImageRgb8(imb)
}



fn map_ray(
	map: &Map,
	origin: Vector3<f32>, 
	direction: Vector3<f32>,
	t_limit: f32,
) -> Option<VoxelRayHit> {
	let mut vx = origin[0].floor() as i32;
	let mut vy = origin[1].floor() as i32; 
	let mut vz = origin[2].floor() as i32;

	let direction = direction.normalize();
	let dx = direction[0]; 
	let dy = direction[1]; 
	let dz = direction[2];

	let v_step_x = dx.signum() as i32;
	let v_step_y = dy.signum() as i32;
	let v_step_z = dz.signum() as i32;

	let t_delta_x = 1.0 / dx.abs();
	let t_delta_y = 1.0 / dy.abs();
	let t_delta_z = 1.0 / dz.abs();

	let dist = |i: i32, p: f32, vs: i32| {
		if vs > 0 {
			i as f32 + 1.0 - p
		} else {
			p - i as f32
		}
	};
	let mut t_max_x = t_delta_x * dist(vx, origin[0], v_step_x);
	let mut t_max_y = t_delta_y * dist(vy, origin[1], v_step_y);
	let mut t_max_z = t_delta_z * dist(vz, origin[2], v_step_z);

	if t_delta_x == 0.0 && t_delta_y == 0.0 && t_delta_z == 0.0 {
		panic!()
	}

	let mut t = 0.0;
	let mut normal = Vector3::zeros();
	while t < t_limit {

		if let Some(v) = map.get_voxel_world([vx, vy, vz]).ok() {
			if !v.is_empty() {
				return Some(VoxelRayHit {
					coords: [vx, vy, vz],
					t,
					normal,
					face_coords: [0.0; 2],
				});
			}
		}

		if t_max_x < t_max_y {
			if t_max_x < t_max_z {
				normal = vector![-v_step_x as f32, 0.0, 0.0];
				vx += v_step_x;
				t = t_max_x;
				t_max_x += t_delta_x;
				
			} else {
				normal = vector![0.0, 0.0, -v_step_z as f32];
				vz += v_step_z;
				t = t_max_z;
				t_max_z += t_delta_z;
			}
		} else {
			if t_max_y < t_max_z {
				normal = vector![0.0, -v_step_y as f32, 0.0];
				vy += v_step_y;
				t = t_max_y;
				t_max_y += t_delta_y;
			} else {
				normal = vector![0.0, 0.0, -v_step_z as f32];
				vz += v_step_z;
				t = t_max_z;
				t_max_z += t_delta_z;
			}
		}
	}

	None
}




#[cfg(test)]
mod tests {
	use std::sync::{Arc, RwLock};
	use super::*;
	use crate::{util::*, world::BlockManager, texture::TextureManager, material::{MaterialManager, load_materials_file}};

	#[test]
	fn test_show_trace() {

		let fovy = 90.0;
		let aspect = 16.0 / 9.0;
		let width = 512;
		let height = (width as f32 / aspect) as u32;
		
		let position = [0.0, -29.0, 0.0].into();
		let rotation = UnitQuaternion::identity();
		
		let mut tm = TextureManager::new();
		let mut mm = MaterialManager::new();
		load_materials_file(
			"resources/materials/kmaterials.ron",
			&mut tm,
			&mut mm,
		).unwrap();
		let bm = {
			let mut bm = BlockManager::new();

			crate::world::blocks::load_blocks_file(
				"resources/kblocks.ron",
				&mut bm,
				&mut tm,
				&mut mm,
			).unwrap();
			
			Arc::new(RwLock::new(bm))
		};
		
		let mut map = Map::new([16; 3], &bm);
		let xs = 3;
		let zs = 3;
		for cx in -xs..=xs {
			for cz in -zs..=zs {
				for cy in -5..=2 {
					map.begin_chunk_generation([cx, cy, cz]);
				}
			}
		}
		println!("Waiting for map generation");
		let mut done = false;
		while !done {
			done = true;
			for cx in -xs..=xs {
				for cz in -zs..=zs {
					for cy in -5..=2 {
						if !map.check_chunk_available([cx, cy, cz]) {
							done = false;
						}
					}
				}
			}
		}
		println!("Done that, tracing!");

		let i = map_trace(
			&map, 
			position, 
			rotation,
			width, 
			height, 
			fovy,
		);
		println!("Done that, showing!");
		show_image(i).unwrap();

		assert!(true);
	}
}


