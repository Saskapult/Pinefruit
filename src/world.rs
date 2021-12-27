use nalgebra::*;
use crate::render::*;
use std::collections::HashMap;
use crate::indexmap::IndexMap;


const CHUNKSIZE: usize = 4;
const CHUNKSIZE_SQUARED: usize = CHUNKSIZE * CHUNKSIZE;
const CHUNKSIZE_CUBED: usize = CHUNKSIZE * CHUNKSIZE * CHUNKSIZE;

const CHUNKSIZE_I32: i32 = CHUNKSIZE as i32;
const CHUNKSIZE_SQUARED_I32: i32 = CHUNKSIZE_I32 * CHUNKSIZE_I32;
const CHUNKSIZE_CUBED_I32: i32 = CHUNKSIZE_I32 * CHUNKSIZE_I32 * CHUNKSIZE_I32;

const YP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(-0.5, 0.5, 0.5),	// top left
	Vector3::new(-0.5, 0.5, -0.5),	// bottom left
	Vector3::new(0.5, 0.5, -0.5),	// bottom right
	Vector3::new(0.5, 0.5, 0.5),	// top right
];
const XP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(0.5, 0.5, -0.5),
	Vector3::new(0.5, -0.5, -0.5),
	Vector3::new(0.5, -0.5, 0.5),
	Vector3::new(0.5, 0.5, 0.5),
];
const ZP_QUAD_VERTICES: [Vector3<f32>; 4] = [
	Vector3::new(-0.5, 0.5, 0.5),
	Vector3::new(-0.5, -0.5, 0.5),
	Vector3::new(0.5, -0.5, 0.5),
	Vector3::new(0.5, 0.5, 0.5),
];

const QUAD_TCS: [[f32; 2]; 4] = [
	[0.0, 0.0],
	[0.0, 1.0],
	[1.0, 1.0],
	[1.0, 0.0],
];
const QUAD_INDICES: [u16; 6] = [
	0, 1, 2,
	2, 3, 0, 
];



#[derive(Debug)]
pub struct Map {
	chunks: HashMap<[i32; 3], Chunk>,
	pub blockmap: IndexMap<BlockData>,
}
impl Map {
	pub fn new() -> Self {
		let mut chunks = HashMap::new();
		let blockmap = IndexMap::new();

		use noise::{NoiseFn, Perlin};
		let noisy = Perlin::new();
		let wdim = 10;
		for x in -wdim..=wdim {
			for y in -wdim..=wdim {
				for z in -wdim..=wdim {
					let loc = [x, y, z];
					let mut c = Chunk::new(loc);
					let bidx = (noisy.get([x as f64, y as f64, z as f64]) * 2.0) as usize;
					if bidx > 0 {
						c.contents = c.contents.all(Voxel::Block(bidx));
					}
					chunks.insert(loc, c);
				}
			}
		}

		Self {
			chunks,
			blockmap,
		}
	}


	pub fn is_chunk_loaded(&self, position: &[i32; 3]) -> bool {
		self.chunks.contains_key(position)
	}


	// Mesh a chunk with respect to those around it
	// This will look bad if seen from an side without a chunk before it
	pub fn mesh_chunk(&self, position: &[i32; 3]) -> (Vec<Vertex>, Vec<u16>) {
		let [px, py, pz] = *position;
		
		let main_chunk = self.get_chunk(position).expect("Tried to mesh unloaded chunk!");
		
		// Copy slices of neighbours
		// If the neighbour is not loaded, pretend there is nothing there (this is bad)
		let xp_slice = match self.get_chunk(&[px+1, py, pz]) {
			Some(chunk) => {
				let mut slice = [Voxel::Empty; CHUNKSIZE_SQUARED];
				for y in 0..CHUNKSIZE {
					for z in 0..CHUNKSIZE {
						// Cache says bad bad bad?
						slice[y*CHUNKSIZE + z] = chunk.get_voxel(0, y as i32, z as i32);
					}
				}
				slice
			},
			None => [Voxel::Empty; CHUNKSIZE_SQUARED],
		};
		
		let yp_slice = [Voxel::Empty; CHUNKSIZE_SQUARED];
		let zp_slice = [Voxel::Empty; CHUNKSIZE_SQUARED];

		let slices = [&xp_slice, &yp_slice, &zp_slice];

		better_mesh(&main_chunk.contents.contents, &slices, &self.blockmap)
	}


