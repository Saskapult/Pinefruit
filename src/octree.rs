use nalgebra::*;


#[derive(Debug, Clone)]
struct AABB {
	p0: Vector3<f32>,
	p1: Vector3<f32>,
	centre: Vector3<f32>,
}
impl AABB {
	pub fn new(
		p0: Vector3<f32>,
		p1: Vector3<f32>,
	) -> Self {
		Self {
			p0, p1,
			centre: p0 + p1,
		}
	}

	// Todo: handle div by nzero
	// https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-box-intersection
	#[inline]
	pub fn ray_intersect(
		&self, 
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		position: Vector3<f32>, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<(f32, f32)> {
		let v_max = self.p1 + position;
		let v_min = self.p0 + position;

		let (mut t_min, mut t_max) = {
			let t_min = (v_min[0] - origin[0]) / direction[0];
			let t_max = (v_max[0] - origin[0]) / direction[0];

			if t_min < t_max {
				(t_min, t_max)
			} else {
				(t_max, t_min)
			}
		};

		let (ty_min, ty_max) = {
			let ty_min = (v_min[1] - origin[1]) / direction[1];
			let ty_max = (v_max[1] - origin[1]) / direction[1];

			if ty_min < ty_max {
				(ty_min, ty_max)
			} else {
				(ty_max, ty_min)
			}
		};

		if t_min > ty_max || ty_min > t_max {
			return None
		}

		if ty_min > t_min {
			t_min = ty_min;
		}
		if ty_max < t_max {
			t_max = ty_max;
		}

		let (tz_min, tz_max) = {
			let tz_min = (v_min[2] - origin[2]) / direction[2];
			let tz_max = (v_max[2] - origin[2]) / direction[2];

			if tz_min < tz_max {
				(tz_min, tz_max)
			} else {
				(tz_max, tz_min)
			}
		};

		if t_min > tz_max || tz_min > t_max {
			return None
		}

		if tz_min > t_min {
			t_min = tz_min;
		}
		if tz_max < t_max {
			t_max = tz_max;
		}
		
		if (t_min < t1) && (t_max > t0) {
			Some((t_min, t_max))
		} else {
			None
		}
	}

	pub fn contains(&self, point: Vector3<f32>) -> bool {
		point >= self.p0 && point <= self.p1

		// point[0] >= self.p0[0] &&
		// point[1] >= self.p0[1] &&
		// point[2] >= self.p0[2] &&
		// point[0] <= self.p1[0] &&
		// point[1] <= self.p1[1] &&
		// point[2] <= self.p1[2]
	}

	pub fn mid_planes(&self) -> [Plane; 3] {
		[
			Plane {
				normal: *Vector3::z_axis(),
				distance: self.centre[2],
			},
			Plane {
				normal: *Vector3::y_axis(),
				distance: self.centre[1],
			},
			Plane {
				normal: *Vector3::x_axis(),
				distance: self.centre[1],
			},
		]
	}
}


#[derive(Debug, Clone)]
struct Plane {
	pub normal: Vector3<f32>,
	pub distance: f32,
}
impl Plane {
	// Restricted to along positive line direction
	pub fn ray_intersect(
		&self, 
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		position: Vector3<f32>, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<f32> {
		let d = self.normal.dot(&direction);
		if d > f32::EPSILON {
			let g = position - origin;
			let t = g.dot(&self.normal) / d;
			if t > t0 && t < t1 {
				return Some(t)
			}
		}
		None
	}
}


/// An octree which can holds Option<T>.
/// This implmentation is meant to be built upward.
/// 
/// The octree has a width dependent on its depth.
/// The side length is equal to 2^depth units.
#[derive(Debug, Clone)]
pub struct Octree<T: PartialEq + Clone + std::fmt::Debug> {
	pub data: Vec<T>,
	pub nodes: Vec<OctreeNode>,

