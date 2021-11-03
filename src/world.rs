
use crate::model::model::Model;
use std::collections::BTreeMap;
use nalgebra::*;
use std::sync::mpsc;


struct World {
	models: Vec<Model>,
	instances: Vec<f32>,
}


struct Map {
	chunks: HashMap<i32, Chunk>,
	new_chunks: mpsc::Receiver,		// New chunks should be sent here
}















/*
https://0fps.net/2012/01/14/an-analysis-of-minecraft-like-engines/

a run-length encoding of a string is formally equivalent to an interval tree representation

An interval tree is just an ordinary binary tree, where the key of each node is the start of a run and the value is the coordinate of the run
*/

// I like z=up x=right y=forward
// This takes that and makes it boring (x=up, y=right, z-forward)
// pub const ks_to_ws: Matrix4 = 
// 	Matrix4::new_nonuniform_scaling(Vector3::new(0.0, -1.0, 0.0)) * 	// Flip y
// 	Matrix4::from_scaled_axis(Vector3::new(0.0, std::f32::pi, 0.0)) *	// Rotate half on y
// 	Matrix4::from_scaled_axis(Vector3::new(0.0, 0.0, std::f32::pi/2));	// Rotate quarter on z




// A 32*32*32 area
// Should be hashed in a z-order curve
struct Chunk {
	location: [i32; 3],			// Chunk coordinates in chunk coordinates
	runs: BTreeMap<i32, i32>,
}
impl Chunk {
	
	fn line() {

	}
	fn unline(data: String) {

	}
}
