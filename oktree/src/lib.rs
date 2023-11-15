use std::fmt::Debug;
use bytemuck::{Pod, Zeroable};



// offset: 0x0000FFFF
// node:   0x00FF0000
// leaf:   0xFF000000
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod, PartialEq, Eq)]
pub struct OctreeNode {
	pub child_offset: u16, // The first child is at parent_index + 1 + child_offset
	pub node: u8,
	pub leaf: u8,
}
impl OctreeNode {
	// Size of preceding child octants
	pub fn octant_offset(&self, octant: u8, leaf_size: usize) -> usize {
		// Special (lazy) case where I want to know the toatl offset
		let preced_mask = if octant == 8 {
			0b11111111_u8
		} else {
			!(0b11111111_u8.wrapping_shr(octant as u32))
		};

		let preceding_leaves = u8::count_ones(self.leaf & preced_mask) as usize;
		let preceding_nodes = u8::count_ones(self.node & preced_mask) as usize;
		let offset = preceding_leaves * leaf_size + preceding_nodes;

		// // println!("octant {octant}, {preced_mask:#010b} of {:#010b} leaves and {:#010b} nodes gives offset of {} ({} nodes, {} leaves)", self.leaf, self.node, offset, preceding_nodes, preceding_leaves);

		offset
	}

	// Used to get a data offset, data should be at index (parent_index + 1 + this)
	pub fn to_end_from(&self, octant: u8, leaf_size: usize) -> usize {
		let total = self.octant_offset(8, leaf_size);
		let preceding = self.octant_offset(octant + 1, leaf_size);
		total - preceding
	}

	pub fn has_children(&self) -> bool {
		self.node != 0 || self.leaf != 0
	}
}
impl Into<u32> for OctreeNode {
	fn into(self) -> u32 {
		bytemuck::cast(self)
	}
}
impl From<u32> for OctreeNode {
	fn from(value: u32) -> Self {
		bytemuck::cast(value)
	}
}
impl<'a> From<&'a mut u32> for &'a mut OctreeNode {
	fn from(value: &'a mut u32) -> Self {
		bytemuck::cast_mut(value)
	}
}
impl std::fmt::Debug for OctreeNode {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "OctreeNode {{ child_index: {}, node: {:#010b}, leaf: {:#010b} }}", self.child_offset, self.node, self.leaf)
	}
}


#[derive(Debug, Clone)]
pub struct Octree {
	data: Vec<u32>,
	node: u8,
	leaf: u8,
	depth: u32,
	leaf_size: usize,
}
impl Octree {
	pub fn new(depth: u32, leaf_size: usize) -> Self {
		Self {
			data: Vec::new(),
			leaf: 0b00000000_u8,
			node: 0b00000000_u8,
			depth,
			leaf_size,
		}
	}

	/// Encodes the octree as a sequence of tetrabytes. 
	/// The first element will always be an [OctreeNode] with child_offset = 0.
	/// This is the root node which is left partially implicit in this implementation. 
	/// 
	/// Note that this does not store the octree's depth or leaf size. 
	pub fn data(&self) -> Vec<u32> {
		let mut data = Vec::with_capacity(self.data.len() + 1);
		data.push(OctreeNode {
			child_offset: 0, 
			node: self.node,
			leaf: self.leaf,
		}.into());
		data.extend_from_slice(self.data.as_slice());
		data
	}