	pub depth: u16,
}
impl<T: PartialEq + Clone + std::fmt::Debug> Octree<T> {
	pub fn base(content: Option<T>, depth: u16) -> Self {
		let (data, content) = match content {
			Some(c) => (vec![c], 1),
			None => (vec![], 0),
		};
		Self {
			data,
			nodes: vec![OctreeNode {
				octants: [0; 8],
				content,
			}],
			depth,
		}
	}

	pub fn root(&self) -> &OctreeNode {
		&self.nodes[0]
	}

	/// Combines a bunch of octrees into a bigger octree
	// This function uses too many clone()s, please make it better-er
	pub fn combine(
		nnn: Octree<T>,
		nnp: Octree<T>,
		npn: Octree<T>,
		npp: Octree<T>,
		pnn: Octree<T>,
		pnp: Octree<T>,
		ppn: Octree<T>,
		ppp: Octree<T>,
		mix_fn: &dyn Fn(&[Option<&T>]) -> Option<T>
	) -> Self {
		
		let octant_depth = ppp.depth;
		let mut octants = vec![nnn, nnp, npn, npp, pnn, pnp, ppn, ppp];
		// Test that same depth for all
		assert!(octants.iter().all(|g| g.depth == octant_depth), "Octants are of differing depth!");

		// If all are of same content and have no children then combine
		let oc = octants[0].root().content_index().and_then(|ci| Some(&octants[0].data[ci]));
		let same_content = octants.iter().all(|o| oc == o.root().content_index().and_then(|ci| Some(&o.data[ci])));
		let all_leaves = octants.iter().all(|o| {
			o.root().octants.iter().all(|&v| v == 0)
		});
		if same_content && all_leaves {
			// println!("WE CAN COMBINE THEM!");
			return Self {
				data: octants[0].data.clone(),
				nodes: vec![OctreeNode {
					octants: [0; 8],
					content: octants[0].root().content,
				}],
				depth: octant_depth + 1,
			}
		}

		// Collect new data, adjusting octant graphs to point to it
		let mut new_data = Vec::new();
		octants.iter_mut().for_each(|o| {
			for (i, data) in o.data.iter().enumerate() {
				// What it would be referenced as in the octree
				let old_idx = i as u16 + 1;

				// What it should be referenced as now
				let new_idx = new_data.iter()
					.position(|x| x == data)
					.unwrap_or_else(|| {
						let idx = new_data.len();
						new_data.push(data.clone());
						idx
					}) as u16 + 1;

				// Traverse tree if needs adjustment
				if old_idx != new_idx {
					// println!("Data index reference {} -> {}", old_idx, new_idx);
					// Adjust index
					for node in o.nodes.iter_mut() {
						if node.content == old_idx {
							node.content = new_idx;
						}
					}
				}
			}
		});

		// For each octant adjust its node indices and merge them nodes with the existing nodes
		// println!("Doing node stuff");
		// Init with dummy root because it works better this way
		let mut new_nodes = vec![OctreeNode {
			octants: [0; 8],
			content: 0,
		}];
		let mut octant_indices = Vec::new();
		octants.into_iter().for_each(|mut o| {

			// Adjust node indices
			let seg_st = new_nodes.len() as u32;
			// println!("Start is {seg_st}");
			o.nodes.iter_mut().for_each(|node| {
				node.octants = node.octants.map(|v| {
					if v != 0 {
						v + seg_st
					} else {
						v
					}
				});
			});
			
			// Insert into tree
			octant_indices.push(seg_st + 1);
			new_nodes.extend_from_slice(&o.nodes[..]);
		});

		// Get contents of octants from mixing function
		let top_contents = octant_indices.iter().map(|&i| {
			let o = &new_nodes[i as usize - 1];
			o.content_index().and_then(|i| Some(&new_data[i]))
		}).collect::<Vec<_>>();
		let new_content = mix_fn(&top_contents[..]);
		let content = if let Some(new_content) = new_content {
			new_data.iter().position(|x| *x == new_content).unwrap_or_else(|| {
				let idx = new_data.len();
				new_data.push(new_content);
				idx
			}) as u16 + 1
		} else {
			0
		};

		// Initialize root
		let root = &mut new_nodes[0];
		root.octants = octant_indices.try_into().unwrap();
		root.content = content;

		Self {
			data: new_data,
			nodes: new_nodes,
			depth: octant_depth + 1,
		}
	}

