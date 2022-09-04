use std::collections::HashMap;

use nalgebra::*;
use crate::rays::AABB;

/// dd delete line
/// p paste
/// ci( change inside ()


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
			Some(c) => (vec![c], Some(0)),
			None => (vec![], None),
		};
		Self {
			data,
			nodes: vec![OctreeNode::new_leaf(content)],
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
		mix_fn: &dyn Fn(&[Option<&T>]) -> Option<T>,
	) -> Self {
		
		let octant_depth = ppp.depth;
		let mut octants = vec![nnn, nnp, npn, npp, pnn, pnp, ppn, ppp];
		// Test that same depth for all
		assert!(octants.iter().all(|g| g.depth == octant_depth), "Octants are of differing depth!");

		// See if they can be combined
		let all_leaves = octants.iter().all(|o| o.root().leaf_mask == 0xFF);
		if all_leaves {
			
			// If octant has constant content then what is that content
			let constant_content = octants.iter().map(|o| {
				let initial = o.root().get_octant_leaf_idx(1);
				let constant = (1..8).map(|i| o.root().get_octant_leaf_idx(i)).all(|v| v == initial);
				if constant {
					Some(initial.and_then(|ci| Some(&o.data[ci as usize])))
				} else {
					None
				}
			});
			// If all octants have constant content then get vec of their content
			let constant_content_content = constant_content.collect::<Option<Vec<_>>>();
			if let Some(octant_content) = constant_content_content {
				// println!("WE CAN COMBINE THEM!");
				let mut data = Vec::new();
				
				// Add or search for octant content
				let octant_content_index = octant_content.iter().map(|c| {
					c.and_then(|c| {
						let idx = data.iter().position(|v| v == c).unwrap_or_else(|| {
							let i = data.len();
							data.push(c.clone());
							i
						});
						Some(idx + 1)
					}).unwrap_or(0) as u32
				}).collect::<Vec<_>>();
				// Add or search for combined content
				let new_base_content = mix_fn(&octant_content[..]);
				let new_base_content_index = new_base_content.and_then(|c| {
					let idx = data.iter().position(|v| *v == c).unwrap_or_else(|| {
						let i = data.len();
						data.push(c);
						i
					});
					Some(idx + 1)
				}).unwrap_or(0) as u32;

				return Self {
					data,
					nodes: vec![OctreeNode {
						octants: octant_content_index.try_into().unwrap(),
						leaf_mask: 0xFF,
						content: new_base_content_index,
					}],
					depth: octant_depth + 1,
				}
			}
		}

		// Collect new data, adjusting leaves to point to it
		let mut new_data = Vec::new();
		octants.iter_mut().for_each(|o| {
			let index_map = o.data.iter().enumerate().map(|(i, data)| {
				// What it would be referenced as in the octree
				let old_idx = i as u32 + 1;

				// What it should be referenced as now
				let new_idx = new_data.iter()
					.position(|x| x == data)
					.unwrap_or_else(|| {
						let idx = new_data.len();
						new_data.push(data.clone());
						idx
					}) as u32 + 1;
				
				// Reassure me that those point to the same data
				let old_t = if old_idx != 0 {
					Some(&o.data[old_idx as usize - 1])
				} else {
					None
				};
				let new_t = if new_idx != 0 {
					Some(&new_data[new_idx as usize - 1])
				} else {
					None
				};
				assert_eq!(old_t, new_t);

				(old_idx, new_idx)
			}).collect::<HashMap<_,_>>();

			for node in o.nodes.iter_mut() {
				// Adjust octant contents
				for i in 0..8 {
					if node.is_leaf(i) {
						if node.octants[i] != 0 {
							let g = index_map.get(&node.octants[i]).unwrap();
							node.octants[i] = *g;
						}
					}
				}
				// Adjust node content
				if node.content != 0 {
					let g = index_map.get(&node.content).unwrap();
					node.content = *g;
				}
			}
		});

		// Collect new nodes, adjusting nodes to point to it
		// Init with dummy root because it works better this way
		let mut new_nodes = vec![OctreeNode::new_leaf(None)];
		let mut octant_indices = Vec::new();
		octants.into_iter().for_each(|mut o| {
			// Adjust node indices
			let seg_st = new_nodes.len() as u32;
			// println!("Start is {seg_st}");
			o.nodes.iter_mut().for_each(|node| {
				for i in 0..8 {
					if !node.is_leaf(i) { // If points to new node
						if node.octants[i] != 0 { // If doesn't signify nothing
							node.octants[i] += seg_st;
						}
					}
				}
			});
			
			// Insert into tree
			octant_indices.push(seg_st);
			new_nodes.extend_from_slice(&o.nodes[..]);
		});

		// Get contents of octants from mixing function
		let top_contents = octant_indices.iter().map(|&i| {
			let o = &new_nodes[i as usize];
			o.get_content_index().and_then(|i| Some(&new_data[i as usize]))
		}).collect::<Vec<_>>();
		let new_content = mix_fn(&top_contents[..]);
		let new_content_index = new_content.and_then(|new_content| {
			// Find position or add new value
			Some(new_data.iter().position(|x| *x == new_content).unwrap_or_else(|| {
				let idx = new_data.len();
				new_data.push(new_content);
				idx
			}) as u32)
		});

		// Un-dummy root
		let root = &mut new_nodes[0];
		for (i, &v) in octant_indices.iter().enumerate() {
			root.set_octant(i, NodeContent::NodeIndex(v));
		}
		root.set_content_index(new_content_index);

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
				node.get_content_index().and_then(|ci| Some(&data[ci as usize])),
			);

			for i in 0..8 {
				let ns = match node.get_octant(i) {
					NodeContent::Empty => {
						"Empty".to_owned()
					},
					NodeContent::ContentIndex(ci) => {
						format!("Content({:?})", &data[ci as usize])
					},
					NodeContent::NodeIndex(ni) => {
						let next_node = &nodes[ni as usize];
						node_printer(data, nodes, next_node, indent + 1)
					},
				};

				s = format!(
					"{s}\n{:indent$}{i}: {ns}", "",
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
			match curr_node.get_octant(idx) {
				NodeContent::Empty => {
					// Could return parent content if wanted, but it makes everything lame
					// return curr_node.get_content_index().and_then(|ci| Some(&self.data[ci as usize]));
					return None;
				},
				NodeContent::ContentIndex(ci) => {
					return Some(&self.data[ci as usize]);
				},
				NodeContent::NodeIndex(ni) => {
					curr_node = &self.nodes[ni as usize];
					if xp { cx -= half_edge_len; }
					if yp { cy -= half_edge_len; }
					if zp { cz -= half_edge_len; }
					half_edge_len /= 2;
				},
			}
		}
	}
}