	pub fn chunks_around(&self, centre: [i32; 3], radius: i32) -> Vec<[i32; 3]> {
		let [cx, cy, cz] = centre;
		let mut res = Vec::new();
		// Consider bounding cube
		for x in (cx-radius)..(cx+radius+1) {
			for y in (cy-radius)..(cy+radius+1) {
				for z in (cz-radius)..(cz+radius+1) {
					// If in sphere
					if x^2 + y^2 + z^2 < radius {
						res.push([x,y,z]);
					}
				}
			}
		}
		res
	}


	pub fn get_chunk(&self, cpos: &[i32; 3]) -> Option<&Chunk> {
		if self.chunks.contains_key(cpos) {
			Some(&self.chunks[cpos])
		} else {
			// Make future to load the chunk?
			None
		}
	}


	pub fn chunk_worldpos(&self, cpos: [i32; 3]) -> Vector3<f32> {
		Vector3::new(
			(cpos[0] * CHUNKSIZE_I32) as f32,
			(cpos[1] * CHUNKSIZE_I32) as f32,
			(cpos[2] * CHUNKSIZE_I32) as f32,
		)
	}


	pub fn get_voxel(&self) {
		todo!();
	}
	pub fn set_voxel(&mut self) {
		todo!();
	}


	// Gets the coordinates of the chunk in which this point resides
	pub fn chunk_of(&self, position: Vector3<f32>) -> [i32; 3] {
		let chunk_pos = [
			(position[0] / (CHUNKSIZE as f32)).floor() as i32,
			(position[1] / (CHUNKSIZE as f32)).floor() as i32,
			(position[2] / (CHUNKSIZE as f32)).floor() as i32,
		];
		chunk_pos
	}
}



#[derive(Debug)]
pub struct Chunk {
	location: [i32; 3],	// Location in chunk coordinates
	contents: ArrayChunk,
}
impl Chunk {
	pub fn new(location: [i32; 3]) -> Self {
		let contents = ArrayChunk::new();
		Self {
			location,
			contents,
		}
	}


	pub fn fill(self, voxel: Voxel) -> Self {
		Self {
			location: self.location,
			contents: self.contents.all(voxel),
		}
	}


	// Get the map position of a voxel position
	pub fn voxel_map_pos(&self, x: i32, y: i32, z: i32) -> [i32; 3] {
		let mx = x + self.location[0] * CHUNKSIZE_I32;
		let my = y + self.location[0] * CHUNKSIZE_I32;
		let mz = z + self.location[0] * CHUNKSIZE_I32;
		[mx, my, mz]
	}


	// Get the voxel position of a map position
	pub fn map_pos_voxel(&self, x: i32, y: i32, z: i32) -> [i32; 3] {
		let vx = x % CHUNKSIZE_I32;
		let vy = y % CHUNKSIZE_I32;
		let vz = z % CHUNKSIZE_I32;
		[vx, vy, vz]
	}


	fn get_voxel(&self, x: i32, y: i32, z: i32) -> Voxel {
		self.contents.get_voxel([x, y, z])
	}


	fn set_voxel(&mut self, x: i32, y: i32, z: i32, to: Voxel) {
		self.contents.set_voxel([x, y, z], to);
	}


