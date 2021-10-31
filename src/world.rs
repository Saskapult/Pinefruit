use std::collections::HashMap;

/*
https://0fps.net/2012/01/14/an-analysis-of-minecraft-like-engines/

a run-length encoding of a string is formally equivalent to an interval tree representation

An interval tree is just an ordinary binary tree, where the key of each node is the start of a run and the value is the coordinate of the run
*/


struct World {
	chunks: HashMap<i32, Chunk>,
}

// A 32*32*32 area
// Should be hashed in a z-order curve
struct Chunk {
	runs: BTreeMap<i32, i32>,
}
impl Chunk {
	
	fn line() {

	}
	fn unline(data: String) {

	}
}
