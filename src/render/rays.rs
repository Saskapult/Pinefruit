use rayon::prelude::*;
use image::DynamicImage;
use crate::world::Map;
use crate::world::VoxelRayHit;
use nalgebra::*;


struct RayPointLight {
	pub position: Vector3<f32>,
	pub radius: f32,
	pub colour: [f32; 3],
}


/// Generates ray directions for each pixel in a thingy
pub fn ray_spread(
	rotation: UnitQuaternion<f32>,
	width: u32, 
	height: u32, 
	fovy: f32,
) -> Vec<Vector3<f32>> {
	let coords = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).collect::<Vec<_>>();

	let near = 1.0 / (fovy.to_radians() / 2.0).tan();
	// println!("near is {near}");
	let directions = coords.iter().map(|&(x, y)| {
		rotation * Vector3::new(
			(((x as f32 + 0.5) / width as f32) - 0.5) * 2.0,
			-(((y as f32 + 0.5) / height as f32) - 0.5) * 2.0,
			near,
		).normalize()
	}).collect::<Vec<_>>();

	directions
}


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



pub fn rendery(
	origin: Vector3<f32>,
	directions: &Vec<Vector3<f32>>,
	albedo: &mut Vec<[f32; 4]>,
	depth: &mut Vec<f32>,
	volume: &crate::octree::Octree<usize>,
	volume_position: Vector3<f32>,
	volume_palette: &Vec<[f32; 4]>,
	distance: f32,
) {
	// let ncpu = 8;
	// Todo: Divide into ncpu chunks and give to par iter

	let new_data = directions.iter().enumerate()
		.filter_map(|(i, &direction)| {
			if let Some((st, en)) = volume.aa_intersect(origin, direction, volume_position, 0.0, distance) {
				// Filter nothingburgers
				if depth[i] < st {
					None
				} else {
					Some((i, direction, (st, en)))
				}
			} else {
				None
			}
		})
		.map(|(i, direction, (st, en))| {
			let hit_pos = origin + direction * (st + 0.05);
			let rel_hit_pos = hit_pos - volume_position;

			let max1 = en - st;
			let max2 = distance - st;

			let mut iiter = crate::render::rays::AWIter::new(
				rel_hit_pos, 
				direction, 
				0.0, 
				f32::min(max1, max2),
				1.0,
			);

			// Mark initial miss as red
			if !volume.in_bounds([iiter.vx, iiter.vy, iiter.vz]) {
				return (i, [f32::MAX, 0.0, 0.0, 0.0], st+iiter.t)
			}
			loop {
				if let Some(&g) = volume.get([iiter.vx, iiter.vy, iiter.vz]) {
					return (i, volume_palette[g], st+iiter.t)
				}

				// Mark out of cast length as green
				if !iiter.next().is_some() {
					return (i, [0.0, f32::MAX, 0.0, 0.0], st+iiter.t)
				}

				// Mark out of bounds as white
				if !volume.in_bounds([iiter.vx, iiter.vy, iiter.vz]) {
					return (i, [f32::MAX, f32::MAX, f32::MAX, 0.0], st+iiter.t)
				}
			}
		}).collect::<Vec<_>>();
	
	for (i, new_albedo, new_depth) in new_data {
		if depth[i] > new_depth {
			depth[i] = new_depth;
			albedo[i] = new_albedo;
		}
	}

}



#[cfg(test)]
mod tests {
	use std::sync::{Arc, RwLock};
	use super::*;
	use crate::{util::*, world::BlockManager, texture::TextureManager, material::{MaterialManager, load_materials_file}};

	#[test]
	fn test_cit() {
		let mut cit = crate::render::rays::AWIter::new(
			Vector3::new(4.0, 4.0, 4.0),
			Vector3::new(1.0, 1.1, 1.2),
			0.0,
			100.0,
			16.0,
		);
		println!("{cit:#?}\n");
		for _ in 0..5 {
			let cp = [cit.vx, cit.vy, cit.vz];
			let t = cit.t;
			println!("{cp:?} ({t})");
			cit.next();
		}
	}