	fn is_in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
		(x < CHUNKSIZE_I32 && y < CHUNKSIZE_I32 && z < CHUNKSIZE_I32) && (x >= 0 && y >= 0 && z >= 0)
	}
	

	pub fn simple_mesh(&self) -> (Vec<Vertex>, Vec<u16>) {
		

		let mut mesh_vertices = Vec::new();
		let mut mesh_indices = Vec::new();

		let directions = [
			Direction::Xp,
			Direction::Yp,
			Direction::Zp,
		];

		for y in 0..CHUNKSIZE {
			for x in 0..CHUNKSIZE {
				for z in 0..CHUNKSIZE {

					// for direction in &directions {
					// }

					// If there should be a face here
					if self.face(Direction::Yp, x as i32, y as i32, z as i32) {
						println!("{} {} {} should have Yp face", x, y, z);

						let l = mesh_vertices.len() as u16;
						for index in &QUAD_INDICES {
							mesh_indices.push(l + *index);
						}

						let offset = Vector3::new(x as f32, y as f32, z as f32);
						for i in 0..YP_QUAD_VERTICES.len() {
							let pt = offset + YP_QUAD_VERTICES[i];
							let v = Vertex {
								position: pt.into(),
								tex_coords: QUAD_TCS[i],
								normal: [0.0, 1.0, 0.0],
								tex_id: 0,
							};
							mesh_vertices.push(v);
						}
					}
					if self.face(Direction::Xp, x as i32, y as i32, z as i32) {
						println!("{} {} {} should have Xp face", x, y, z);

						let l = mesh_vertices.len() as u16;
						for index in &QUAD_INDICES {
							mesh_indices.push(l + *index);
						}

						let offset = Vector3::new(x as f32, y as f32, z as f32);
						for i in 0..XP_QUAD_VERTICES.len() {
							let pt = offset + XP_QUAD_VERTICES[i];
							let v = Vertex {
								position: pt.into(),
								tex_coords: QUAD_TCS[i],
								normal: [1.0, 0.0, 0.0],
								tex_id: 0,
							};
							mesh_vertices.push(v);
						}
					}
					if self.face(Direction::Zp, x as i32, y as i32, z as i32) {
						println!("{} {} {} should have Zp face", x, y, z);

						let l = mesh_vertices.len() as u16;
						for index in &QUAD_INDICES {
							mesh_indices.push(l + *index);
						}

						let offset = Vector3::new(x as f32, y as f32, z as f32);
						for i in 0..ZP_QUAD_VERTICES.len() {
							let pt = offset + ZP_QUAD_VERTICES[i];
							let v = Vertex {
								position: pt.into(),
								tex_coords: QUAD_TCS[i],
								normal: [0.0, 0.0, 1.0],
								tex_id: 0,
							};
							mesh_vertices.push(v);
						}
					}
				}
			}
		}

		(mesh_vertices, mesh_indices)
	}


	// Should this block have a face in this direction?
	fn face(&self, direction: Direction, x: i32, y: i32, z: i32) -> bool {
		// We retreive the voxel for every direction test, which is badd
		let voxel = self.get_voxel(x, y, z);
		
		match voxel {
			Voxel::Empty => false,
			Voxel::Block(_bid) => {

				let [other_x, other_y, other_z] = match direction {
					Direction::Yp => [x, y+1, z],
					Direction::Yn => [x, y-1, z],
					Direction::Xp => [x+1, y, z],
					Direction::Xn => [x-1, y, z],
					Direction::Zp => [x, y, z+1],
					Direction::Zn => [x, y, z-1],
				};
				
				if self.is_in_bounds(other_x, other_y, other_z) {
					let other_voxel = self.get_voxel(other_x, other_y, other_z);
					match other_voxel {
						Voxel::Empty => true,
						Voxel::Block(_id) => false, // Should test if block is transparent
					}
				} else {
					true
				}
			},
		}
	}

}



// A chunk data container
trait ChunkData {
	fn new() -> Self;
	fn all(self, voxel: Voxel) -> Self;
	fn get_voxel(&self, position: [i32; 3]) -> Voxel;
	fn set_voxel(&mut self, position: [i32; 3], voxel: Voxel);
}



// Chunk data stored as an array
#[derive(Debug)]
struct ArrayChunk {
	contents: [Voxel; CHUNKSIZE_CUBED]	
}
impl ChunkData for ArrayChunk {
	fn new() -> Self {
		let contents = [Voxel::Empty; CHUNKSIZE_CUBED];
		Self {
			contents,
		}
	}
	fn all(self, voxel: Voxel) -> Self {
		let contents = [voxel; CHUNKSIZE_CUBED];
		Self {
			contents,
		}
	}
	fn get_voxel(&self, position: [i32; 3]) -> Voxel {
		let idx = position[0] as usize;
		self.contents[idx]
	}
	fn set_voxel(&mut self, position: [i32; 3], voxel: Voxel) {
		let idx = position[0] as usize;
		self.contents[idx] = voxel;
	}
}