	pub fn print_test(&self) -> String {
		
		fn node_printer<T: std::fmt::Debug>(
			data: &Vec<T>, 
			nodes: &Vec<OctreeNode>, 
			node: &OctreeNode, 
			indent: u32,
		) -> String {

			let mut s = format!(
				"Node containing {:?}", 
				node.content_index().and_then(|ci| Some(&data[ci])),
			);

			for (i, &octant) in node.octants.iter().enumerate() {
				if octant == 0 {
					continue
				}
				// println!("recurse to octant {octant}");
				let next_node = &nodes[octant as usize - 1];
				
				s = format!(
					"{s}\n{:_<indent$}{i}: {}", "",
					node_printer(data, nodes, next_node, indent + 1),
					indent = indent as usize,
				);
			}

			s
		}

		format!(
			"Octree of depth {} with content {}", 
			self.depth, 
			node_printer(&self.data, &self.nodes, self.root(), 1)
		)
	}

	// Todo: handle div by nzero
	// https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-box-intersection
	#[inline]
	pub fn aa_intersect(
		&self, 
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		position: Vector3<f32>, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<(f32, f32)> {
		let side_length = 2_u32.pow(self.depth as u32);
		let oct_max = side_length as f32;
		let oct_min = 0.0;
		let aabb = AABB::new(
			Vector3::new(oct_min, oct_min, oct_min),
			Vector3::new(oct_max, oct_max, oct_max),
		);
		aabb.ray_intersect(origin, direction, position, t0, t1)
	}

	// https://daeken.svbtle.com/a-stupidly-simple-fast-octree-traversal-for-ray-intersection
	// https://lsi2.ugr.es/curena/inves/wscg00/revelles-wscg00.pdf

	pub fn get_size(&self) -> usize {
		std::mem::size_of::<Self>() 
		+
		std::mem::size_of::<OctreeNode>() * self.nodes.len() 
		+
		std::mem::size_of::<T>() * self.data.len()
	}

	pub fn in_bounds(&self, coords: [i32; 3]) -> bool {
		let edge_len = 2_i32.pow(self.depth as u32);	
		let [cx, cy, cz] = coords;
		cx < edge_len && cx >= 0 && cy < edge_len && cy >= 0 && cz < edge_len && cz >= 0
	}

	pub fn get(&self, coords: [i32; 3]) -> Option<&T> {
		// println!("Getting {coords:?}");

		if !self.in_bounds(coords) {
			println!("{coords:?} is out of bounds!");
			return None
		}

		let mut curr_node = self.root();
		let [mut cx, mut cy, mut cz] = coords;
		let mut half_edge_len = 2_i32.pow(self.depth as u32) / 2;
		loop {
			// println!("Iter with half edge len {half_edge_len} and coords [{cx}, {cy}, {cz}]");

			let xp = cx >= half_edge_len;
			let yp = cy >= half_edge_len;
			let zp = cz >= half_edge_len;

			let idx = if xp {4} else {0} + if yp {2} else {0} + if zp {1} else {0};
			// println!("idx is {idx}");
			let target = curr_node.octants[idx];
			// println!("Target is {target}");
			if target == 0 {
				break;
			}

			curr_node = &self.nodes[target as usize - 1];
			if xp { cx -= half_edge_len; }
			if yp { cy -= half_edge_len; }
			if zp { cz -= half_edge_len; }

			half_edge_len /= 2;
		}
		// println!("Leaf!");

		if curr_node.content > 0 {
			Some(&self.data[curr_node.content as usize - 1])
		} else {
			None
		}
	}
}



#[derive(Clone, Copy, Debug)]
pub struct OctreeNode {
	// Order is described by 'An Efficient Parametric Algorithm for Octree Traversal'
	// 0 - nnn
	// 1 - nnp
	// 2 - npn
	// 3 - npp
	// 4 - pnn
	// 5 - pnp
	// 6 - ppn
	// 7 - ppp
	// Index to (blah) is ( if i == 0 { None } else { Some(i-1) } )
	pub octants: [u32; 8],	// Empty if 0 in this case
	pub content: u16,	// Empty if 0 in this case too!
}
impl OctreeNode {
	pub fn content_index(&self) -> Option<usize> {
		if self.content != 0 {
			Some(self.content as usize - 1)
		} else {
			None
		}
	}
}



/// Returns Some((t_min, t_max)) if there is an intersection
// https://people.csail.mit.edu/amy/papers/box-jgt.pdf
#[inline]
fn box_jgt(
	origin: Vector3<f32>,
	// direction: Vector3<f32>,
	direction_inverse: Vector3<f32>,
	direction_sign: Vector3<usize>,		// 0 is neg and 1 is pos?
	t0: f32,	// Ray min distance
	t1: f32,	// Ray max distance
	bounds: [Vector3<f32>; 2],	// [vec3; 2] st (bounds[0] < bounds[1])
) -> Option<(f32, f32)> {
	let mut t_min = (bounds[direction_sign[0]][0] - origin[0]) * direction_inverse[0]; 
	let mut t_max = (bounds[1-direction_sign[0]][0] - origin[0]) * direction_inverse[0]; 

	let t_min_y = (bounds[direction_sign[1]][1] - origin[1]) * direction_inverse[1]; 
	let t_max_y = (bounds[1-direction_sign[1]][1] - origin[1]) * direction_inverse[1]; 

	if (t_min > t_max_y) || (t_min_y > t_max) {
		return None;
	}
	if t_min_y > t_min {
		t_min = t_min_y;
	}
	if t_max_y < t_max {
		t_max = t_max_y;
	}

	let t_min_z = (bounds[direction_sign[2]][2] - origin[2]) * direction_inverse[2]; 
	let t_max_z = (bounds[1-direction_sign[2]][2] - origin[2]) * direction_inverse[2]; 

	if (t_min > t_max_z) || (t_min_z > t_max) {
		return None;
	}
	if t_min_z > t_min {
		t_min = t_min_z;
	}
	if t_max_z < t_max {
		t_max = t_max_z;
	}

	if (t_min < t1) && (t_max > t0) {
		Some((t_min, t_max))
	} else {
		None
	}
}



/// Converts a chunk to an octree
// Should this be in Chunk?
pub fn chunk_to_octree(chunk: &crate::world::Chunk) -> Option<Octree<usize>> {
	// Check all size is same
	assert_eq!(chunk.size[0], chunk.size[1]);
	assert_eq!(chunk.size[1], chunk.size[2]);

	// Check size is pow of 2
	let lsf32 = (chunk.size[0] as f32).log2();
	assert!(lsf32.floor() == lsf32.ceil());

	let mut dim = chunk.size[0];
	// Construct initial size thingy
	// Could group directly into depth 1 trees for greater speed
	let mut trees = (0..dim).flat_map(|x| {
		let x = x * dim.pow(2);
		(0..dim).flat_map(move |y| {
			let y = y * dim;
			(0..dim).map(move |z| {
				Octree::base(chunk.contents[(x + y + z) as usize].id(), 0)
			})
		})
	}).collect::<Vec<_>>();

	let mix_fn = |_: &[Option<&usize>]| Some(0_usize);

	// Reduce until one octree remains
	while dim != 1 {
		let mut reduced_trees = Vec::new();

		for x in (0..dim).step_by(2) {
			let xn = x * dim * dim;
			let xp = (x+1) * dim * dim;
			for y in (0..dim).step_by(2) {
				let yn = y * dim;
				let yp = (y+1) * dim;
				for z in (0..dim).step_by(2) {
					let zn = z;
					let zp = z+1;

					let nnn = trees[(xn + yn + zn) as usize].clone();
					let nnp = trees[(xn + yn + zp) as usize].clone();
					let npn = trees[(xn + yp + zn) as usize].clone();
					let npp = trees[(xn + yp + zp) as usize].clone();
					let pnn = trees[(xp + yn + zn) as usize].clone();
					let pnp = trees[(xp + yn + zp) as usize].clone();
					let ppn = trees[(xp + yp + zn) as usize].clone();
					let ppp = trees[(xp + yp + zp) as usize].clone();
					
					
					reduced_trees.push(Octree::combine(nnn, nnp, npn, npp, pnn, pnp, ppn, ppp, &mix_fn));
				}
			}
		}

		trees = reduced_trees;
		dim /= 2;
	}

	Some(trees[0].clone())
}



#[cfg(test)]
mod tests {
	// use std::mem::size_of;
	use super::*;