	// Panics if can't be converted or stored
	fn extract_data<'a>(content: &'a (impl Pod + Zeroable), leaf_size: usize) -> &'a [u32] {
		let data = bytemuck::bytes_of(content);
		let data = bytemuck::try_cast_slice::<_, u32>(data)
			.expect("data content is not tetrabyte aligned!");
		assert_eq!(leaf_size, data.len(), "data content does not fit this octree leaf size!");
		data
	}

	// fn check_bounds(&self, x: u32, y: u32, z: u32) {
	// 	let extent = 2_u32.pow(self.depth as u32);
	// 	assert!(x < extent, "x coordiante exceeds extent!");
	// 	assert!(y < extent, "y coordiante exceeds extent!");
	// 	assert!(z < extent, "z coordiante exceeds extent!");
	// }

	pub fn print_guess(&self) {
		println!("leaf {:#010b} ({})", self.leaf, self.leaf);
		println!("node {:#010b} ({})", self.node, self.node);
		for (i, value) in self.data.iter().cloned().enumerate() {
			if value > 100 {
				let node: OctreeNode = value.into();
				println!("{i}: {node:?}")
			} else {
				println!("{i}: {value}")	
			}
		}
	}

	pub fn print_tree(&self) {
		fn print_node(
			segment_start: usize, 
			leaf: u8, 
			node: u8, 
			depth: usize, 
			leaf_size: usize, 
			data: &Vec<u32>,
		) {
			for octant in 0..8 {
				let octant_mask = 1 << (7 - octant);
				let mut octant_index = segment_start + octant_offset(octant, leaf, node, leaf_size);
				
				let has_leaf = leaf & octant_mask != 0;
				let has_node = node & octant_mask != 0;
				if has_leaf || has_node {
					print!("\n");
					print!("{:indent$}", "", indent=depth);
					print!("{octant}: ");
				}

				match (has_leaf, has_node) {
					(false, false) => {}, //print!("empty"),
					(true, false) => print!("leaf"),
					(false, true) => print!("node"),
					(true, true) => print!("leafnode"),
				}

				if has_leaf {
					let data = &data[octant_index..octant_index+leaf_size];
					print!(" ({:?})", data);
					octant_index += leaf_size;
				}
				if has_node {
					let node: OctreeNode = data.get(octant_index).unwrap().clone().into();
					let next_index = octant_index + node.child_offset as usize + 1;
					print_node(
						next_index, 
						node.leaf, 
						node.node, 
						depth+1, 
						leaf_size, 
						data,
					)
				}
			}
		}

		print_node(
			0, 
			self.leaf, 
			self.node, 
			0, 
			self.leaf_size, 
			&self.data,
		);

		print!("\n");
	}

	pub fn insert(&mut self, x: u32, y: u32, z: u32, content: &(impl Pod + Zeroable)) {
		fn insert_inner(
			segment_start: usize, // The start of the child octants
			octant: u8, // Which octant should we look at?
			mut leaf_mask: u8, // The parent's leaf mask
			mut node_mask: u8, // The parent's node mask
			mut octant_iter: OctantCodeIterator,
			leaf_size: usize,
			data: &mut Vec<u32>,
			item: &[u32], 
		 ) -> (i32, u8, u8) {
			let octant_mask = 1 << (7 - octant);
			let mut octant_index = segment_start + octant_offset(octant, leaf_mask, node_mask, leaf_size); // The index of the octant we're looking at
			// println!("\tOctant {octant}");

			if let Some(next_octant) = octant_iter.next() {
				let mut self_size_delta = 0;

				if leaf_mask & octant_mask != 0 { octant_index += leaf_size; } // Skip leaf
				
				// Create node 
				if node_mask & octant_mask == 0 {
					// println!("Create node for octant {octant} ({octant_mask:#010b}) at {octant_index}");

					// Yo this might be wrong
					let child_offset = segment_start + octant_offset(8, leaf_mask, node_mask, leaf_size) - octant_index; // Data starts directly after the end of this data
					data.insert(octant_index, OctreeNode {
						child_offset: child_offset as u16, 
						node: 0b00000000,
						leaf: 0b00000000,
					}.into());

					// Every octant before this must have its pointer adjusted to compensate
					for octant in 0..octant {
						let octant_mask = 1_u8 << (7 - octant);
						if node_mask & octant_mask != 0 {
							let mut octant_index = segment_start + octant_offset(octant, leaf_mask, node_mask, leaf_size);
							if leaf_mask & octant_mask != 0 { octant_index += leaf_size; } // Skip leaf

							// println!("Adjust octant {octant} (at index {octant_index}) with +1");

							let octant_node: &mut OctreeNode = data.get_mut(octant_index).unwrap().into();
							octant_node.child_offset += 1;
						}
					}

					node_mask |= octant_mask;
					self_size_delta += 1;
				}

				let octant_node: OctreeNode = data.get(octant_index).unwrap().clone().into();
				let next_leaf_mask = octant_node.leaf;
				let next_node_mask = octant_node.node;
				let next_segment_start = octant_index + octant_node.child_offset as usize + 1;

				let (mut child_size_delta, new_leaf_mask, new_node_mask) = insert_inner(
					next_segment_start, 
					next_octant, 
					next_leaf_mask, 
					next_node_mask, 
					octant_iter, 
					leaf_size, 
					data,
					item,
				);

				// println!("\tBack to octant {octant}");

				// Write changes back to that child
				let octant_node: &mut OctreeNode = data.get_mut(octant_index).unwrap().into();
				octant_node.leaf = new_leaf_mask;
				octant_node.node = new_node_mask;

				// Reduce if possible, please remember to adjust leaf_mask and node_mask
				if new_node_mask == 0b00000000_u8 {
					// None have nodes, can combine iff
					// - All have leaf with same content
					// - All have no content
					if new_leaf_mask == 0b11111111_u8 {
						// Read all things and decide
						assert_eq!(1, leaf_size, "Combining octants with content only works if leaf_size=1, make a better comparison function if you can");
						let contents = &data[next_segment_start..next_segment_start+8*leaf_size];
						if let Some(&e) = are_elements_equal(contents) {
							// println!("WE CAN COMBINE THEM (all are {e})");
							child_size_delta -= 8 * leaf_size as i32;
							data.drain(next_segment_start..next_segment_start+8*leaf_size);
							// This was a node and now it should be a leaf
							data[octant_index] = e;
							node_mask ^= octant_mask;
							leaf_mask ^= octant_mask;
						}
					} else if new_leaf_mask == 0b00000000_u8 {
						// hwy wait this only happens if we're removing
						// println!("WE CAN COMBINE THEM (all are None)");
						// Doesn't point to anything, so we can just remove the node component
						data.remove(octant_index);
						node_mask ^= octant_mask; // it was a node so now it isn't
						self_size_delta -= 1;
					}
				}

				// Need to adjust for every octant with data after this one
				// Easy for insertion (everything is after), but that is only for insertion
				// In general, another node needs to be adjusted iff it points to an index beyond the start of this affected index
				if child_size_delta != 0 {
					for other_octant in 0..8 {
						if other_octant == octant { continue; } // Skip the inserting child
						let octant_mask = 1_u8 << (7 - other_octant);
						if node_mask & octant_mask != 0 {
							let mut octant_index = segment_start + octant_offset(other_octant, leaf_mask, node_mask, leaf_size);
							if leaf_mask & octant_mask != 0 { octant_index += leaf_size; } // Skip leaf

							// println!("Adjust octant {other_octant} (at index {octant_index}) with {child_size_delta}");

							let octant_node: &mut OctreeNode = data.get_mut(octant_index).unwrap().into();
							let other_octant_data_index = octant_index + octant_node.child_offset as usize + 1;
							// Adjust iff it points to a data index after (or eq) to changed stuff
							if other_octant_data_index < next_segment_start {
								// println!("Nvm not doing that");
								continue;
							}

							octant_node.child_offset = (octant_node.child_offset as i32 + child_size_delta) as u16;
						}
					}
				}

				(child_size_delta + self_size_delta, leaf_mask, node_mask)
			} else {
				let mut size_delta = 0;
				// This is the insertion point
				if leaf_mask & octant_mask != 0 {
					// println!("Overwrite leaf at {octant_index}");
					data.splice(octant_index..octant_index+leaf_size, item.iter().cloned());
				} else {
					// println!("Create leaf at {octant_index}");
					data.splice(octant_index..octant_index, item.iter().cloned());
					size_delta += leaf_size as i32;
					leaf_mask |= octant_mask;
				}
				(size_delta, leaf_mask, node_mask)
			}
		}

		let mut octant_iter = OctantCodeIterator::new(x, y, z, self.depth);
		let octant = octant_iter.next().unwrap();
		let item = Octree::extract_data(content, self.leaf_size);

		// println!("[{x}, {y}, {z}] ({:?}) <- {item:?}", OctantCodeIterator::new(x, y, z, self.depth).collect::<Vec<_>>());

		let (_, leaf, node) = insert_inner(
			0, 
			octant, 
			self.leaf, 
			self.node, 
			octant_iter, 
			self.leaf_size, 
			&mut self.data,
			item,
		);
		self.leaf = leaf;
		self.node = node; 

	}

	// There are more idiomatic ways to do this, but I want it to be easy to translate to GLSL
	pub fn get<'a>(&'a self, x: u32, y: u32, z: u32) -> Option<&'a [u32]> {
		let mut octant_iter = OctantCodeIterator::new(x, y, z, self.depth);

		let octant = octant_iter.next().unwrap();
		let octant_mask = 1 << (7 - octant);

		let mut has_leaf = self.leaf & octant_mask != 0;
		let mut has_node = self.node & octant_mask != 0;
		let mut index = octant_offset(octant, self.leaf, self.node, self.leaf_size);

		while let Some(octant) = octant_iter.next() {
			if !has_node {
				// println!("We're out of nodes, terminating early");
				break
			}

			// Do this with `+= leaf_size * has_leaf` in GLSL version
			if has_leaf {
				index += self.leaf_size;
			}

			let node: OctreeNode = self.data.get(index).unwrap().clone().into();

			let octant_mask = 1_u8 << (7 - octant);

			has_leaf = (node.leaf & octant_mask) != 0;
			has_node = (node.node & octant_mask) != 0;

			index += node.child_offset as usize + 1;
			index += node.octant_offset(octant, self.leaf_size);
		}

		if has_leaf {
			return Some(&self.data[index..index + self.leaf_size]);
		} else {
			return None;
		}
	}

	// Not fully tested
	// Todo: 
	// - combine octants if they are all of the same type (None)
	// - split combined octants if they are no longer of the same type
	// I've thought about combining this with insert() but I became very scared
	pub fn remove(&mut self, x: u32, y: u32, z: u32) {
		todo!()
	}

	// It's like insert but it can insert None
	// This way I can have seperate remove() and insert() implementations
	pub fn set(&mut self, x: u32, y: u32, z: u32, content: Option<&(impl Pod + Zeroable)>) {
		if let Some(content) = content {
			self.insert(x, y, z, content);
		} else {
			self.remove(x, y, z);
		}
	}
}