#[derive(Debug, Copy, Clone)]
enum Direction {
	Xp,
	Xn,
	Yp,
	Yn,
	Zp,
	Zn,
}



#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Voxel {
	Empty,
	Block(usize),
}



#[derive(Debug)]
pub struct BlockData {
	name: String,
	material_id: u32,
	transparent: bool,
}



const DIRECTION_HELPER: [(Direction, [usize; 3]); 3] = [
	(Direction::Xp, [1, 0, 0]),
	(Direction::Yp, [0, 1, 0]),
	(Direction::Zp, [0, 0, 1]),
];



// Never generate faces for negativemost blocks, they are covered by their chunks
// Maybe pass indices directly instead of Voxel
// Empty could be 0 and all non zero are decremented to get their actual id
pub fn better_mesh(
	contents: &[Voxel; CHUNKSIZE_CUBED],
	neighbour_slices: &[&[Voxel; CHUNKSIZE_SQUARED]; 3], // xp, yp, zp
	blockmap: &IndexMap<BlockData>,
) -> (Vec<Vertex>, Vec<u16>) {
	let mut mesh_vertices = Vec::new();
	let mut mesh_indices = Vec::new();

	// Because we are considering the positivemost slice and not considering the negativemost slice there will be CHUNKSIZE slices

	for (direction, direction_vector) in &DIRECTION_HELPER {
		// println!("Meshing {:?}", direction);

		for x in 0..CHUNKSIZE {
			// Could we create an "a" slice and a "b" slice?
			// When ending iteration "b" becomes "a" and we only need to read the new "b"
			for y in 0..CHUNKSIZE {
				for z in 0..CHUNKSIZE {

					// Get blocks to compare
					let a = contents[x*CHUNKSIZE_SQUARED + y*CHUNKSIZE + z];
					let bx = x+direction_vector[0];
					let by = y+direction_vector[1];
					let bz = z+direction_vector[2];
					let b = {
						// These *should* already be cache-optimized, don't worry about it
						if bx == CHUNKSIZE {
							neighbour_slices[0][by*CHUNKSIZE + bz]
						} else if by == CHUNKSIZE {
							neighbour_slices[1][bx*CHUNKSIZE + bz]
						} else if bz == CHUNKSIZE {
							neighbour_slices[2][bx*CHUNKSIZE + by]
						} else {
							contents[bx*CHUNKSIZE_SQUARED + by*CHUNKSIZE + bz]
						}
					};

					// Are they transparent? (this is crude make it better)
					let mut a_index = 0;
					let at = match a {Voxel::Empty => true, Voxel::Block(idx) => {a_index = idx; false}};
					let mut b_index = 0;
					let bt = match b {Voxel::Empty => true, Voxel::Block(idx) => {b_index = idx; false}};
					
					// Note: we don't have to worry about transparent-opaque boundaries because they will be rendered in another pass

					// a opaque b transparent
					// Make positive face for a
					if !at && bt {
						let atexturei = blockmap.get_index(a_index).material_id;

						// Indices
						let l = mesh_vertices.len() as u16;
						QUAD_INDICES.iter().for_each(|index| mesh_indices.push(l + *index));

						// Vertices
						let quad_verts = match direction {
							Direction::Yp => YP_QUAD_VERTICES,
							Direction::Xp => XP_QUAD_VERTICES,
							Direction::Zp => ZP_QUAD_VERTICES,
							_ => YP_QUAD_VERTICES,
						};
						let offset = Vector3::new(x as f32, y as f32, z as f32);
						let normal = [
							direction_vector[0] as f32, 
							direction_vector[1] as f32, 
							direction_vector[2] as f32, 
						];
						for i in 0..quad_verts.len() {
							let pt = offset + quad_verts[i];
							let v = Vertex {
								position: pt.into(),
								tex_coords: QUAD_TCS[i],
								normal,
								tex_id: atexturei,
							};
							mesh_vertices.push(v);
						}
					}
					// a transparent b opaque
					// Make negative face for b
					if at && !bt {
						println!("Making a negative face for b");
					}
				}
			}
		}
	}

	(mesh_vertices, mesh_indices)
}






