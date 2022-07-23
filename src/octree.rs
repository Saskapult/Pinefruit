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
	pub fn ray_intersect_positive(
		&self, 
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		position: Vector3<f32>, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<f32> {
		todo!()
	}
}


/// This octree implmentation is meant to be built upward.
/// 
/// The octree has a width dependent on its depth.
/// The side length is equal to 2^depth units.
#[derive(Debug, Clone)]
pub struct Octree<T: PartialEq + Clone + std::fmt::Debug> {
	data: Vec<T>,
	nodes: Vec<OctreeNode>,

	root: OctreeNode,
	depth: u16,
}
impl<T: PartialEq + Clone + std::fmt::Debug> Octree<T> {
	pub fn base(content: Option<T>, depth: u16) -> Self {
		let (data, content) = match content {
			Some(c) => (vec![c], 1),
			None => (vec![], 0),
		};
		Self {
			data,
			nodes: vec![],
			root: OctreeNode {
				octants: [0; 8],
				content,
			},
			depth,
		}
	}

	/// Combines a bunch of octrees into a bigger octree
	// This function uses too many clone()s, please make it better-er
	pub fn combine(
		nnn: Octree<T>, // 6
		nnp: Octree<T>, // 2
		npn: Octree<T>, // 5
		npp: Octree<T>, // 1
		pnn: Octree<T>, // 7
		pnp: Octree<T>, // 3
		ppn: Octree<T>, // 4
		ppp: Octree<T>, // 0
		mix_fn: &dyn Fn(&[&T]) -> T
	) -> Self {
		
		let octant_depth = ppp.depth;
		let mut octants = vec![nnn,nnp, npn, npp, pnn, pnp, ppn, ppp];

		// Test that same depth for all
		assert!(octants.iter().all(|g| g.depth == octant_depth), "Octants are of differing depth!");

		// Collect new data, adjusting octant graphs to point to it
		let mut new_data = Vec::new();
		octants.iter_mut().for_each(|o| {

			for (old_idx, data) in o.data.iter().enumerate() {
				// What it would be referenced as
				let old_idx = old_idx as u16 + 1;

				// What it should be referenced as now
				let new_idx = match new_data.iter().position(|x| x == data) {
					Some(idx) => idx,
					None => {
						let idx = new_data.len();
						new_data.push(data.clone());
						idx
					},
				} as u16 + 1;

				// Traverse tree if needs adjustment
				if old_idx != new_idx {
					// println!("Data index reference {} -> {}", old_idx, new_idx);
					// Adjust index in root
					if o.root.content == old_idx {
						o.root.content = new_idx;
					}
					// Adjust index in children
					for node in o.nodes.iter_mut() {
						if node.content == old_idx {
							node.content = new_idx;
						}
					}
				}
			}
		});

		// If all are of same content and have no children then combine
		let same_content = octants.iter().all(|o| o.root.content == octants[0].root.content);
		let all_leaves = octants.iter().all(|o| {
			o.root.octants.iter().all(|&v| v == 0)
		});
		if same_content && all_leaves {
			// println!("WE CAN COMBINE THEM!");
			return Self {
				data: new_data,
				nodes: vec![],

				root: OctreeNode {
					octants: [0; 8],
					content: octants[0].root.content,
				},
				depth: octant_depth + 1,
			}
		}

		// For each octant, merge its data with the existing data and adjust its indices
		// println!("Doing node stuff");
		let mut new_nodes = Vec::new();
		let mut octant_indices = Vec::new();
		octants.into_iter().for_each(|mut o| {

			// Adjust node indices
			let offset = new_nodes.len() as u32 + 1; // Offet 1 away from its root, which also happens to be the root index ref
			// println!("Offset is {offset}");
			o.nodes.iter_mut().for_each(|node| {
				node.octants = node.octants.map(|v| {
					if v != 0 {
						v + offset
					} else {
						v
					}
				});
			});
			o.root.octants = o.root.octants.map(|v| {
				if v != 0 {
					v + offset
				} else {
					v
				}
			});
			
			// Insert into tree
			new_nodes.push(o.root);
			octant_indices.push(offset);
			new_nodes.extend(o.nodes.into_iter())
		});

		// Todo: optionally search for existing value
		let contents = octant_indices.iter().map(|&i| &new_data[i as usize]).collect::<Vec<_>>();
		let new_content = mix_fn(&contents[..]);
		let content = new_data.len() as u16;
		new_data.push(new_content);

		Self {
			data: new_data,
			nodes: new_nodes,

			root: OctreeNode {
				octants: octant_indices.try_into().unwrap(),
				content,
			},
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
			let mut s = format!("{:?}", if node.content > 0 { Some(&data[node.content as usize - 1]) } else { None });

			for (idx, octant) in node.octants.iter().enumerate() {
				let octant = *octant;
				if octant == 0 {
					continue
				}

				let new_bit = node_printer(data, nodes, &nodes[octant as usize - 1], indent+1);

				s = format!("{s}\n{:indent$}{}: {}", "", idx+1, new_bit, indent=(indent as usize + 1)*2);
			}

			s
		}

		format!(
			"Octree of depth {} with content {}", 
			self.depth, 
			node_printer(&self.data, &self.nodes, &self.root, 0)
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

	// // https://daeken.svbtle.com/a-stupidly-simple-fast-octree-traversal-for-ray-intersection
	// // Not limited to standard octrees, maybe not the best here
	// #[inline]
	// pub fn stupid_ray(
	// 	&self, 
	// 	origin: Vector3<f32>,
	// 	direction: Vector3<f32>,
	// 	position: Vector3<f32>, 
	// ) -> Option<()> {
	// 	// if empty return null
	// 	// if have content return content

	// 	let mut octant = self.root;


	// 	let side_length = 2_u32.pow(self.depth as u32);
	// 	let oct_max = side_length as f32;
	// 	let oct_min = 0.0;
	// 	let aabb = AABB::new(
	// 		Vector3::new(oct_min, oct_min, oct_min),
	// 		Vector3::new(oct_max, oct_max, oct_max),
	// 	);
	// 	let [pz, py, px] = aabb.mid_planes();
	// 	let mut side = [
	// 		origin.dot(&px.normal) - px.distance >= 0.0,
	// 		origin.dot(&py.normal) - py.distance >= 0.0,
	// 		origin.dot(&pz.normal) - pz.distance >= 0.0,
	// 	];

	// 	let mut xdist = if side[0] == (direction[0] < 0.0) {
	// 		px.ray_intersect_positive(origin, direction, position, 0.0, 100.0).unwrap()
	// 	} else {
	// 		f32::INFINITY
	// 	};
	// 	let mut ydist = if side[1] == (direction[1] < 0.0) {
	// 		py.ray_intersect_positive(origin, direction, position, 0.0, 100.0).unwrap()
	// 	} else {
	// 		f32::INFINITY
	// 	};
	// 	let mut zdist = if side[2] == (direction[2] < 0.0) {
	// 		pz.ray_intersect_positive(origin, direction, position, 0.0, 100.0).unwrap()
	// 	} else {
	// 		f32::INFINITY
	// 	};

	// 	for _ in 0..3 {
	// 		let idx = if side[2] { 1 } else { 0 } |
	// 			if side[1] { 2 } else { 0 } |
	// 			if side[0] { 4 } else { 0 };
			
	// 		// let ret = recurse on indexed octant
	// 		// if not none return ret

	// 		let min_dist = f32::min(f32::min(xdist, ydist), zdist);

	// 		let hitpos = origin + direction * min_dist;
	// 		if !aabb.contains(hitpos) {
	// 			return None;
	// 		}
	// 		if min_dist == xdist {
	// 			side[0] = !side[0];
	// 			xdist = f32::INFINITY;
	// 		} else if min_dist == ydist {
	// 			side[1] = !side[1];
	// 			ydist = f32::INFINITY;
	// 		} else if min_dist == zdist {
	// 			side[2] = !side[2];
	// 			zdist = f32::INFINITY;
	// 		}
	// 	}

	// 	None
	// }

	// // https://lsi2.ugr.es/curena/inves/wscg00/revelles-wscg00.pdf
	// pub fn jr_ray_thing(
	// 	&self, 
	// 	mut origin: Vector3<f32>, 
	// 	mut direction: Vector3<f32>,
	// ) {

	// 	direction = direction.normalize();
		
	// 	let octree_size = 2_u32.pow(self.depth as u32);
	// 	let oct_max = (octree_size / 2) as i32;
	// 	let oct_min = -oct_max;

	// 	// What is this?
	// 	let mut a = 0_u32;

	// 	// Fix negative direction
	// 	if direction[0] < 0.0 {
	// 		origin[0] = octree_size as f32 - origin[0];
	// 		direction[0] = -direction[0];
	// 		a |= 4;
	// 	}
	// 	if direction[1] < 0.0 {
	// 		origin[1] = octree_size as f32 - origin[1];
	// 		direction[1] = -direction[1];
	// 		a |= 2;
	// 	}
	// 	if direction[2] < 0.0 {
	// 		origin[2] = octree_size as f32 - origin[2];
	// 		direction[2] = -direction[2];
	// 		a |= 1;
	// 	}

	// 	let tx0 = (oct_min as f32 - origin[0]) / direction[0];
	// 	let tx1 = (oct_max as f32 - origin[0]) / direction[0];
	// 	let ty0 = (oct_min as f32 - origin[1]) / direction[1];
	// 	let ty1 = (oct_max as f32 - origin[1]) / direction[1];
	// 	let tz0 = (oct_min as f32 - origin[2]) / direction[2];
	// 	let tz1 = (oct_max as f32 - origin[2]) / direction[2];

	// 	if f32::max(f32::max(tx0, ty0), tz0) < f32::min(f32::min(tx1, ty1), tz1) {
	// 		self.proc_subtree(tx0, ty0, tz0, tx1, ty1, tz1, &self.root);
	// 	}

	// }
	// fn first_node(&self, tx0: f32, ty0: f32, tz0: f32, txm: f32, tym: f32, tzm: f32) -> u32 {
	// 	let mut answer = 0_u32;
	// 	if tx0 > ty0 {
	// 		if tx0 > tz0 {
	// 			// YZ
	// 			if tym < tx0 { answer |= 2; }
	// 			if tzm < tx0 { answer |= 1; }
	// 			return answer
	// 		}
	// 	} else {
	// 		if ty0 > tz0 {
	// 			// XZ
	// 			if txm < ty0 { answer |= 4; }
	// 			if tzm < ty0 { answer |= 1; }
	// 			return answer
	// 		}
	// 	}
	// 	// XY
	// 	if txm < tz0 { answer |= 4; }
	// 	if tym < tz0 { answer |= 2; }
	// 	return answer
	// }
	// fn new_node(&self, txm: f32, x: u32, tym: f32, y: u32, tzm: f32, z: u32) -> u32 {
	// 	if txm < tym {
	// 		if txm < tzm {
	// 			return x // YZ
	// 		}
	// 	} else {
	// 		if tym < tzm {
	// 			return y // XZ
	// 		}
	// 	}
	// 	z // XY
	// }
	// fn proc_subtree(
	// 	&self,
	// 	tx0: f32, 
	// 	ty0: f32, 
	// 	tz0: f32, 
	// 	tx1: f32, 
	// 	ty1: f32, 
	// 	tz1: f32, 
	// 	node: &OctreeNode,
	// ) -> u16 {
	// 	if tx1 < 0.0 || ty1 < 0.0 || tz1 < 0.0 {
	// 		return 0;
	// 	}

	// 	if node.octants.iter().all(|&o| o == 0) {
	// 		return node.content;
	// 	}

	// 	let txm = 0.5 * (tx0 + tx1);
	// 	let tym = 0.5 * (ty0 + ty1);
	// 	let tzm = 0.5 * (tz0 + tz1);

	// 	let mut curr_node = self.first_node(tx0, ty0, tz0, txm, tym, tzm);
	// 	loop {
	// 		match curr_node {
	// 			0 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			1 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			2 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			3 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			4 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			5 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			6 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			7 => {
	// 				let node_content = node.octants[0] as usize;
	// 				if node_content == 0 {
	// 					return 0
	// 				}
	// 				let node = &self.nodes[node_content-1];
	// 				self.proc_subtree(tx0, ty0, tz0, txm, tym, tzm, node);
	// 				curr_node = self.new_node(txm, 4, tym, 2, txm, 1);
	// 				break
	// 			},
	// 			_ => panic!(),
	// 		}
	// 		if !(curr_node < 8) {
	// 			break
	// 		} 
	// 	}

	// }

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

		let mut curr_node = &self.root;
		let [mut cx, mut cy, mut cz] = coords;
		let mut half_edge_len = 2_i32.pow(self.depth as u32 - 1);
		while !curr_node.octants.iter().all(|&o| o == 0) {
			// println!("Iter with half edge len {half_edge_len} and coords [{cx}, {cy}, {cz}]");

			let xp = cx >= half_edge_len;
			let yp = cy >= half_edge_len;
			let zp = cz >= half_edge_len;
			if xp {
				cx -= half_edge_len;
				if yp {
					cy -= half_edge_len;
					if zp {
						cz -= half_edge_len;
						// xp, yp, zp
						// println!("ppp");
						curr_node = &self.nodes[curr_node.octants[7] as usize - 1];
					} else {
						// xp, yp, zn
						// println!("ppn");
						curr_node = &self.nodes[curr_node.octants[6] as usize - 1];
					}
				} else {
					if zp {
						cz -= half_edge_len;
						// xp, yn, zp
						// println!("pnp");
						curr_node = &self.nodes[curr_node.octants[5] as usize - 1];
					} else {
						// xp, yn, zn
						// println!("pnn");
						curr_node = &self.nodes[curr_node.octants[4] as usize - 1];
					}
				}
			} else {
				if yp {
					cy -= half_edge_len;
					if zp {
						cz -= half_edge_len;
						// xn, yp, zp
						// println!("npp");
						curr_node = &self.nodes[curr_node.octants[3] as usize - 1];
					} else {
						// xn, yp, zn
						// println!("npn");
						curr_node = &self.nodes[curr_node.octants[2] as usize - 1];
					}
				} else {
					if zp {
						cz -= half_edge_len;
						// xn, yn, zp
						// println!("nnp");
						curr_node = &self.nodes[curr_node.octants[1] as usize - 1];
					} else {
						// xn, yn, zn
						// println!("nnn");
						curr_node = &self.nodes[curr_node.octants[0] as usize - 1];
					}
				}
			}

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
struct OctreeNode {
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
		let x = x * dim * dim;
		(0..dim).flat_map(move |y| {
			let y = y * dim;
			(0..dim).map(move |z| {
				Octree::base(chunk.contents[(x + y + z) as usize].id(), 0)
			})
		})
	}).collect::<Vec<_>>();

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
					let mixfn = |_: &[&usize]| 1_usize;
					
					reduced_trees.push(Octree::combine(nnn, nnp, npn, npp, pnn, pnp, ppn, ppp, &mixfn));
				}
			}
		}

		trees = reduced_trees;
		dim /= 2;
	}

	Some(trees[0].clone())
}
// pub fn octree_to_chunk(octree: &Octree<usize>) -> crate::world::Chunk {
// 	let chunk_size = 2_u32.pow(octree.depth as u32);