/// An iterator that reduces a 3d postion to its octant indices.
/// I've included a table to describe this. 
/// "p" is for positive relative to the centre and "n" is for negative relative to the centre. 
/// 
/// | position (xyz) | octant index |
/// | - | - |
/// | nnn | 0 |
/// | nnp | 1 |
/// | npn | 2 |
/// | npp | 3 |
/// | pnn | 4 |
/// | pnp | 5 |
/// | ppn | 6 |
/// | ppp | 7 |
#[derive(Debug)]
pub struct OctantCodeIterator {
	edge: u32,
	x: u32,
	y: u32,
	z: u32,
}
impl OctantCodeIterator {
	pub fn new(x: u32, y: u32, z: u32, depth: u32) -> Self {
		let edge = 2_u32.pow(depth);
		assert!(x < edge, "x coordiante exceeds edge length for depth {depth}!");
		assert!(y < edge, "y coordiante exceeds edge length for depth {depth}!");
		assert!(z < edge, "z coordiante exceeds edge length for depth {depth}!");
		Self {
			edge, x, y, z, 
		}
	}
}
impl Iterator for OctantCodeIterator {
	type Item = u8;
	fn next(&mut self) -> Option<Self::Item> {
		self.edge /= 2;
		let xp = self.x >= self.edge;
		let yp = self.y >= self.edge;
		let zp = self.z >= self.edge;

		if xp { self.x -= self.edge; }
		if yp { self.y -= self.edge; }
		if zp { self.z -= self.edge; }

		let octant = ((xp as u8) << 2) + ((yp as u8) << 1) + (zp as u8);

		if self.edge == 0 {
			None
		} else {
			Some(octant)
		}
	}
}