	#[test]
	fn test_show_trace() {

		let fovy = 90.0;
		let aspect = 16.0 / 9.0;
		let width = 512;
		let height = (width as f32 / aspect) as u32;
		
		let position = [1.0, -29.0, 1.0].into();
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

		let directions = ray_spread(rotation, width, height, fovy);
		let albedo_map = |voxel_name: &str| {
			match voxel_name {
				"stone" => [0.5, 0.5, 0.5],
				"grass" => [0.25, 0.5, 0.2],
				"dirt" => [0.25, 0.2, 0.15],
				_ => [1.0; 3],
			}
		};
		let bm = map.blocks.read().unwrap();
		let st = std::time::Instant::now();
		let albedo = directions.par_iter().map(|&direction| {
			match map.ray(position, direction, 100.0) {
				Some((voxel, _t, n)) => {
					let name = &*bm.index(voxel.unwrap_id()).name;
					// println!("Well something's happening...");
					let base = albedo_map(name);

					let g = n.angle(&Vector3::new(1.0, 1.0, 1.0));
					let perc = 1.0 - g / f32::pi();

					base.map(|f| f32::max(f * perc, f / 2.0))
				},
				None => [0.0; 3]
			}
			
		}).collect::<Vec<_>>();
		// let albedo = directions.par_chunks(width as usize).map(|bits| {
		// 	bits.iter().map(|&direction|{
		// 		match map.ray(position, direction, 100.0) {
		// 			Some((voxel, _t, n)) => {
		// 				let name = &*bm.index(voxel.unwrap_id()).name;
		// 				// println!("Well something's happening...");
		// 				let base = albedo_map(name);
	
		// 				let g = n.angle(&Vector3::new(1.0, 1.0, 1.0));
		// 				let perc = 1.0 - g / f32::pi();
	
		// 				base.map(|f| f32::max(f * perc, f / 2.0))
		// 			},
		// 			None => [0.0; 3]
		// 		}
		// 	}).collect::<Vec<_>>()
		// }).flatten().collect::<Vec<_>>();
		let en = std::time::Instant::now();
		println!("Done in {}ms", (en-st).as_millis());
		
		let imb = image::ImageBuffer::from_vec(
			width, 
			height, 
			albedo.iter()
				.flatten()
				.map(|&v| (v * u8::MAX as f32) as u8)
				.collect::<Vec<_>>(),
		).unwrap();
		let img = image::DynamicImage::ImageRgb8(imb);
		println!("Done that, showing!");
		show_image(img).unwrap();
	}

	#[test]
	fn test_awiter() {
		let iiter = AWIter::new(
			Vector3::new(4.5, 4.5, 4.5),
			Vector3::new(-0.5, 0.0, 0.0),
			0.0,
			100.0,
			8.0,
		);
		assert_eq!(4.5, iiter.t_max_x);
	}

	#[test]
	fn test_awiter_2() {
		let iiter = AWIter::new(
			Vector3::new(4.5, 4.5, 4.5),
			Vector3::new(0.5, 0.0, 0.0),
			0.0,
			100.0,
			8.0,
		);

		assert_eq!(3.5, iiter.t_max_x);
	}
}



/// An iterator for Fast Voxel Traversal
#[derive(Debug)]
pub struct AWIter {
	origin: Vector3<f32>,
	direction: Vector3<f32>,
	pub vx: i32,
	pub vy: i32,
	pub vz: i32,
	v_step_x: i32,
	v_step_y: i32,
	v_step_z: i32,
	t_delta_x: f32,
	t_delta_y: f32,
	t_delta_z: f32,
	t_max_x: f32,
	t_max_y: f32,
	t_max_z: f32,
	pub t: f32,
	t_max: f32,
	pub normal: Vector3<f32>,
}
impl AWIter {
	pub fn new(
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		_t_min: f32, // Could do origin = origin + direction * t_min but that loses normal data
		t_max: f32,
		voxel_scale: f32,
	) -> Self {

		if t_max < 0.0 {
			panic!("No.")
		}

		// Origin cell
		let vx = (origin[0] / voxel_scale).floor() as i32;
		let vy = (origin[1] / voxel_scale).floor() as i32; 
		let vz = (origin[2] / voxel_scale).floor() as i32;

		let direction = direction.normalize();
		let dx = direction[0]; 
		let dy = direction[1]; 
		let dz = direction[2];

		let v_step_x = dx.signum() as i32;
		let v_step_y = dy.signum() as i32;
		let v_step_z = dz.signum() as i32;

		let t_delta_x = voxel_scale / dx.abs();
		let t_delta_y = voxel_scale / dy.abs();
		let t_delta_z = voxel_scale / dz.abs();


		let frac = |f: f32, dp: bool| {
			if dp {
				f - f.floor()
			} else {
				1.0 - f + f.floor()
			}
		};
		let t_max_x = t_delta_x * (1.0 - frac(origin[0] / voxel_scale, v_step_x >= 0));
		let t_max_y = t_delta_y * (1.0 - frac(origin[1] / voxel_scale, v_step_y >= 0));
		let t_max_z = t_delta_z * (1.0 - frac(origin[2] / voxel_scale, v_step_z >= 0));

		if t_delta_x == 0.0 && t_delta_y == 0.0 && t_delta_z == 0.0 {
			panic!("This train is going nowhere!")
		}
		if t_delta_x == f32::INFINITY && t_delta_y == f32::INFINITY && t_delta_z == f32::INFINITY {
			panic!("This train is also going nowhere!")
		}

		Self {
			origin,
			direction,
			vx, vy, vz,
			v_step_x, v_step_y, v_step_z,
			t_delta_x, t_delta_y, t_delta_z, 
			t_max_x, t_max_y, t_max_z, 
			t: 0.0,
			t_max,
			normal: Vector3::zeros(),
		}
	}
}
impl Iterator for AWIter {
	type Item = ();