// 	todo!()
// }





#[cfg(test)]
mod tests {
	// use std::mem::size_of;
	use super::*;

	// #[test]
	// fn test_octree_print() {
	// 	let octree = Octree::base(Some(4_i32), 0);
	// 	println!("{}", octree.print_test());

	// 	let octree2 = Octree {
	// 		data: vec![4_i32, 5_i32],
	// 		nodes: vec![
	// 			OctreeNode {
	// 				octants: [0, 0, 0, 0, 0, 0, 0, 0], 
	// 				content: 2, 
	// 			},
	// 			OctreeNode {
	// 				octants: [0, 0, 3, 0, 0, 0, 0, 0], 
	// 				content: 1, 
	// 			},
	// 			OctreeNode {
	// 				octants: [0, 0, 0, 0, 0, 0, 0, 0], 
	// 				content: 0, 
	// 			},
	// 		],
	// 		root: OctreeNode { 
	// 			octants: [0, 1, 0, 0, 0, 2, 0, 0], 
	// 			content: 1, 
	// 		},
	// 		depth: 2,
	// 	};
	// 	println!("{}", octree2.print_test());

	// 	let octree3 = Octree::combine(
	// 		Octree::base(Some(1_i32), 0), 
	// 		Octree::base(Some(2_i32), 0),
	// 		Octree::base(Some(3_i32), 0), 
	// 		Octree::base(Some(4_i32), 0),
	// 		Octree::base(Some(5_i32), 0), 
	// 		Octree::base(Some(6_i32), 0),
	// 		Octree::base(Some(7_i32), 0), 
	// 		Octree::base(Some(8_i32), 0),
	// 	);
	// 	println!("{}", octree3.print_test());