	#[test]
	fn test_octree_combine_get() {
		let octree1 = Octree::base(Some(4_i32), 0);
		assert!(octree1.get([0,0,0]) == Some(&4));
		// println!("{}", octree1.print_test());

		let mix_fn = |_: &[Option<&i32>]| Some(8_i32);
		let octree2 = Octree::combine(
			Octree::base(Some(0_i32), 0), 
			Octree::base(Some(1_i32), 0),
			Octree::base(Some(2_i32), 0), 
			Octree::base(Some(3_i32), 0),
			Octree::base(Some(4_i32), 0), 
			Octree::base(Some(5_i32), 0),
			Octree::base(Some(6_i32), 0), 
			Octree::base(Some(7_i32), 0),
			&mix_fn,
		);
		// println!("{:#?}", octree2.nodes);
		// println!("{}", octree2.print_test());
		let g = octree2.get([0,0,0]);
		assert!(g == Some(&0), "g is {g:?}");
		// assert!(octree2.get([0,0,0]) == Some(&0));
		let g = octree2.get([0,0,1]);
		assert!(g == Some(&1), "g is {g:?}");
		assert!(octree2.get([0,1,0]) == Some(&2));
		assert!(octree2.get([0,1,1]) == Some(&3));
		assert!(octree2.get([1,0,0]) == Some(&4));
		assert!(octree2.get([1,0,1]) == Some(&5));
		assert!(octree2.get([1,1,0]) == Some(&6));
		assert!(octree2.get([1,1,1]) == Some(&7));


		let octree3 = Octree::combine(
			octree2.clone(),
			Octree::base(Some(42_i32), 1),
			Octree::base(Some(43_i32), 1), 
			Octree::base(Some(44_i32), 1),
			Octree::base(Some(45_i32), 1), 
			Octree::base(Some(46_i32), 1),
			Octree::base(None, 1), 
			Octree::base(None, 1), 
			&mix_fn,
		);
		assert!(octree3.get([0,0,0]) == Some(&0));
		assert!(octree3.get([0,0,1]) == Some(&1));
		assert!(octree3.get([0,1,0]) == Some(&2));
		assert!(octree3.get([0,1,1]) == Some(&3));
		assert!(octree3.get([1,0,0]) == Some(&4));
		assert!(octree3.get([1,0,1]) == Some(&5));
		assert!(octree3.get([1,1,0]) == Some(&6));
		assert!(octree3.get([1,1,1]) == Some(&7));

		assert!(octree3.get([3,2,3]) == None);
		assert!(octree3.get([3,3,3]) == None);

		println!("{}", octree3.print_test());
	}