pub fn position_from_octant_code(code: &[u8]) -> [u32; 3] {
	let mut x = 0;
	let mut y = 0;
	let mut z = 0;

	let depth = code.len();
	let mut edge = 2_u32.pow(depth as u32) / 2;

	for octant in code.iter().cloned() {
		let xp = octant & 0b100 != 0;
		let yp = octant & 0b010 != 0;
		let zp = octant & 0b001 != 0;
		edge /= 2;
		if xp { x += 2_u32.pow(edge) }
		if yp { y += 2_u32.pow(edge) }
		if zp { z += 2_u32.pow(edge) }
	}

	[x, y, z]
}


fn octant_offset(octant: u8, leaf: u8, node: u8, leaf_size: usize) -> usize {
	let preced_mask = if octant == 8 {
		0b11111111_u8
	} else {
		!(0b11111111_u8.wrapping_shr(octant as u32))
	};
	let preceding_leaves = u8::count_ones(leaf & preced_mask) as usize;
	let preceding_nodes = u8::count_ones(node & preced_mask) as usize;
	let offset = preceding_leaves * leaf_size + preceding_nodes;
	offset
}


// Credit to Sven Marnach
// https://stackoverflow.com/questions/69396843/how-to-know-if-all-slice-elements-are-equal-and-if-so-return-a-reference-to-th
fn are_elements_equal<T: PartialEq>(elems: &[T]) -> Option<&T> {
	match elems {
		[head, tail @ ..] => tail.iter().all(|x| x == head).then(|| head),
		[] => None,
	}
}