#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_meshing() {

		let array = [Voxel::Block(0); CHUNKSIZE_CUBED];
		let slices = [&[Voxel::Empty; CHUNKSIZE_SQUARED]; 3];
		//let res = better_mesh(&array, &slices);

		assert!(false);
	}
}



// fn greedy_mesh(&self) {
// 	let mut mesh_vertices = Vec::new();
// 	// let mut mesh_indices = Vec::new();

	
	
// 	// The layer the mask uses (should iterate in future)
// 	let y = CHUNKSIZE-1;
// 	let yi32 = y as i32;
// 	println!("Layer y = {}", &y);

// 	// The direction of the mask
// 	let direction = Direction::Yp;

// 	// This layer's visited mask
// 	let mut visited = [false; CHUNKSIZE_SQUARED];

// 	// For each x slice
// 	for mut x in 0..CHUNKSIZE {
// 		let xi32 = x as i32;
// 		//println!("examining x {}", &x);

// 		// For each z slice
// 		for mut z in 0..CHUNKSIZE {
// 			let zi32 = z as i32;
// 			//println!("examining z {}", &z);

// 			if !visited[x + z*CHUNKSIZE] {
// 				visited[x + z*CHUNKSIZE] = true;
// 				println!("visiting x,z = {},{}", &x, &z);

// 				if self.face(direction, xi32, yi32, zi32) {

// 					// Found starting point
// 					let start = (x, z);
// 					println!("start: {:?}", &start);
					
// 					// extend x until invalid, marking as visited
// 					while x+1 < CHUNKSIZE && !visited[x+1 + z*CHUNKSIZE] && self.face(direction, (x+1) as i32, y as i32, z as i32) {
// 						x = x+1;
// 						visited[x + z*CHUNKSIZE] = true;
// 					}
// 					println!("x: {} -> {}", &start.0, &x);


// 					let vmap = visited.map(|b| if b {"#"} else {"_"});
// 					for i in 0..vmap.len() {
// 						print!("{}", vmap[i]);
// 						if i % CHUNKSIZE == CHUNKSIZE-1 {
// 							println!("");
// 						}
// 					}
// 					println!("");


// 					// extend z until any invalid, marking as visited
// 					while z+1 < CHUNKSIZE && !(start.0..x+1).map(|x| {
// 						!visited[x + (z+1)*CHUNKSIZE] // Not visited element in row
// 						&& 
// 						self.face(direction, x as i32, y as i32, (z+1) as i32) // Element has exposed face
// 					}).any(|x| !x) { // All true (none false)
// 						z = z+1;
// 						// Set the row as visited
// 						for tempx in start.0..x+1 {
// 							visited[tempx + z*CHUNKSIZE] = true;
// 						}

// 						let vmap = visited.map(|b| if b {"#"} else {"_"});
// 						for i in 0..vmap.len() {
// 							print!("{}", vmap[i]);
// 							if i % CHUNKSIZE == CHUNKSIZE-1 {
// 								println!("");
// 							}
// 						}
// 						println!("");

// 					}
// 					println!("z: {} -> {}", &start.1, &z);

// 					// Make quad
// 					let end = (x, z);
// 					println!("st {:?}, end {:?}", &start, &end);
// 					mesh_vertices.push(start);
// 					mesh_vertices.push(end);


// 					// Show visited map
// 					let vmap = visited.map(|b| if b {"#"} else {"_"});
// 					for i in 0..vmap.len() {
// 						print!("{}", vmap[i]);
// 						if i % CHUNKSIZE == CHUNKSIZE-1 {
// 							println!("");
// 						}
// 					}
// 					println!("");
// 				}
// 			}

// 		}
// 	}

// 	println!("verts: {:?}", &mesh_vertices);

// 	// make actual quads how?

// }