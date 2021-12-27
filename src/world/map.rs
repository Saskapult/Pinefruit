use nalgebra::*;
use std::collections::HashMap;
use crate::world::*;




#[derive(Debug)]
pub struct Map {
	chunks: HashMap<[i32; 3], Chunk>,
	chunk_dimensions: [u32; 3]
}
impl Map {
	pub fn new(chunk_dimensions: [u32; 3]) -> Self {
		let chunks = HashMap::new();
		Self { chunks, chunk_dimensions, }
	}

	pub fn is_chunk_loaded(&self, position: &[i32; 3]) -> bool {
		self.chunks.contains_key(position)
	}

	// Mesh a chunk with respect to those around it
	// This will look bad if seen from an side without a chunk before it
	pub fn mesh_chunk(&self, position: &[i32; 3]) -> usize {
		let [px, py, pz] = *position;
		
		let main_chunk = self.get_chunk(position).expect("Tried to mesh unloaded chunk!");
		
		// Copy slices of neighbours
		// If the neighbour is not loaded, pretend there is nothing there (this is bad)
		let xp_slice = match self.get_chunk(&[px+1, py, pz]) {
			Some(chunk) => {
				let mut xp = Vec::with_capacity((self.chunk_dimensions[1] * self.chunk_dimensions[2]) as usize);
				for y in 0..self.chunk_dimensions[1] {
					let y_offset = y * self.chunk_dimensions[1];
					for z in 0..self.chunk_dimensions[2] {
						xp[(y_offset + z) as usize] = chunk.get_voxel([0, y as i32, z as i32]);
					}
				}
				xp
			},
			None => vec![Voxel::Empty; (self.chunk_dimensions[1] * self.chunk_dimensions[2]) as usize],
		};
		
		let yp_slice = vec![Voxel::Empty; (self.chunk_dimensions[0] * self.chunk_dimensions[2]) as usize];
		let zp_slice = vec![Voxel::Empty; (self.chunk_dimensions[0] * self.chunk_dimensions[1]) as usize];

		let slices = [&xp_slice, &yp_slice, &zp_slice];

		let (parts, _) = map_mesh(&main_chunk.contents, &slices);

		// Return Vec<MeshData>?
		todo!()
	}


	pub fn positions_around(&self, centre: [i32; 3], radius: i32) -> Vec<[i32; 3]> {
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


	// Gets the coordinates of the chunk in which this point resides
	pub fn chunk_of(&self, position: Vector3<f32>) -> [i32; 3] {
		let chunk_pos = [
			(position[0] / (self.chunk_dimensions[0] as f32)).floor() as i32,
			(position[1] / (self.chunk_dimensions[1] as f32)).floor() as i32,
			(position[2] / (self.chunk_dimensions[2] as f32)).floor() as i32,
		];
		chunk_pos
	}
}



// Never generate faces for negativemost blocks, they are covered by their chunks
// If not collect_transparent then don't group faces with a transparent material together, allowing them to be drawn individually (could we use instancing for this?)
struct MapMeshOutput {
	face_collections: Vec<(usize, Vec<Vertex>, Vec<u16>)>, // Vec<(material id, Vec<vertex>, Vec<index>)>
	models: Vec<(usize, Instance)>, // Vec<(model id, instance)>
}
fn map_mesh(
	chunk_contents: &Vec<Voxel>,
	chunk_size: [usize; 3],
	neighbour_slices: &[Vec<Voxel>; 3], // xp, yp, zp
	blockmap: &BlockManager,
	collect_transparent: bool,
) -> MapMeshOutput {

	let mut faces = HashMap::new();

	let mut mesh_vertices = Vec::new();
	let mut mesh_indices = Vec::new();

	let [x_size, y_size, z_size] = chunk.size;
	let x_multiplier = y_size * z_size;
	let y_multiplier = z_size;
	let z_multiplier = 1;

	for (direction, direction_vector) in &[(Direction::Xp, [1, 0, 0]), (Direction::Yp, [0, 1, 0]), (Direction::Zp, [0, 0, 1]),] {
		// println!("Meshing {:?}", direction);

		for x in 0..x_size {
			let x_offset = x * x_multiplier;
			// Could we create an "a" slice and a "b" slice?
			// When ending iteration "b" becomes "a" and we only need to read the new "b"
			for y in 0..y_size {
				let y_offset = y * y_multiplier;
				for z in 0..z_size {

					// Get 'a' and 'b' blocks to compare
					let a = contents[x_offset + y_offset + z];
					let bx = x+direction_vector[0];
					let by = y+direction_vector[1];
					let bz = z+direction_vector[2];
					let b = {
						// These *should* already be cache-optimized, so don't worry about that
						// Todo: re-order so we don't do 3 comparisons in regular case
						if bx == x_size {
							neighbour_slices[0][by*x_size + bz]
						} else if by == y_size {
							neighbour_slices[1][bx*y_size + bz]
						} else if bz == z_size {
							neighbour_slices[2][bx*z_size + by]
						} else {
							contents[bx*x_multiplier + by*y_multiplier + bz]
						}
					};

					// Are they transparent? (this is crude make it better)
					// Todo: Check if specific face is opaque
					let mut a_index = 0;
					let at = match a {Voxel::Empty => true, Voxel::Block(idx) => {a_index = idx; false}};
					let mut b_index = 0;
					let bt = match b {Voxel::Empty => true, Voxel::Block(idx) => {b_index = idx; false}};

					// a opaque b transparent
					// Make positive face for a
					if !at && bt {
						let a_material_id = blockmap.get_index(a_index).material_id;

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

	todo!()
}