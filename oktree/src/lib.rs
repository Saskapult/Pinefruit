pub mod simple;

use std::fmt::Debug;
use bytemuck::{Pod, Zeroable};

#[macro_use]
extern crate log;



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

	pub fn octant_offset2(&self, octant: u8, leaf_size: usize) -> usize {
		octant_offset2(octant, self.leaf, self.node, leaf_size)
	}

	// Used to get a data offset, data should be at index (parent_index + 1 + this)
	pub fn to_end_from(&self, octant: u8, leaf_size: usize) -> usize {
		let total = self.octant_offset(8, leaf_size);
		let preceding = self.octant_offset(octant + 1, leaf_size);
		total - preceding
	}

	pub fn has_subtree(&self) -> bool {
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
	pub fn remove(&mut self, _x: u32, _y: u32, _z: u32) {
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
#[derive(Debug, Clone, Copy)]
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


// octant_offset but the lower octants are on the right
fn octant_offset2(octant: u8, leaf: u8, node: u8, leaf_size: usize) -> usize {
	let preced_mask = if octant == 8 {
		0b11111111_u8
	} else {
		!(0xFF_u8.wrapping_shl(octant as u32))
	};
	// println!("Octant {} gives p-mask {:#010b}", octant, preced_mask);
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


pub struct Octree2 {
	// Always node AND THEN leaf, never leaf and then node
	data: Vec<u32>,
	// Depth is not needed actually!
	// It is implicit and should be known externally
	leaf_size: u8,
}
impl Octree2 {
	pub fn new(leaf_size: u8) -> Self {
		Self {
			data: vec![OctreeNode {
				leaf: 0b00000000_u8,
				node: 0b00000000_u8,
				child_offset: 0,
			}.into()],
			leaf_size,
		}
	}

	// For testing, probably remove once insert is done
	pub fn new_leaf(leaf_data: &[u32]) -> Self {
		let leaf_size = leaf_data.len();
		let mut data = vec![OctreeNode {
			leaf: 0b00000000_u8,
			node: 0b00000000_u8,
			child_offset: leaf_size as u16,
		}.into()];
		data.extend_from_slice(leaf_data);
		Self {
			data,
			leaf_size: leaf_size as u8,
		}
	}

	pub fn size(&self) -> usize {
		std::mem::size_of::<Self>() + self.data.len() * 4
	}

	pub fn data(&self) -> &[u32] {
		self.data.as_slice()
	}

	/// Gets the leaf value of the root if it exists
	/// 
	/// This is a special case 
	fn root_leaf(&self) -> Option<&[u32]> {
		let root: OctreeNode = self.data[0].into();
		(root.child_offset != 0).then(|| &self.data[1..(1 + self.leaf_size as usize)])
	}

	fn root(&self) -> OctreeNode {
		self.data[0].into()
	}

	// Test this please
	pub fn get(&self, mut iterator: OctantCodeIterator) -> Option<&[u32]> {
		let octant = iterator.next().unwrap(); // If none, do root leaf case

		let root = self.root();
		let mut has_leaf = (root.leaf & (1 << octant)) != 0;
		let mut has_node = (root.node & (1 << octant)) != 0;
		let mut index = 1 + octant_offset2(octant, root.leaf, root.node, self.leaf_size as usize);

		if self.root_leaf().is_some() {
			index += self.leaf_size as usize;
		}

		println!("Subtree {octant} at idx {index}");
		while let Some(octant) = iterator.next() {
			println!("Subtree {octant} at idx {index}");
			if !has_node {
				println!("This has no subtree, terminating early");
				break
			}

			let node: OctreeNode = self.data.get(index).unwrap().clone().into();

			has_leaf = (node.leaf & (1_u8 << octant)) != 0;
			has_node = (node.node & (1_u8 << octant)) != 0;

			index += node.child_offset as usize + 1;
			index += node.octant_offset2(octant, self.leaf_size as usize);
		}

		if has_node {
			println!("Skip over node");
			index += 1;
		}

		if has_leaf {
			println!("Leaf at idx {index}");
			return Some(&self.data[index..index + self.leaf_size as usize]);
		} else {
			println!("Empty");
			return None;
		}
	}

	pub fn combine(
		nnn: Self, 
		nnp: Self, 
		npn: Self, 
		npp: Self, 
		pnn: Self, 
		pnp: Self, 
		ppn: Self, 
		ppp: Self, 
	) -> Self {
		// node then leaf this time
		let octants = [
			nnn, 
			nnp, 
			npn, 
			npp, 
			pnn, 
			pnp, 
			ppn, 
			ppp, 
		];

		let leaf_size = octants[0].leaf_size;
		assert!(octants.iter().all(|o| o.leaf_size == leaf_size), "Octants do not have same leaf size!");

		// If all have no children
		if octants.iter().all(|o| !o.root().has_subtree()) {
			println!("All childless...");
			// If all leaves have same value
			let content = octants[0].root_leaf();
			if octants.iter().all(|o| o.root_leaf() == content) {
				// Combine them
				println!("Same content, we can combine them!");
				// Can just return the first tree
				let [g, _, _, _, _, _, _, _] = octants;
				return g
			} else {
				println!("But differing content!");
			}
		}

		// Make new root
		let mut new_root = OctreeNode {
			leaf: 0b00000000_u8,
			node: 0b00000000_u8,
			child_offset: 0,
		};
		// Make masks
		for (i, octant) in octants.iter().enumerate() {
			let octant_root: OctreeNode = octant.data[0].into();

			// Only include it as a node it it has children
			// Otherwise we just want the leaf value
			if octant_root.has_subtree() {
				println!("Octant {} has a subtree", i);
				new_root.node |= 1 << i;
			}

			// If it has leaf content, set that flag
			let root_leaf = octant_root.child_offset != 0;
			if root_leaf {
				println!("Octant {} has a leaf content", i);
				new_root.leaf |= 1 << i;
			}
		}
		
		// Use basic combination function of the leaves of each node
		let new_leaf = {
			let content = octants[0].root_leaf();
			if octants.iter().all(|o| o.root_leaf() == content) {
				content
			} else {
				None
			}
		};
		if let Some(s) = new_leaf {
			new_root.child_offset = leaf_size as u16;
			println!("Combined to content {s:?}");
		} else {
			println!("Combined to no content");
		}
		let mut new_data: Vec<u32> = vec![new_root.into()];
		if let Some(s) = new_leaf {
			new_data.extend_from_slice(s);
		}

		// Make nodes
		let mut pushed_data_size = 0;
		for (i, octant) in octants.iter().enumerate() {
			
			// If has a subtree, push a node to describe that
			let has_node = new_root.node & (1 << i) != 0;
			if has_node {
				let mut octant_root = octant.root();
				let to_end = new_root.to_end_from(i as u8, leaf_size as usize);
				// Not adding because leaf offset is built in to to_end_from
				octant_root.child_offset = (to_end + pushed_data_size) as u16;
				pushed_data_size += octant.data.len() - 1;

				println!("Push node for octant {} (offset will be is {})", i, octant_root.child_offset);

				new_data.push(octant_root.into());
			}

			// If has leaf data, push that
			let has_leaf = new_root.leaf & (1 << i) != 0;
			if has_leaf {
				println!("Push leaf for octant {}", i);
				new_data.extend_from_slice(&octant.data[1..(leaf_size as usize + 1)]);
			}
		}

		// Push subtrees
		for (i, octant) in octants.iter().enumerate() {
			let mut offset = 1;
			let has_leaf = new_root.leaf & (1 << i) != 0;
			if has_leaf {
				offset += leaf_size as usize;
			}

			let s = &octant.data[offset..];
			if s.len() != 0 {
				println!("Push subtree for octant {} (len is {})", i, s.len());
			}
			new_data.extend_from_slice(s);
		}

		Self {
			data: new_data,
			leaf_size
		}
	}

	// Accurate as long as your values are less than 2^16
	pub fn print_guess(&self) {
		let root = self.root();
		println!("0: {root:?}");
		for (i, value) in self.data[1..].iter().cloned().enumerate() {
			let i = i + 1;
			if value & 0xFFFF0000 > 0 {
				let node: OctreeNode = value.into();
				println!("{i}: {node:?}")
			} else {
				println!("{i}: {value}")	
			}
		}
	}

	pub fn print_pretty(&self) {
		fn print_rec(
			data: &[u32],
			leaf: u8, 
			node: u8, 
			depth: usize, 
			leaf_size: usize, 
		) {
			for octant in 0..8 {
				let octant_mask = 1 << octant;
				let mut octant_index = octant_offset2(octant, leaf, node, leaf_size);

				let has_leaf = leaf & octant_mask != 0;
				let has_node = node & octant_mask != 0;
				if has_leaf || has_node {
					// Print indentation
					print!("{:indent$}", "", indent=depth);
					// Print octant index
					print!("{octant}: ");
				}

				// Print identifier
				match (has_leaf, has_node) {
					(false, false) => {}, //print!("empty"),
					(true, false) => print!("leaf"),
					(false, true) => print!("node"),
					(true, true) => print!("leafnode"),
				}

				// Print leaf content
				if has_leaf {
					let leaf_data = &data[(octant_index)..(octant_index+leaf_size)];
					print!(" ({:?})", leaf_data);
					// Skip over leaf when going to children
					if has_leaf {
						octant_index += leaf_size;
					}
				}

				// if has_node {
				// 	let node: OctreeNode = data.get(octant_index).unwrap().clone().into();
				// 	print!(" ({:?})", node);
				// }

				// End line
				if has_leaf || has_node {
					print!("\n");
				}

				// Print rest of subtree
				if has_node {
					let node: OctreeNode = data.get(octant_index).unwrap().clone().into();
					let next_index = octant_index + node.child_offset as usize + 1;
					// println!("Recurse {next_index} away");
					print_rec(
						&data[next_index..],
						node.leaf, 
						node.node, 
						depth+1, 
						leaf_size, 
					)
				}
			}
		}

		let root = self.root();
		println!("root {root:?}");
		let mut offs = 1;
		if let Some(v) = self.root_leaf() {
			println!("{:?}", v);
			offs += self.leaf_size as usize;
		}
		if root.has_subtree() {
			// println!("offs {offs}");
			print_rec(&self.data[offs..], root.leaf, root.node, 1, self.leaf_size as usize);
		}		
	}

	// If data was created in a subtree, then we need to adjust the offset of everything that points to after that data was inserted
	#[inline]
	fn external_adjust(
		octant: u8, leaf_mask: u8, node_mask: u8, leaf_size: usize, 
		data_st: usize, data: &mut Vec<u32>, adjust: i32, 
	) {
		let octant_index = data_st + octant_offset2(octant, leaf_mask, node_mask, leaf_size);
		let octant_node: OctreeNode = data[octant_index].into();
		let octant_children_location = octant_index + octant_node.child_offset as usize + 1;
		for other_octant in 0..8 {
			if other_octant == octant { continue; } // Skip self
			// If has subtree
			if (node_mask & (1 << other_octant)) != 0 {
				let offs = octant_offset2(other_octant, leaf_mask, node_mask, leaf_size);
				let node_idx = data_st + offs;
				debug!("Adjust node for octant {} ({} + {} = idx {}) ({})", other_octant, data_st, offs, node_idx, adjust);

				let octant_node: &mut OctreeNode = (&mut data[node_idx]).into();

				// If subtree location is greater than the insertion point
				let other_children_location = node_idx + octant_node.child_offset as usize + 1;
				trace!("{} >= {}", other_children_location, octant_children_location);
				if other_children_location >= octant_children_location {
					let n = octant_node.child_offset as i32 + adjust;
					trace!("{} + {} = {}", octant_node.child_offset, adjust, n);
					octant_node.child_offset = n as u16;
				} else {
					trace!("Lol nvm");
				}						
			}
		}
	}

	// If data is created in this collection, then every subtree pointer before the creation point must be adjusted
	#[inline]
	fn internal_adjust(
		octant: u8, leaf_mask: u8, node_mask: u8, leaf_size: usize, 
		data_st: usize, data: &mut Vec<u32>, adjust: i32, 
	) {
		for other_octant in 0..octant {
			if (node_mask & (1 << other_octant)) != 0 {
				let offs = octant_offset2(other_octant, leaf_mask, node_mask, leaf_size);
				let node_idx = data_st + offs;
				debug!("Adjust node for (preceeding) octant {} ({} + {} = idx {}) ({})", other_octant, data_st, offs, node_idx, adjust);

				let octant_node: &mut OctreeNode = (&mut data[node_idx]).into();
				let n = octant_node.child_offset as i32 + adjust;
				trace!("{} + {} = {}", octant_node.child_offset, adjust, n);
				octant_node.child_offset = n as u16;
			}
		}
	}

	pub fn insert(
		&mut self,
		mut iterator: OctantCodeIterator,
		content: &(impl Pod + Zeroable),
	) {
		fn print_tree_guess(data: &Vec<u32>, leaf_size: usize) {
			Octree2 {
				data: data.clone(),
				leaf_size: leaf_size as u8,
			}.print_guess();
		}

		// Returns (new node, new leaf, offset adjustment)
		fn insert_rec(
			mut iterator: OctantCodeIterator,
			data: &mut Vec<u32>,
			octant: u8,
			leaf_mask: u8, // The parent's leaf mask
			mut node_mask: u8, // The parent's node mask
			leaf_size: usize,
			data_st: usize,
			content: &[u32],
		) -> (bool, bool, i32) {
			print_tree_guess(data, leaf_size);

			debug!("Index {}, octant {}, leaf {:010b}, node {:010b}", data_st, octant, leaf_mask, node_mask);
			if let Some(next_octant) = iterator.next() {
				debug!("Want to go to subtree octant {}", next_octant);
				// Find the index of the node for (octant)
				let octant_index = data_st + octant_offset2(octant, leaf_mask, node_mask, leaf_size);

				// If (octant) doesn't have a node entry
				// Insert a node there
				let mut before_adjust = 0; // Applied to offsets before octant
				let has_node = (node_mask & (1 << octant)) != 0;
				// println!("Mask is 0b{:010b} & 0b{:010b}", node_mask, 1 << octant);
				if !has_node {
					before_adjust += 1;
					node_mask |= 1 << octant;
					
					let subtree_offset = octant_offset2(8, leaf_mask, node_mask, leaf_size) - octant_offset2(octant+1, leaf_mask, node_mask, leaf_size);
					debug!("Doesn't have a subtree, create one at index {} with offset {}", octant_index, subtree_offset);
					data.insert(octant_index, OctreeNode {
						leaf: 0,
						node: 0,
						child_offset: subtree_offset as u16,
					}.into());
				}
				
				// Load node for (octant)
				let octant_node: OctreeNode = data[octant_index].into();
				let octant_children_location = octant_index + octant_node.child_offset as usize + 1;
				
				// Recurse at the location of its children
				debug!("Recurse");
				let (
					set_node, 
					set_leaf, 
					mut all_adjust, 
				) = insert_rec(
					iterator, 
					data, 
					next_octant, 
					octant_node.leaf, octant_node.node,
					leaf_size, 
					octant_children_location, 
					content, 
				);

				print_tree_guess(data, leaf_size);

				// Apply new_node, new_leaf to (octant)'s node entry
				// Misnomer: we have to set them to this in any case
				let octant_node: &mut OctreeNode = (&mut data[octant_index]).into();
				if set_node {
					// if (octant_node.node & (1 << next_octant)) == 0 {
					// 	trace!("They made a node");
					// }
					octant_node.node |= 1 << next_octant;
				} else {
					// if (octant_node.node & (1 << next_octant)) != 0 {
					// 	trace!("They removed a node");
					// }
					octant_node.node &= !(1 << next_octant);
				}
				if set_leaf {
					// if (octant_node.leaf & (1 << next_octant)) == 0 {
					// 	trace!("They made a leaf");
					// }
					octant_node.leaf |= 1 << next_octant;
				} else {
					// if (octant_node.leaf & (1 << next_octant)) != 0 {
					// 	trace!("They removed a leaf");
					// }
					octant_node.leaf &= !(1 << next_octant);
				}
				let octant_node = octant_node.clone();

				// print_tree_guess(data, leaf_size);

				// If all of (octant)'s node's children have no children 
				if octant_node.node == 0x00 {
					trace!("This node does not have subtrees...");
					// and have same leaf content
					// (look in octant_children_location)
					
					// Uses heap allocation?! Todo: don't do that! 
					let same = (0..8)
						.map(|i| if (octant_node.leaf & (1 << i)) != 0 {
							let o = octant_children_location + octant_node.octant_offset2(i as u8, leaf_size);
							trace!("Octant {i} has leaf, read at {o} ({} + {})", octant_children_location, octant_node.octant_offset2(i as u8, leaf_size));
							Some(&data[o..o+leaf_size])
						} else {
							None
						})
						.collect::<Vec<_>>()
						.windows(2).all(|w| w[0] == w[1]);

					if same {
						trace!("And same leaf values, we can combine them!");
						// Remove their entries and (octant)'s node becomes a leaf of that content
						
						let leaf_count = (0..8)
							.filter(|&i| (octant_node.leaf & (1 << i)) != 0)
							.count();

						// Remove subtree
						debug!("Remove subtree ({} leaves)", leaf_count);
						let _ = data.drain(octant_children_location..octant_children_location+leaf_count*leaf_size);
						all_adjust -= (8 * leaf_size) as i32;

						print_tree_guess(data, leaf_size);				

						// Remove node
						debug!("Remove node");
						data.remove(octant_index);
						before_adjust -= 1; 
						node_mask &= !(1 << octant);

						print_tree_guess(data, leaf_size);

						// Add or overwrite leaf
						let has_leaf = (leaf_mask & (1 << octant)) != 0;
						if has_leaf {
							debug!("Overwrite leaf");
							data.splice(octant_index..octant_index+leaf_size, content.iter().copied());
						} else {
							debug!("Create leaf");
							before_adjust += leaf_size as i32;
							data.splice(octant_index..octant_index, content.iter().copied());
						}

						// Make sure to return (false, true, _)
					} else {
						trace!("But has differing leaf values!");
						// Else perform the blending function and overwrite leaf with that
						// Todo: that
					}
				} else {
					// Else perform the blending function and overwrite leaf with that
					// Todo: that
				}

				// print_tree_guess(data, leaf_size);

				// println!("");
				Octree2::internal_adjust(octant, leaf_mask, node_mask, leaf_size, data_st, data, before_adjust);
				// print_tree_guess(data, leaf_size);

				// println!("");
				Octree2::external_adjust(octant, leaf_mask, node_mask, leaf_size, data_st, data, all_adjust);
				// print_tree_guess(data, leaf_size);

				let set_node = (node_mask & (1 << octant)) != 0;
				let set_leaf = (leaf_mask & (1 << octant)) != 0;

				let adjust = all_adjust + before_adjust;
				trace!("return {:?}", (set_node, set_leaf, adjust));
				(set_node, set_leaf, adjust)
			} else {
				// We're here!
				debug!("Terminal!");
				
				// If (octant) has a node, skip over that data for writing
				let mut leaf_idx = data_st + octant_offset2(octant, leaf_mask, node_mask, leaf_size);
				let has_node = (node_mask & (1 << octant)) != 0;
				if has_node {
					trace!("Has a node, so skipping over it");
					leaf_idx += 1;
				}

				// If (octant) did not have a leaf, add size to adjustment
				let mut before_adjust = 0;
				let has_leaf = (leaf_mask & (1 << octant)) != 0;
				if !has_leaf {
					trace!("Insert leaf at index {}", leaf_idx);
					data.splice(leaf_idx..leaf_idx, content.iter().copied());
					before_adjust += leaf_size as i32;
				} else {
					trace!("Overwrite leaf at index {}", leaf_idx);
					data.splice(leaf_idx..leaf_idx+leaf_size, content.iter().copied());
				}

				Octree2::internal_adjust(octant, leaf_mask, node_mask, leaf_size, data_st, data, before_adjust);

				trace!("return {:?}", (has_node, true, before_adjust));
				(has_node, true, before_adjust)
			}
		}

		let content = bytemuck::bytes_of(content);
		let content = bytemuck::try_cast_slice::<_, u32>(content)
			.expect("data content is not tetrabyte aligned!");
		let leaf_size = self.leaf_size as usize;
		assert_eq!(leaf_size, content.len());
		let root = self.root();

		if let Some(octant) = iterator.next() {
			let data_st = root.child_offset as usize + 1;
			let (set_node, set_leaf, _) = insert_rec(
				iterator, 
				&mut self.data, 
				octant, 
				root.leaf, 
				root.node, 
				leaf_size, 
				data_st, 
				content,
			);

			let root_mut: &mut OctreeNode = (&mut self.data[0]).into();
			if set_node {
				root_mut.node |= 1 << octant;
			} else {
				root_mut.node &= !(1 << octant);
			}
			if set_leaf {
				root_mut.leaf |= 1 << octant;
			} else {
				root_mut.leaf &= !(1 << octant);
			}

			// external_adjust(octant, root_mut.leaf, root_mut.node, leaf_size, data_st, &mut self.data, adjust);
		} else {
			if root.child_offset != 0 {
				debug!("Overwrite root leaf");
				self.data.splice(1..1+leaf_size, content.iter().copied());
			} else {
				debug!("Create root leaf");
				self.data.splice(1..1, content.iter().copied());
				let root_mut: &mut OctreeNode = (&mut self.data[0]).into();
				root_mut.child_offset = leaf_size as u16;
			}
		}
		
	}
	
	// This doesn't delete subtrees, only leaf values
	pub fn remove(
		&mut self,
		mut iterator: OctantCodeIterator,
	) {
		// This should be like insert but maybe more simple

		fn remove_rec(
			mut iterator: OctantCodeIterator,
			data: &mut Vec<u32>,
			octant: u8,
			leaf_mask: u8, // The parent's leaf mask
			mut node_mask: u8, // The parent's node mask
			leaf_size: usize,
			data_st: usize,
		) -> (bool, bool, i32) {
			if let Some(next_octant) = iterator.next() {
				let has_node = (node_mask & (1 << octant)) != 0;
				if has_node {
					// Load node for (octant)
					let octant_index = data_st + octant_offset2(octant, leaf_mask, node_mask, leaf_size);
					let octant_node: OctreeNode = data[octant_index].into();
					let octant_children_location = octant_index + octant_node.child_offset as usize + 1;

					let (
						set_node, 
						set_leaf, 
						all_adjust, // Applied to offsets before AND after octant
					) = remove_rec(
						iterator, 
						data, 
						next_octant, 
						octant_node.leaf, octant_node.node,
						leaf_size, 
						octant_children_location,  
					);

					// Apply new_node, new_leaf to (octant)'s node entry
					// Misnomer: we have to set them to this in any case
					let octant_node: &mut OctreeNode = (&mut data[octant_index]).into();
					if set_node {
						// if (octant_node.node & (1 << next_octant)) == 0 {
						// 	trace!("They made a node");
						// }
						octant_node.node |= 1 << next_octant;
					} else {
						// if (octant_node.node & (1 << next_octant)) != 0 {
						// 	trace!("They removed a node");
						// }
						octant_node.node &= !(1 << next_octant);
					}
					if set_leaf {
						// if (octant_node.leaf & (1 << next_octant)) == 0 {
						// 	trace!("They made a leaf");
						// }
						octant_node.leaf |= 1 << next_octant;
					} else {
						// if (octant_node.leaf & (1 << next_octant)) != 0 {
						// 	trace!("They removed a leaf");
						// }
						octant_node.leaf &= !(1 << next_octant);
					}
					let octant_node = octant_node.clone();

					// Can we combine them?
					// Special case: are all empty leaves
					let mut before_adjust = 0;
					if octant_node.node == 0x00 && octant_node.leaf == 0x00 {
						trace!("Can combine leaf values (all empty)");
						data.remove(octant_index);
						before_adjust -= 1;
						node_mask &= !(1 << octant);
						// Idk if this will work, my head hurts
					}

					Octree2::internal_adjust(octant, leaf_mask, node_mask, leaf_size, data_st, data, before_adjust);
					Octree2::external_adjust(octant, leaf_mask, node_mask, leaf_size, data_st, data, all_adjust);

					let set_node = (node_mask & (1 << octant)) != 0;
					let set_leaf = (leaf_mask & (1 << octant)) != 0;

					let adjust = all_adjust + before_adjust;
					trace!("return {:?}", (set_node, set_leaf, adjust));
					(set_node, set_leaf, adjust)
				} else {
					// can't go deeper, terminate
					trace!("Terminated before reaching end");
					let has_leaf = (leaf_mask & (1 << octant)) != 0;
					(has_node, has_leaf, 0)
				}
			} else {
				// remove leaf if exists
				let mut leaf_idx = data_st + octant_offset2(octant, leaf_mask, node_mask, leaf_size);

				let has_node = (node_mask & (1 << octant)) != 0;
				if has_node {
					trace!("Has a node, so skipping over it");
					leaf_idx += 1;
				}

				let mut before_adjust = 0;
				let has_leaf = (leaf_mask & (1 << octant)) != 0;
				if has_leaf {
					before_adjust -= leaf_size as i32;
					data.drain(leaf_idx..leaf_idx+leaf_size);
				} else {
					trace!("Nothing to remove!");
				}

				Octree2::internal_adjust(octant, leaf_mask, node_mask, leaf_size, data_st, data, before_adjust);

				(has_node, false, before_adjust)
			}
		}

		let leaf_size = self.leaf_size as usize;
		let root = self.root();

		if let Some(octant) = iterator.next() {
			let data_st = root.child_offset as usize + 1;
			let (set_node, set_leaf, _) = remove_rec(
				iterator, 
				&mut self.data, 
				octant, 
				root.leaf, 
				root.node, 
				leaf_size, 
				data_st, 
			);

			let root_mut: &mut OctreeNode = (&mut self.data[0]).into();
			if set_node {
				root_mut.node |= 1 << octant;
			} else {
				root_mut.node &= !(1 << octant);
			}
			if set_leaf {
				root_mut.leaf |= 1 << octant;
			} else {
				root_mut.leaf &= !(1 << octant);
			}
		} else {
			// Remove root leaf if exists
			if root.child_offset != 0 {
				self.data.drain(1..1+leaf_size);
				let root_mut: &mut OctreeNode = (&mut self.data[0]).into();
				root_mut.child_offset = 0;
			}
		}
	}

	pub fn set(&mut self, iterator: OctantCodeIterator, content: Option<&(impl Pod + Zeroable)>) {
		match content {
			Some(content) => self.insert(iterator, content),
			None => self.remove(iterator),
		}
	}
}


#[cfg(test)]
mod tests {
    use crate::{Octree2, OctantCodeIterator};

	#[test]
	fn test_print_leaf() {
		let o = Octree2::new_leaf(&[42]);

		o.print_pretty()
	}

	#[test]
	fn test_combine_empty() {
		let o = Octree2::combine(
			Octree2::new(1), 
			Octree2::new(1), 
			Octree2::new(1), 
			Octree2::new(1), 
			Octree2::new(1), 
			Octree2::new(1), 
			Octree2::new(1), 
			Octree2::new(1),
		);

		o.print_guess()
	}

	#[test]
	fn test_combine_leaf_same() {
		let o = Octree2::combine(
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[42]),
		);

		o.print_guess()
	}

	#[test]
	fn test_combine_leaf_different() {
		let o = Octree2::combine(
			Octree2::new_leaf(&[0]), 
			Octree2::new_leaf(&[1]), 
			Octree2::new_leaf(&[2]), 
			Octree2::new_leaf(&[3]), 
			Octree2::new_leaf(&[4]), 
			Octree2::new_leaf(&[5]), 
			Octree2::new_leaf(&[6]), 
			Octree2::new_leaf(&[7]),
		);

		o.print_guess();
		o.print_pretty();
	}

	#[test]
	fn test_combine_subtree() {
		let o1 = Octree2::combine(
			Octree2::new_leaf(&[0]), 
			Octree2::new_leaf(&[1]), 
			Octree2::new_leaf(&[2]), 
			Octree2::new_leaf(&[3]), 
			Octree2::new_leaf(&[4]), 
			Octree2::new_leaf(&[5]), 
			Octree2::new_leaf(&[6]), 
			Octree2::new_leaf(&[7]),
		);

		let o = Octree2::combine(
			o1,
			Octree2::new_leaf(&[41]), 
			Octree2::new_leaf(&[42]), 
			Octree2::new_leaf(&[43]), 
			Octree2::new_leaf(&[44]), 
			Octree2::new_leaf(&[45]), 
			Octree2::new_leaf(&[46]), 
			Octree2::new_leaf(&[47]),
		);

		o.print_guess();
		o.print_pretty();
	}

	#[test]
	fn test_combine_subtrees() {
		let o1 = Octree2::combine(
			Octree2::new_leaf(&[0]), 
			Octree2::new_leaf(&[1]), 
			Octree2::new_leaf(&[2]), 
			Octree2::new_leaf(&[3]), 
			Octree2::new_leaf(&[4]), 
			Octree2::new_leaf(&[5]), 
			Octree2::new_leaf(&[6]), 
			Octree2::new_leaf(&[7]),
		);

		let o2 = Octree2::combine(
			Octree2::new_leaf(&[30]), 
			Octree2::new_leaf(&[31]), 
			Octree2::new_leaf(&[32]), 
			Octree2::new_leaf(&[33]), 
			Octree2::new_leaf(&[34]), 
			Octree2::new_leaf(&[35]), 
			Octree2::new_leaf(&[36]), 
			Octree2::new_leaf(&[37]),
		);

		let o = Octree2::combine(
			o1,
			Octree2::new_leaf(&[41]), 
			Octree2::new_leaf(&[42]), 
			o2, 
			Octree2::new_leaf(&[44]), 
			Octree2::new_leaf(&[45]), 
			Octree2::new_leaf(&[46]), 
			Octree2::new_leaf(&[47]),
		);

		o.print_guess();
		o.print_pretty();
	}

	#[test]
	fn test_insert_root_empty() {
		let mut o = Octree2::new(1);

		o.insert(OctantCodeIterator::new(0, 0, 0, 0), &[42]);

		println!("-");
		o.print_guess();
		println!("-");
		o.print_pretty();
	}

	#[test]
	fn test_insert_root_leaf() {
		let mut o = Octree2::new_leaf(&[41]);

		o.insert(OctantCodeIterator::new(0, 0, 0, 0), &[42]);

		println!("-");
		o.print_guess();
		println!("-");
		o.print_pretty();
	}

	#[test]
	fn test_insert_subtree_empty() {
		let mut o = Octree2::new(1);

		o.insert(OctantCodeIterator::new(0, 0, 0, 2), &[42]);

		println!("-");
		o.print_guess();
		println!("-");
		o.print_pretty();
	}

	#[test]
	fn test_insert_subtree_empty_overwrite() {
		let mut o = Octree2::new(1);

		let items = [
			[3, 0, 0],
			[0, 0, 0],
		].iter().copied().collect::<Vec<_>>();

		for (i, [x, y, z]) in items.iter().copied().enumerate() {
			println!("Insert {i} at [{x}, {y}, {z}]");
			o.insert(OctantCodeIterator::new(x, y, z, 2), &[i as u32]);
			println!("Now it looks like:");
			o.print_guess();
			println!("-");
			o.print_pretty();
			println!("\n-\n");
		}
		
		for (i, [x, y, z]) in items.iter().copied().enumerate() {
			println!("Get at [{x}, {y}, {z}] ({i})");
			let v = o.get(OctantCodeIterator::new(x, y, z, 2));
			let g = [i as u32];
			let int = Some(g.as_slice());
			assert_eq!(int, v);
		}
	}

	#[test]
	fn test_insert_subtree_group() {
		let mut o = Octree2::new(1);

		let depth = 2;
		let max = 2_u32.pow(depth);

		let mut previous = Vec::new();
		let mut i = 0;
		for x in 0..max {
			for y in 0..max {
				for z in 0..max {
					println!("-");
					o.print_pretty();
					o.print_guess();

					println!("Insert {i} to [{x}, {y}, {z}]");
					o.insert(OctantCodeIterator::new(x, y, z, depth), &[i]);
					
					println!("Check integrity...");
					o.print_guess();
					previous.push((i, (x, y, z)));
					for (v, (x, y, z)) in previous.iter().copied() {
						let i = OctantCodeIterator::new(x, y, z, depth);
						let octants = i.collect::<Vec<_>>();
						println!("[{x}, {y}, {z}] ({octants:?}) should be {v}");

						let actual = o.get(i);
						assert_eq!(Some([v].as_slice()), actual);
					}

					i += 1;
				}
			}
		}

		println!("-");
		o.print_guess();
		println!("-");
		o.print_pretty();

		o.remove(OctantCodeIterator::new(1, 1, 1, depth));
		assert_eq!(None, o.get(OctantCodeIterator::new(1, 1, 1, depth)));
	}

	#[test]
	fn test_get() {
		let mut o = Octree2::new(1);

		o.insert(OctantCodeIterator::new(0, 0, 0, 2), &[42]);

		println!("-");
		o.print_guess();
		println!("-");
		o.print_pretty();
		
		let v = o.get(OctantCodeIterator::new(0, 0, 0, 2));
		println!("{v:?}");
	}
}