	// 	let coords = [0,0,0];
	// 	println!("Octree 3 {coords:?} is {:?}", octree3.get(coords));
	// 	// should be 7

	// 	let octree5 = Octree::combine(
	// 		Octree::base(Some(2_i32), 0),
	// 		Octree::base(Some(2_i32), 0),
	// 		Octree::base(Some(2_i32), 0), 
	// 		Octree::base(Some(2_i32), 0),
	// 		Octree::base(Some(2_i32), 0), 
	// 		Octree::base(Some(2_i32), 0),
	// 		Octree::base(Some(2_i32), 0), 
	// 		Octree::base(Some(2_i32), 0),
	// 	);
	// 	println!("{}", octree5.print_test());

	// 	let octree4 = Octree::combine(
	// 		octree3.clone(),
	// 		Octree::base(Some(2_i32), 1),
	// 		Octree::base(Some(3_i32), 1), 
	// 		Octree::base(Some(4_i32), 1),
	// 		Octree::base(Some(5_i32), 1), 
	// 		Octree::base(Some(6_i32), 1),
	// 		Octree::base(None, 1), 
	// 		octree5,
	// 	);
	// 	// println!("Constructed o4");
	// 	// println!("o4 :{:#?}\n", octree4);
	// 	println!("{}", octree4.print_test());