// pub struct Octree2 {
// 	data: Vec<u32>,
// 	// Depth is not needed actually!
// 	leaf_size: u8,
// }
// impl Octree2 {
// 	pub fn new(depth: u8, leaf_size: u8) -> Self {
// 		Self {
// 			data: vec![OctreeNode {
// 				leaf: 0b00000000_u8,
// 				node: 0b00000000_u8,
// 				child_offset: 0,
// 			}.into()],
// 			// depth,
// 			leaf_size,
// 		}
// 	}

// 	/// Gets the leaf value of the root if it exists
// 	/// 
// 	/// This is a special case 
// 	fn root_leaf(&self) -> Option<&[u32]> {
// 		let root: OctreeNode = self.data[0].into();
// 		(root.child_offset != 0).then(|| &self.data[1..(1 + self.leaf_size as usize)])
// 	}

// 	fn root(&self) -> OctreeNode {
// 		self.data[0].into()
// 	}

// 	pub fn combine(
// 		nnn: Self, 
// 		nnp: Self, 
// 		npn: Self, 
// 		npp: Self, 
// 		pnn: Self, 
// 		pnp: Self, 
// 		ppn: Self, 
// 		ppp: Self, 
// 	) -> Self {
// 		// node then leaf this time
// 		let octants = [
// 			nnn, 
// 			nnp, 
// 			npn, 
// 			npp, 
// 			pnn, 
// 			pnp, 
// 			ppn, 
// 			ppp, 
// 		];
// 		// let depth = octants[0].depth + 1;
// 		// assert!(octants.iter().all(|o| o.depth == depth - 1), "Octants are not of same depth!");

// 		let leaf_size = octants[0].leaf_size;
// 		assert!(octants.iter().all(|o| o.leaf_size == leaf_size), "Octants do not have same leaf size!");

// 		// If all do not have children
// 		if octants.iter().all(|o| !o.root().has_children()) {
// 			// And all have same content
// 			let content = octants[0].root_leaf();
// 			if octants.iter().all(|o| o.root_leaf() == content) {
// 				// Combine them
// 				println!("WE CAN COMBINE THEM");
// 				// Can just return the first tree with one more depth
// 				let mut data = octants[0];
// 				data.depth += 1;
// 				return data;
// 			}
// 		}

// 		// Otherwise this is harder
// 		let mut 
// 		let child_offset = 0;
// 		for octant in octants {
// 			let mut data = octant.data;
			
// 			// Remove special case for root leaf flag
// 			let root_mut: &mut OctreeNode = data.get_mut(0).unwrap().into();
// 			let has_leaf = root_mut.child_offset != 0;
// 			root_mut.child_offset = 0;




// 		}

// 		Self {

// 		}
// 	}

// 	pub fn insert(
// 		&mut self,
// 		iterator: OctantCodeIterator,
// 	) {
// 		todo!()
// 	}
// }