	#[test]
	fn test_octree_chunk_sizes() {
		const CHUNK_SIZE: u32 = 32;
		const WORLD_SQUARE_RADIUS: i32 = 2;

		let mut chunk = crate::world::Chunk::new([CHUNK_SIZE; 3]);
		chunk.contents = (0..(32*32*32)).map(|_| {
			let i = (rand::random::<f32>() * 8.0).floor() as usize;
			if i > 0 {
				crate::world::Voxel::Block(i-1)
			} else {
				crate::world::Voxel::Empty
			}
		}).collect::<Vec<_>>();

		let octree = chunk_to_octree(&chunk).unwrap();

		let tgen = crate::world::TerrainGenerator::new(0);
		let chunk2 = tgen.chunk_base_3d(
			[0, -1, 0],
			crate::world::Chunk::new([CHUNK_SIZE; 3]),
			crate::world::Voxel::Block(0),
		);
		let octree2 = chunk_to_octree(&chunk2).unwrap();

		let chunk3 = tgen.chunk_base_3d(
			[0, 3, 0],
			crate::world::Chunk::new([CHUNK_SIZE; 3]),
			crate::world::Voxel::Block(0),
		);
		let octree3 = chunk_to_octree(&chunk3).unwrap();

		let theory_chunk_size: usize = CHUNK_SIZE.pow(3) as usize * std::mem::size_of::<crate::world::Voxel>() + std::mem::size_of::<crate::world::Chunk>();
		println!("Array chunk size:		{:>16} bytes ({:.2}%)", theory_chunk_size, 100.0);

		let max_octree_size = 
		std::mem::size_of::<Octree<usize>>() 
		+ 
		(8_usize.pow(CHUNK_SIZE.log2() + 1) - 1) * (std::mem::size_of::<OctreeNode>() + std::mem::size_of::<usize>());
		let op = max_octree_size as f32 / theory_chunk_size as f32 * 100.0;
		println!("Max octree size:		{:>16} bytes ({:.2}%)", max_octree_size, op);

		let ors = octree.get_size();
		let orsp = ors as f32 / theory_chunk_size as f32 * 100.0;
		println!("With random(8) size:	{:>16} bytes ({:.2}%)", ors, orsp);

		let ots = octree2.get_size();
		let otsp = ots as f32 / theory_chunk_size as f32 * 100.0;
		println!("With terrain size:	{:>16} bytes ({:.2}%)", ots, otsp);

		let oes = octree3.get_size();
		let oesp = oes as f32 / theory_chunk_size as f32 * 100.0;
		println!("Empty size:			{:>16} bytes ({:.2}%)", oes, oesp);

		// return;

		// let coordinates = (-WORLD_SQUARE_RADIUS..=WORLD_SQUARE_RADIUS).flat_map(|x| {
		// 	(-WORLD_SQUARE_RADIUS..=WORLD_SQUARE_RADIUS).flat_map(move |y| {
		// 		(-WORLD_SQUARE_RADIUS..=WORLD_SQUARE_RADIUS).map(move |z| {
		// 			[x, y, z]
		// 		})
		// 	})
		// }).collect::<Vec<_>>();

		// use rayon::prelude::*;
		// let chunks = coordinates.par_iter().map(|&cp| {
		// 	let c = tgen.chunk_base_3d(
		// 		cp,
		// 		crate::world::Chunk::new([CHUNK_SIZE; 3]),
		// 		crate::world::Voxel::Block(0),
		// 	);
		// 	tgen.cover_chunk(
		// 		c, 
		// 		cp, 
		// 		crate::world::Voxel::Block(1),
		// 		crate::world::Voxel::Block(2),
		// 		3,
		// 	)
		// }).collect::<Vec<_>>();

		// let octrees = chunks.par_iter().map(|c| {
		// 	chunk_to_octree(c).unwrap()
		// }).collect::<Vec<_>>();

		// let total_chunks_size = theory_chunk_size * coordinates.len();
		// let total_octree_size: usize = octrees.iter().map(|o| o.get_size()).sum();

		// println!("---");
		// println!("Map arrays size:		{:>16} bytes ({:.2}%)", total_chunks_size, 100.0);
		// println!("Map octrees size:		{:>16} bytes ({:.2}%)", total_octree_size, total_octree_size as f32 / total_chunks_size as f32 * 100.0);

		// assert!(chunk == decoded_chunk);
	}