const MAX_CONTENTS_LEN: usize = (u32::MAX - 1) as usize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeContent {
	Empty,
	ContentIndex(u32),
	NodeIndex(u32),
}
#[derive(Clone, Copy, Debug)]
pub struct OctreeNode {
	// 0 - nnn
	// 1 - nnp
	// 2 - npn
	// 3 - npp
	// 4 - pnn
	// 5 - pnp
	// 6 - ppn
	// 7 - ppp
	pub octants: [u32; 8],
	pub leaf_mask: u8,
	pub content: u32,
}
impl OctreeNode {
	pub fn new_leaf(content_index: Option<u32>) -> Self {
		let content_index = content_index.and_then(|i| Some(i+1)).unwrap_or(0);
		Self {
			octants: [content_index; 8],
			leaf_mask: 0xFF,
			content: content_index,
		}
	}
	pub fn set_content_index(&mut self, content_index: Option<u32>) {
		self.content = content_index.and_then(|i| Some(i+1)).unwrap_or(0);
	}
	pub fn get_content_index(&self) -> Option<u32> {
		if self.content != 0 {
			Some(self.content - 1)
		} else {
			None
		}
	}
	pub fn set_octant(&mut self, i: usize, node_content: NodeContent) {
		let mask = 0b1 << i;
		match node_content {
			NodeContent::Empty => {
				self.leaf_mask = self.leaf_mask | mask;
				self.octants[i] = 0;
			}
			NodeContent::ContentIndex(ci) => {
				self.leaf_mask = self.leaf_mask | mask;
				self.octants[i] = ci + 1;
			}
			NodeContent::NodeIndex(ni) => {
				self.leaf_mask = self.leaf_mask & (mask ^ 0xFF);
				self.octants[i] = ni + 1;
			}
		}
	}
	pub fn get_octant(&self, i: usize) -> NodeContent {
		let o = self.octants[i];
		if o == 0 {
			NodeContent::Empty
		} else if self.is_leaf(i) {
			NodeContent::ContentIndex(o-1)
		} else {
			NodeContent::NodeIndex(o-1)
		}
	}
	pub fn is_leaf(&self, i: usize) -> bool {
		let mask = 0b1 << i;
		let leaf = self.leaf_mask & mask;
		let is_leaf = leaf > 0;
		is_leaf
	}
	/// Assuming is leaf, get either empty or index
	pub fn get_octant_leaf_idx(&self, i: usize) -> Option<u32> {
		if self.is_leaf(i) {
			let v = self.octants[i];
			if v == 0 {
				None
			} else {
				Some(v - 1)
			}
		} else {
			panic!()
		}
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
	fn test_sphere_size() {
		use crate::world::{Chunk, Voxel};

		// Old impl is 33220 bytes
		// New impl is 10984 bytes
		// 33% of the size!

		const CHUNK_SIZE: u32 = 32;
		const SPHERE_RADIUS: u32 = CHUNK_SIZE / 2;
		let mut sphere_chunk = Chunk::new([CHUNK_SIZE; 3]);
		for x in 0..CHUNK_SIZE as usize {
			for y in 0..CHUNK_SIZE as usize {
				for z in 0..CHUNK_SIZE as usize {
					if x.pow(2) + y.pow(2) + z.pow(2) <= (SPHERE_RADIUS as usize).pow(2) {
						sphere_chunk.set_voxel([x as i32, y as i32, z as i32], Voxel::Block(0));
					}
				}
			}
		}

		let sphere_octree = chunk_to_octree(&sphere_chunk).unwrap();

		println!("Octree takes {} bytes", sphere_octree.get_size());
	}

	#[test]
	fn test_octree_combine_get() {
		let octree1 = Octree::base(Some(4_i32), 0);
		assert!(octree1.get([0,0,0]) == Some(&4));
		println!("{}", octree1.print_test());

		let mix_fn = |_: &[Option<&i32>]| Some(8_i32);
		let octree2 = Octree::combine(
			Octree::base(None, 0), 
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
		println!("{}", octree2.print_test());
		let g = octree2.get([0,0,0]);
		assert!(g == None, "g is {g:?}");
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
			octree2.clone(),
			Octree::base(Some(43_i32), 1), 
			Octree::base(Some(44_i32), 1),
			Octree::base(Some(45_i32), 1), 
			Octree::base(Some(46_i32), 1),
			Octree::base(None, 1), 
			Octree::base(None, 1), 
			&mix_fn,
		);
		// println!("{:#?}", octree2.nodes);
		// println!("{:#?}", octree2.data);
		// println!("{:#?}", octree3.nodes);
		// println!("{:#?}", octree3.data);
		println!("{}", octree3.print_test());
		// nnn
		assert!(octree3.get([0,0,0]) == None);
		assert!(octree3.get([0,0,1]) == Some(&1));
		assert!(octree3.get([0,1,0]) == Some(&2));
		assert!(octree3.get([0,1,1]) == Some(&3));
		assert!(octree3.get([1,0,0]) == Some(&4));
		assert!(octree3.get([1,0,1]) == Some(&5));
		assert!(octree3.get([1,1,0]) == Some(&6));
		assert!(octree3.get([1,1,1]) == Some(&7));

		// nnp
		assert!(octree3.get([0,0,2]) == None);
		assert!(octree3.get([0,0,3]) == Some(&1));
		assert!(octree3.get([0,1,2]) == Some(&2));
		assert!(octree3.get([0,1,3]) == Some(&3));
		assert!(octree3.get([1,0,2]) == Some(&4));
		assert!(octree3.get([1,0,3]) == Some(&5));
		assert!(octree3.get([1,1,2]) == Some(&6));
		assert!(octree3.get([1,1,3]) == Some(&7));

		let assert_range = |mi: [i32; 3], ma: [i32; 3], v: Option<&i32>| {
			for x in mi[0]..=ma[0] {
				for y in mi[1]..=ma[1] {
					for z in mi[2]..=ma[2] {
						let p = [x,y,z];
						assert_eq!(octree3.get(p), v, "different at {p:?}");
					}
				}
			}
		};
		assert_range([0,2,0], [1,3,1], Some(&43)); // npn
		assert_range([0,2,2], [1,3,3], Some(&44)); // npp
		assert_range([2,0,0], [3,1,1], Some(&45)); // pnn
		assert_range([2,0,2], [3,1,3], Some(&46)); // pnp
		assert_range([2,2,0], [3,3,1], None); // ppn
		assert_range([2,2,2], [3,3,3], None); // ppp
	}

	#[test]
	fn test_octree_get() {
		const CHUNK_SIZE: u32 = 4;
		const N_BLOCKS: u32 = 4;

		let mut chunk = crate::world::Chunk::new([CHUNK_SIZE; 3]);
		chunk.contents = (0..(CHUNK_SIZE.pow(3))).map(|_| {
			let i = (rand::random::<f32>() * N_BLOCKS as f32).floor() as usize;
			if i > 0 {
				crate::world::Voxel::Block(i-1)
			} else {
				crate::world::Voxel::Empty
			}
		}).collect::<Vec<_>>();

		let octree = chunk_to_octree(&chunk).unwrap();

		println!("{}", octree.print_test());

		let mut g = vec![];
		for x in 0..(CHUNK_SIZE as i32) {
			for y in 0..(CHUNK_SIZE as i32) {
				for z in 0..(CHUNK_SIZE as i32) {
					let coords = [x,y,z];
					let cv = chunk.get_voxel(coords).id();
					let ov = octree.get(coords).and_then(|u| Some(*u));
					// assert_eq!(cv, ov, "different at {coords:?}");
					if cv != ov {
						g.push((coords, cv, ov));
					}
				}
			}
		}
		println!("{:?}", octree.data);
		for (p, cv, ov) in g.iter() {
			println!("{p:?}: {cv:?} != {ov:?}");
		}
		// assert!(g.len() == 0);
	}

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
		let directions = crate::rays::ray_spread(
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
		crate::rays::trace_octree(
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
			.map(|c| c.map(|c| (c * u8::MAX as f32) as u8))
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