	fn next(&mut self) -> Option<Self::Item> {

		if self.t_max_x < self.t_max_y {
			if self.t_max_x < self.t_max_z {
				self.normal = Vector3::new(-self.v_step_x as f32, 0.0, 0.0);
				self.vx += self.v_step_x;
				self.t = self.t_max_x;
				self.t_max_x += self.t_delta_x;
				
			} else {
				self.normal = Vector3::new(0.0, 0.0, -self.v_step_z as f32);
				self.vz += self.v_step_z;
				self.t = self.t_max_z;
				self.t_max_z += self.t_delta_z;
			}
		} else {
			if self.t_max_y < self.t_max_z {
				self.normal = Vector3::new(0.0, -self.v_step_y as f32, 0.0);
				self.vy += self.v_step_y;
				self.t = self.t_max_y;
				self.t_max_y += self.t_delta_y;
			} else {
				self.normal = Vector3::new(0.0, 0.0, -self.v_step_z as f32);
				self.vz += self.v_step_z;
				self.t = self.t_max_z;
				self.t_max_z += self.t_delta_z;
			}
		}

		if self.t <= self.t_max {
			Some(())
		} else {
			None
		}
	}
}


// #[repr(C)]
// pub struct GPUMap {
// 	max_size: u32,
// 	chunk_size_x: u32,
// 	chunk_size_y: u32,
// 	chunk_size_z: u32,
// 	content: Vec<u64>,
// 	colour_map: Vec<[f32; 4]>,
// }
// impl GPUMap {
// 	const HASH_FN: fn(u32) -> u32 = lowbias32;

// 	pub fn new(
// 		max_size: u32,
// 		chunk_size_x: u32,
// 		chunk_size_y: u32,
// 		chunk_size_z: u32,
// 	) -> Self {
// 		let mut content = Vec::with_capacity(max_size as usize);
// 		content.resize_with(max_size as usize, 0);

// 		Self {
// 			max_size,
// 			chunk_size_x,
// 			chunk_size_y,
// 			chunk_size_z,
// 			content,
// 			colour_map: Vec::new(),
// 		}
// 	}

// 	pub fn insert(&mut self, pos: [i32; 3], data: &Chunk) {
// 		let mut idx = ((
// 			GPUMap::HASH_FN(pos[0] as u32) ^ 
// 			GPUMap::HASH_FN(pos[1] as u32) ^ 
// 			GPUMap::HASH_FN(pos[2] as u32)
// 		) % self.max_size) as usize;
// 		println!("Idx is {idx}");

// 		loop {
// 			if self.content[idx] == 0 {
// 				self.content[idx] = 42;
// 				break;
// 			} else {
// 				idx = (idx + 1) % self.max_size as usize;

// 				idx = 
// 			}
// 		}
// 	}
// }



// https://nullprogram.com/blog/2018/07/31/
fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x *= 0x7feb352d;
    x ^= x >> 15;
    x *= 0x846ca68b;
    x ^= x >> 16;
    x
}
fn triple32(mut x: u32) -> u32 {
    x ^= x >> 17;
    x *= 0xed5ad4bb;
    x ^= x >> 11;
    x *= 0xac4c1b51;
    x ^= x >> 15;
    x *= 0x31848bab;
    x ^= x >> 14;
    x
}