	#[test]
	fn test_octree_get() {
		const CHUNK_SIZE: u32 = 4;
		const N_BLOCKS: u32 = 8;

		let mut chunk = crate::world::Chunk::new([CHUNK_SIZE; 3]);
		chunk.contents = (0..(N_BLOCKS.pow(3))).map(|_| {
			let i = (rand::random::<f32>() * N_BLOCKS as f32).floor() as usize;
			if i > 0 {
				crate::world::Voxel::Block(i-1)
			} else {
				crate::world::Voxel::Empty
			}
		}).collect::<Vec<_>>();

		let octree = chunk_to_octree(&chunk).unwrap();

		println!("{}", octree.print_test());

		for x in 0..(CHUNK_SIZE as i32) {
			for y in 0..(CHUNK_SIZE as i32) {
				for z in 0..(CHUNK_SIZE as i32) {
					let coords = [x,y,z];
					let cv = chunk.get_voxel(coords).id();
					let ov = octree.get(coords).and_then(|u| Some(*u));
					assert_eq!(cv, ov, "different at {coords:?}");
				}
			}
		}
	}

	// #[test]
	// fn test_size() {
		
	// 	println!("size of [f32;4] = {}", size_of::<[f32; 4]>());
	// 	println!("size of OctreeNodeType<[f32;4]> = {}", size_of::<OctreeNodeType<[f32; 4]>>());