	// 	let octree6 = Octree::combine(
	// 		octree3.clone(),
	// 		octree3.clone(),
	// 		octree3.clone(),
	// 		octree3.clone(),
	// 		octree3.clone(),
	// 		octree3.clone(),
	// 		Octree::base(None, 1),
	// 		octree3.clone(),
	// 	);
	// 	// println!("Constructed o4");
	// 	// println!("o4 :{:#?}\n", octree4);
	// 	println!("{}", octree6.print_test());

	// 	let coords = [0,0,0];
	// 	println!("Octree 6 {coords:?} is {:?}", octree6.get(coords));
	// 	let coords = [1,1,0];
	// 	println!("Octree 6 {coords:?} is {:?}", octree6.get(coords));
	// 	let coords = [2,1,0];
	// 	println!("Octree 6 {coords:?} is {:?}", octree6.get(coords));


	// 	assert!(true);
	// }

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

		// println!("{}", octree.print_test());
		// panic!();

		let colours = vec![
			[0.0, 0.0, 0.0],
			[1.0, 0.0, 0.0],
			[1.0, 1.0, 0.0],
			[1.0, 0.0, 1.0],
			[0.0, 1.0, 0.0],
			[0.0, 0.0, 1.0],
			[1.0, 1.0, 1.0],

			[0.5, 0.0, 0.0],
			[0.5, 0.5, 0.0],
			[0.5, 0.0, 0.5],
			[0.0, 0.5, 0.0],
			[0.0, 0.0, 0.5],
			[0.5, 0.5, 0.5],
		];

