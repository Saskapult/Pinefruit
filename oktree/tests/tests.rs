use oktree::*;


#[test]
fn test_node_into_from() {
	let node = OctreeNode {
		child_offset: 42,
		node: 0b11001100,
		leaf: 0b00101000,
	};
	let packed: u32 = node.into();
	let unpacked = packed.into();

	assert_eq!(node, unpacked, "Unpacked is not equal to original!");
}


#[test]
fn test_octant_code_translation() {
	let depth = 2;
	let extent = 2_u32.pow(depth);

	for x in 0..extent {
		for y in 0..extent {
			for z in 0..extent {
				let code = OctantCodeIterator::new(x, y, z, depth).into_iter().collect::<Vec<_>>();
				let code_xyz = position_from_octant_code(&code[..]);
				let xyz = [x, y, z];
				assert_eq!(xyz, code_xyz, "Output does not equal input!");

			}
		}
	}	
}


// Insert into the leaf of an example tree
#[test]
fn test_overwrite() {
	let mut example_tree = Octree::new(2, 1);
	example_tree.print_tree();
	println!("{:#?}", example_tree);

	let [x, y, z] = [0, 1, 1];
	example_tree.insert(x, y, z, &52_u32);

	assert_eq!(Some([52].as_slice()), example_tree.get(x, y, z));
}


#[test]
fn test_insert_fill() {
	let depth = 2;

	let mut tree = Octree::new(depth, 1);
	tree.print_tree();

	let extent = 2_u32.pow(depth);
	
	let mut data = Vec::new();
	for x in 0..extent {
		for y in 0..extent {
			for z in 0..extent {
				let v = x * extent * extent + y * extent + z;
				data.push((x, y, z, v));
			}
		}
	}

	for (i, (x, y, z, v)) in data.iter().cloned().enumerate() {
		tree.insert(x, y, z, &v);
		tree.print_tree();
		tree.print_guess();
		
		
		for (j, (x2, y2, z2, v2)) in data[0..=i].iter().cloned().enumerate() {
			assert_eq!(
				Some([v2].as_slice()), 
				tree.get(x2, y2, z2),
				"insertion {i} ([{x}, {y}, {z}] ({:?}) <- {v}) breaks insertion {j} ([{x2}, {y2}, {z2}] ({:?}) <- {v2})", 
				OctantCodeIterator::new(x, y, z, depth).collect::<Vec<_>>(),
				OctantCodeIterator::new(x2, y2, z2, depth).collect::<Vec<_>>(),
			);
		}
	}

	tree.print_tree();
	
	for x in 0..extent {
		for y in 0..extent {
			for z in 0..extent {
				let v = x * extent * extent + y * extent + z;
				let g = [v];
				let v = g.as_slice();
				
				let c = tree.get(x, y, z);
				assert_eq!(Some(v), c, "[{x}, {y}, {z}]");
			}
		}
	}
}

// Insert into the leaf of an example tree
#[test]
fn test_remove() {
	let mut example_tree = Octree::new(2, 1);
	example_tree.print_tree();

	let [x, y, z] = [0, 0, 0];
	example_tree.remove(x, y, z);

	example_tree.print_tree();

	assert_eq!(None, example_tree.get(x, y, z));
}

#[test]
fn manual_insert_combine_content() {
	let mut tree = Octree::new(2, 1);

	tree.insert(2, 2, 2, &52);
	tree.insert(0, 0, 0, &42);
	tree.insert(0, 0, 1, &42);
	tree.insert(0, 1, 0, &42);
	tree.insert(0, 1, 1, &42);
	tree.insert(1, 0, 0, &42);
	tree.insert(1, 0, 1, &42);
	tree.insert(1, 1, 0, &42);
	tree.insert(1, 1, 1, &42);
	tree.print_guess();
	tree.print_tree();
	println!("Done");
}

#[test]
fn manual_insert_combine_none() {
	let mut tree = Octree::new(2, 1);

	tree.insert(2, 2, 2, &52);
	tree.remove(2, 2, 2);
	tree.print_guess();
	tree.print_tree();
	println!("Done");
}

// #[test]
// fn idk2() {
// 	let this = 5;

// 	let leaf_mask = 0b10010000_u8;
// 	let node_mask = 0b01010000_u8;
// 	let to_this = octant_offset(this, leaf_mask, node_mask, 1);
// 	let to_end = octant_offset(8, leaf_mask, node_mask, 1);
// 	let this_to_end = to_end - to_this;

// 	let b = to_this + 1 + this_to_end;

// 	for i in 0..16 {
// 		print!("{i:3}");
// 	}
// 	print!("\n");
// 	for i in 0..8 {
// 		let octant_mask = 1 << (7 - i);
// 		let leaf = leaf_mask & octant_mask != 0;
// 		let node = node_mask & octant_mask != 0;
// 		match (leaf, node) {
// 			(true, true) => print!(" ln"),
// 			(true, false) => print!("  l"),
// 			(false, true) => print!("  n"),
// 			(false, false) => print!("   "),
// 		}
// 	}
// 	print!("\n");
// 	for i in 0..16 {
// 		let mut space = 3;
// 		if i == to_this {
// 			print!("t");
// 			space -= 1;
// 		} else if i == to_end {
// 			print!("e");
// 			space -= 1;
// 		} else if i == b {
// 			print!("n");
// 			space -= 1;
// 		}
// 		print!("{:space$}", "", space=space);
// 	}
// 	println!("");

// }
