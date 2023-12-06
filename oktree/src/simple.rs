use crate::OctantCodeIterator;



#[derive(Debug, Default)]
pub enum Thing<T> {
	Subtree(u32),
	Leaf(T),
	#[default]
	Empty,
}

pub struct SimpleNode<T> {
	pub octants: [Thing<T>; 8],
}

pub struct SimpleOctree<T> {
	root: Thing<T>,
	nodes: Vec<SimpleNode<T>>, 
	depth: u32
}
impl<T: Clone> SimpleOctree<T> {
	pub fn new(depth: u32) -> Self {
		Self {
			root: Thing::Empty,
			nodes: Vec::new(),
			depth,
		}
	}

	pub fn get(&mut self, x: u32, y: u32, z: u32) -> Option<&T> {
		let mut iterator = OctantCodeIterator::new(x, y, z, self.depth);
		let mut curr = &self.root;
		while let Some(octant) = iterator.next() {
			match curr {
				&Thing::Subtree(s) => {
					curr = &self.nodes[s as usize].octants[octant as usize];
				},
				_ => break,
			}
		}
		match curr {
			Thing::Empty => None,
			Thing::Leaf(v) => Some(v),
			Thing::Subtree(_) => None,
		}
	}

	// pub fn insert(&mut self, x: u32, y: u32, z: u32, value: T) -> Option<&T> {
	// 	let mut iterator = OctantCodeIterator::new(x, y, z, self.depth);
	// 	let mut curr = &mut self.root;
	// 	while let Some(octant) = iterator.next() {
	// 		match curr {
	// 			Thing::Subtree(s) => {
	// 				curr = &mut self.nodes[*s as usize].octants[octant as usize];
	// 			},
	// 			_ => {
	// 				// self.nodes.push(SimpleNode::d)
	// 				*curr = Thing::Subtree(self.nodes.len() as u32);
	// 			},
	// 		}
	// 	}

	// 	todo!()
		
	// }
}