		let xz_colours = (0..CHUNK_SIZE).flat_map(|x| {
			(0..CHUNK_SIZE).map(move |z| {
				[x as i32, z as i32]
			})
		}).map(|[x, z]| {
			let idx = match octree.get([x, (CHUNK_SIZE-1) as i32, z]) {
				Some(idx) => idx+1,
				None => 0,
			};
			colours[idx].map(|f| {
				(f * u8::MAX as f32).floor() as u8
			})
		})
		.flatten()
		.collect::<Vec<_>>();
		let imb = image::ImageBuffer::from_vec(CHUNK_SIZE, CHUNK_SIZE, xz_colours.clone()).unwrap();
		let img = image::DynamicImage::ImageRgb8(imb);
		std::thread::spawn(move || {
			crate::util::show_image(img).unwrap();
		});
		std::thread::sleep(std::time::Duration::from_secs(1));


		let xz_colours_2 = (0..CHUNK_SIZE).flat_map(|x| {
			(0..CHUNK_SIZE).map(move |z| {
				[x as i32, z as i32]
			})
		}).map(|[x, z]| {
			let idx = match chunk.get_voxel([x, 0, z]).id() {
				Some(idx) => idx+1,
				None => 0,
			};
			colours[idx].map(|f| {
				(f * u8::MAX as f32).floor() as u8
			})
		})
		.flatten()
		.collect::<Vec<_>>();
		let imb = image::ImageBuffer::from_vec(CHUNK_SIZE, CHUNK_SIZE, xz_colours_2.clone()).unwrap();
		let img = image::DynamicImage::ImageRgb8(imb);
		// std::thread::spawn(move || {
		crate::util::show_image(img).unwrap();
		// });
		