	// 	println!("size of bool = {}", size_of::<bool>());
	// 	println!("size of OctreeNodeType<bool> = {}", size_of::<OctreeNodeType<bool>>());

	// 	println!("size of u8 = {}", size_of::<u8>());
	// 	println!("size of OctreeNodeType<u8> = {}", size_of::<OctreeNodeType<u8>>());

	// 	assert!(true);
	// }

	#[test]
	fn test_octree_trace() {

		let width = 400;
		let height = 300;
		let fovy = 90.0;
		let n_blocks = 8;
		let chunk_size = 16;

		// Map block colours
		let palette = (0..n_blocks).map(|_| [rand::random::<f32>(), rand::random::<f32>(), rand::random::<f32>(), 0.0]).collect::<Vec<_>>();

		// Make contents
		let mut chunk = crate::world::Chunk::new([chunk_size; 3]);
		for x in 0..chunk_size as i32 {
			for y in 0..chunk_size as i32 {
				for z in 0..chunk_size as i32 {
					if (x - chunk_size as i32 / 2).pow(2) + (y - chunk_size as i32 / 2).pow(2) + (z - chunk_size as i32 / 2).pow(2) <= 6_i32.pow(2) {
						chunk.set_voxel([x, y, z], crate::world::Voxel::Block(0));
					}
				}
			}
		}
		let octree = chunk_to_octree(&chunk).unwrap();

		// Make hits
		let origin = Vector3::new(0.0, 0.0, 0.0);
		let directions = crate::render::rays::ray_spread(
			UnitQuaternion::identity(), 
			width, 
			height, 
			fovy,
		);
		let octree_position = Vector3::new(
			chunk_size as f32 / -2.0, chunk_size as f32 / -2.0, chunk_size as f32,
		);

		// Make textures
		let mut albedo = vec![[0.0; 4]; (width*height) as usize];
		let mut depth = vec![f32::MAX; (width*height) as usize];

		let st = std::time::Instant::now();
		crate::render::rays::rendery(
			origin,
			&directions,
			&mut albedo,
			&mut depth,
			&octree,
			octree_position,
			&palette,
			100.0,
		);
		let en = std::time::Instant::now();
		println!("Rendered in {}ms", (en-st).as_millis());

		let buf = albedo.iter()
			.map(|&[r,g,b,_]| [r,g,b])
			.map(|c| c.map(|c| (c / f32::MAX * u8::MAX as f32) as u8))
			.flatten()
			.collect::<Vec<_>>();
		let imb = image::ImageBuffer::from_vec(
			width, height, buf,
		).unwrap();
		let img = image::DynamicImage::ImageRgb8(imb);

		crate::util::show_image(img).unwrap();

		// crate::util::save_image(img, &"/tmp/_tset.png").unwrap();

	}
}