		// std::thread::sleep(std::time::Duration::from_secs(30));
		assert!(xz_colours == xz_colours_2, "Colour maps not equal!");
		// assert_eq!(xz_colours, xz_colours_2);
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
	fn test_di_trace() {

		let width = 400;
		let height = 300;
		let fovy = 90.0;
		let n_blocks = 8;
		let chunk_size = 8;


		// Map block colours
		let colours = (0..n_blocks).map(|_| [rand::random::<f32>(), rand::random::<f32>(), rand::random::<f32>()]).collect::<Vec<_>>();


		// Make contents
		let mut chunk = crate::world::Chunk::new([chunk_size; 3]);
		for x in 0..chunk_size as i32 {
			for y in 0..chunk_size as i32 {
				for z in 0..chunk_size as i32 {
					if (x-4).pow(2) + (y-4).pow(2) + (z-4).pow(2) <= 3_i32.pow(2) {
						chunk.set_voxel([x, y, z], crate::world::Voxel::Block(0));
					}
					// if x % 2 == 0 {
					// 	chunk.set_voxel([x, y, z], crate::world::Voxel::Block(0));
					// }
				}
			}
		}
		// chunk.contents = (0..(chunk_size*chunk_size*chunk_size)).map(|_| {
		// 	let i = (rand::random::<f32>() * n_blocks as f32).floor() as usize;
		// 	if i > 0 {
		// 		crate::world::Voxel::Block(i-1)
		// 	} else {
		// 		crate::world::Voxel::Empty
		// 	}
		// }).collect::<Vec<_>>();
		let octree = chunk_to_octree(&chunk).unwrap();


		// Make hits
		let origin = Vector3::new(0.0, 0.0, 0.0);
		let directions = crate::render::rays::ray_spread(
			UnitQuaternion::identity(), 
			width, 
			height, 
			fovy,
		);
		let octree_position = Vector3::new(-4.0, -4.0, 8.0);

		let render = |octree: &Octree<usize>, octree_position| {
			let st = std::time::Instant::now();
			let img_data = directions.iter().flat_map(|&d| {
				match octree.aa_intersect(origin, d, octree_position, 0.0, 100.0) {
					Some((st, _en)) => {

						let hit_pos = origin + d * (st + 0.05);
						
						let octree_rel_hit = hit_pos - octree_position;

						let mut iiter = crate::render::rays::AWIter::new(
							octree_rel_hit, 
							d, 
							0.0, 
							25.0, 
							1.0,
						);

						// Mark initial miss as red
						if !octree.in_bounds([iiter.vx, iiter.vy, iiter.vz]) {
							return [u8::MAX, 0, 0]
						}

						loop {
							// if let Some(&g) = octree.get([iiter.vx, iiter.vy, iiter.vz]) {
							// 	return colours[g].map(|f| (f * u8::MAX as f32) as u8)
							// }

							if let Some(g) = chunk.get_voxel([iiter.vx, iiter.vy, iiter.vz]).id() {
								return colours[g].map(|f| (f * u8::MAX as f32) as u8)
							}

							// Mark out of cast length as green
							if !iiter.next().is_some() {
								return [0, u8::MAX, 0]
							}

							// Mark out of bounds as white
							if !octree.in_bounds([iiter.vx, iiter.vy, iiter.vz]) {
								// println!("OOB in iteration {c} ({:?})", [iiter.vx, iiter.vy, iiter.vz]);
								return [u8::MAX; 3]
							}
						}
					},
					None => [u8::MIN; 3],
				}
			}).collect::<Vec<_>>();	
			let en = std::time::Instant::now();

			let imb = image::ImageBuffer::from_vec(width, height, img_data).unwrap();
			let img = image::DynamicImage::ImageRgb8(imb);

			(img, en-st)
		};


		// // Image time
		// let conf = viuer::Config {
		// 	..Default::default()
		// };

		// let o1 = Octree::base(Some(1_usize), 1);
		// let o2 = Octree::base(Some(2_usize), 1);
		// let o12 = Octree::combine(
		// 	o1.clone(), 
		// 	o2.clone(), 
		// 	o2.clone(), 
		// 	o1.clone(), 
		// 	o1.clone(), 
		// 	o2.clone(), 
		// 	o2.clone(), 
		// 	o1.clone(),
		// );
		// let o122 = Octree::combine(
		// 	o12.clone(), 
		// 	o12.clone(), 
		// 	o12.clone(), 
		// 	o12.clone(), 
		// 	o12.clone(), 
		// 	o12.clone(), 
		// 	o12.clone(), 
		// 	o12.clone(),
		// );

		// let (img, dur) = render(&o1, octree_position, octree_rotation);
		// // viuer::print(&img, &conf).expect("Image printing failed.");
		// println!("Done in {}ms", dur.as_millis());
		// crate::util::save_image(img, &"/tmp/_o1.png").unwrap();

		// let (img, dur) = render(&o2, octree_position, octree_rotation);
		// // viuer::print(&img, &conf).expect("Image printing failed.");
		// println!("Done in {}ms", dur.as_millis());
		// crate::util::save_image(img, &"/tmp/_o2.png").unwrap();

		// let (img, dur) = render(&o12, octree_position, octree_rotation);
		// // viuer::print(&img, &conf).expect("Image printing failed.");
		// println!("Done in {}ms", dur.as_millis());
		// crate::util::save_image(img, &"/tmp/_o12.png").unwrap();

		// let (img, dur) = render(&o122, octree_position, octree_rotation);
		// // viuer::print(&img, &conf).expect("Image printing failed.");
		// println!("Done in {}ms", dur.as_millis());
		// crate::util::save_image(img, &"/tmp/_o122.png").unwrap();

		let (img, dur) = render(&octree, octree_position);
		// viuer::print(&img, &conf).expect("Image printing failed.");
		println!("Done in {}ms", dur.as_millis());
		crate::util::save_image(img, &"/tmp/_before.png").unwrap();

		// println!("Setting corners (to Some(4))");
		// for x in [0, chunk_size as i32 - 1] {
		// 	for y in [0, chunk_size as i32 - 1] {
		// 		for z in [0, chunk_size as i32 - 1] {
		// 			println!("Setting {:?}", [x, y, z]);
		// 			chunk.set_voxel([x, y, z], crate::world::Voxel::Block(4));
		// 		}
		// 	}
		// }
		// let octree = chunk_to_octree(&chunk).unwrap();

		// println!("Getting corners (result should be Some(4))");
		// for x in [0, chunk_size as i32 - 1] {
		// 	for y in [0, chunk_size as i32 - 1] {
		// 		for z in [0, chunk_size as i32 - 1] {
		// 			println!("Getting {:?} -> {:?}", [x, y, z], octree.get([x, y, z]));
		// 		}
		// 	}
		// }

		// let (img, dur) = render(&octree, octree_position);
		// // viuer::print(&img, &conf).expect("Image printing failed.");
		// println!("Done in {}ms", dur.as_millis());
		// crate::util::save_image(img, &"/tmp/_after.png").unwrap();
	}

	#[test]
	fn test_di_trace_2() {

		let width = 400;
		let height = 300;
		let fovy = 90.0;
		let n_blocks = 8;
		let chunk_size = 8;

		// Map block colours
		let palette = (0..n_blocks).map(|_| [rand::random::<f32>(), rand::random::<f32>(), rand::random::<f32>(), 0.0]).collect::<Vec<_>>();

		// Make contents
		let mut chunk = crate::world::Chunk::new([chunk_size; 3]);
		for x in 0..chunk_size as i32 {
			for y in 0..chunk_size as i32 {
				for z in 0..chunk_size as i32 {
					if (x-4).pow(2) + (y-4).pow(2) + (z-4).pow(2) <= 3_i32.pow(2) {
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
		let octree_position = Vector3::new(-4.0, -4.0, 8.0);

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
