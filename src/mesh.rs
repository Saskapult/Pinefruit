use std::{path::PathBuf, collections::HashMap};
use anyhow::*;
use rapier3d::prelude::*;
use crate::render::vertex::*;
// use thiserror::Error;




#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Mesh {
	pub name: String,
	pub positions: Option<Vec<[f32; 3]>>,	// Should not be optional
	pub uvs: Option<Vec<[f32; 2]>>,
	pub normals: Option<Vec<[f32; 3]>>,
	pub tangents: Option<Vec<[f32; 3]>>,
	pub bitangents: Option<Vec<[f32; 3]>>,
	pub colours: Option<Vec<[f32; 3]>>,
	pub indices: Option<Vec<u16>>,			// Should not be optional
	pub path: Option<PathBuf>,
	#[derivative(Debug="ignore")]
	pub collider_shape: Option<SharedShape>,
}
impl Mesh {
	pub fn new(name: &String) -> Self {
		Self {
			name: name.clone(),
			positions: None,
			uvs: None,
			normals: None,
			tangents: None,
			bitangents: None,
			colours: None,
			indices: None,
			path: None,
			collider_shape: None,
		}
	}

	pub fn with_positions(self, positions: Vec<[f32; 3]>) -> Self {
		Self {
			name: self.name,
			positions: Some(positions),
			uvs: self.uvs,
			normals: self.normals,
			tangents: self.tangents,
			bitangents: self.bitangents,
			colours: self.colours,
			indices: self.indices,
			path: self.path,
			collider_shape: self.collider_shape,
		}
	}

	pub fn with_uvs(self, uvs: Vec<[f32; 2]>) -> Self {
		Self {
			name: self.name,
			positions: self.positions,
			uvs: Some(uvs),
			normals: self.normals,
			tangents: self.tangents,
			bitangents: self.bitangents,
			colours: self.colours,
			indices: self.indices,
			path: self.path,
			collider_shape: self.collider_shape,
		}
	}

	pub fn with_normals(self, normals: Vec<[f32; 3]>) -> Self {
		Self {
			name: self.name,
			positions: self.positions,
			uvs: self.uvs,
			normals: Some(normals),
			tangents: self.tangents,
			bitangents: self.bitangents,
			colours: self.colours,
			indices: self.indices,
			path: self.path,
			collider_shape: self.collider_shape,
		}
	}

	pub fn with_indices(self, indices: Vec<u16>) -> Self {
		Self {
			name: self.name,
			positions: self.positions,
			uvs: self.uvs,
			normals: self.normals,
			tangents: self.tangents,
			bitangents: self.bitangents,
			colours: self.colours,
			indices: Some(indices),
			path: self.path,
			collider_shape: self.collider_shape,
		}
	}

	/// Gets data for this mesh with these properties.
	/// Throws and error if lacking requisite data.
	/// (Todo: actually implement that)
	pub fn vertex_data(&self, vertex_properties: &Vec<VertexProperty>) -> Result<Vec<u8>> {
		let mut vertices_bytes = Vec::new();
		for i in 0..self.positions.as_ref().unwrap().len() {
			for input in vertex_properties {
				match input {
					VertexProperty::VertexPosition => {
						vertices_bytes.extend_from_slice(bytemuck::bytes_of(&VertexPosition {
							position: self.positions.as_ref().unwrap()[i],
						}));
					},
					VertexProperty::VertexColour => {
						// Todo: don't simply fill with default value
						vertices_bytes.extend_from_slice(bytemuck::bytes_of(&VertexColour {
							colour: [0.0, 0.0, 0.0],
						}));
					},
					VertexProperty::VertexUV => {
						if let Some(uvs) = &self.uvs {
							vertices_bytes.extend_from_slice(bytemuck::bytes_of(&VertexUV {
								uv: uvs[i],
							}));
						} else {
							// Should make error but I don't want to
							vertices_bytes.extend_from_slice(bytemuck::bytes_of(&VertexUV {
								uv: [0.0, 0.0],
							}));
						}
					},
					_ => panic!("Weird vertex input or something idk"),
				}
			}
		}
		Ok(vertices_bytes)
	}

	pub fn quad() -> Self {
		todo!()
	}
	
	pub fn cross() -> Self {
		todo!()
	}

	pub fn from_obj_model(obj_model: tobj::Model) -> Result<Self> {
		
		let positions = match obj_model.mesh.positions.len() > 0 {
			true => {
				Some((0..obj_model.mesh.positions.len() / 3).map(|i| {
					[
						obj_model.mesh.positions[i * 3 + 0],
						obj_model.mesh.positions[i * 3 + 1],
						obj_model.mesh.positions[i * 3 + 2],
					]
				}).collect::<Vec<_>>())
			},
			false => None,
		};
		
		let normals = match obj_model.mesh.normals.len() > 0 {
			true => {
				Some((0..obj_model.mesh.normals.len() / 3).map(|i| {
					[
						obj_model.mesh.normals[i * 3 + 0],
						obj_model.mesh.normals[i * 3 + 1],
						obj_model.mesh.normals[i * 3 + 2],
					]
				}).collect::<Vec<_>>())
			},
			false => None,
		};

		let uvs = match obj_model.mesh.texcoords.len() > 0 {
			true => {
				Some((0..obj_model.mesh.texcoords.len() / 3).map(|i| {
					[
						obj_model.mesh.texcoords[i * 2 + 0],
						obj_model.mesh.texcoords[i * 2 + 1],
					]
				}).collect::<Vec<_>>())
			},
			false => None,
		};

		let indices = obj_model.mesh.indices.iter().cloned().map(|v| v as u16).collect::<Vec<_>>();
		
		Ok(Self {
			name: obj_model.name,
			positions,
			uvs,
			normals,
			tangents: None,
			bitangents: None,
			colours: None,
			indices: Some(indices),
			path: None,
			collider_shape: None,
		})
	}

	pub fn make_trimesh(&mut self) -> Result<()> {
		self.collider_shape = Some(mesh_trimesh(&self)?);
		Ok(())
	}
}
impl std::fmt::Display for Mesh {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		//Todo: Mesh {} [positions, normals, indices]
		write!(f, "Mesh {}", &self.name)
	}
}



#[derive(Debug)]
pub struct MeshManager {
	meshes: Vec<Mesh>,
	index_name: HashMap<String, usize>,
	index_path: HashMap<PathBuf, usize>,
}
impl MeshManager {
	pub fn new() -> Self {
		Self {
			meshes: Vec::new(), 
			index_name: HashMap::new(), 
			index_path: HashMap::new(),
		}
	}
	
	pub fn insert(&mut self, mesh: Mesh) -> usize {
		let idx = self.meshes.len();
		self.index_name.insert(mesh.name.clone(), idx);
		if let Some(path) = mesh.path.clone() {
			self.index_path.insert(path, idx);
		}
		self.meshes.push(mesh);
		idx
	}

	pub fn index(&self, i: usize) -> &Mesh {
		&self.meshes[i]
	}

	pub fn index_name(&self, name: &String) -> Option<usize> {
		if self.index_name.contains_key(name) {
			Some(self.index_name[name])
		} else {
			None
		}
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<usize> {
		if self.index_path.contains_key(path) {
			Some(self.index_path[path])
		} else {
			None
		}
	}
}



// #[derive(Error, Debug)]
// pub enum MeshError {
// 	#[error("Mesh has a number of indices which is not divisible by three")]
// 	NonTriMeshError,
// 	#[error("Mesh has an index with no corresponding data value")]
// 	IndexBoundsError,
// 	#[error("Mesh is missing data required for compilation")]
// 	LacksPropertyError,
// }



pub fn mesh_trimesh(mesh: &Mesh) -> Result<SharedShape> {
	use nalgebra::Point3;

	if mesh.indices.as_ref().unwrap().len() % 3 != 0 {
		warn!("This mesh ('{}') has a number of indices not divisible by three", &mesh.name);
	}

	let shape = SharedShape::trimesh(
		mesh.positions.as_ref().unwrap().iter().map(|pos| Point3::new(pos[0], pos[1], pos[2])).collect::<Vec<_>>(),
		mesh.indices.as_ref().unwrap().chunks_exact(3).map(|i| [i[0] as u32, i[1] as u32, i[2] as u32]).collect::<Vec<_>>(),
	);

	Ok(shape)
}



/// Create a trimesh from many meshes
pub fn meshes_trimesh(
	meshes: Vec<&Mesh>
) -> Result<SharedShape> {
	use nalgebra::Point3;

	let mut vertices = Vec::new();
	let mut indices = Vec::new();
	for mesh in meshes {
		if mesh.indices.as_ref().unwrap().len() % 3 != 0 {
			// return Err(MeshError::NonTriMeshError)
			warn!("This mesh ('{}') has a number of indices not divisible by three", &mesh.name);
		}
		vertices.extend(mesh.positions.as_ref().unwrap().iter().map(|pos| Point3::new(pos[0], pos[1], pos[2])));
		indices.extend(mesh.indices.as_ref().unwrap().chunks_exact(3).map(|i| [i[0] as u32, i[1] as u32, i[2] as u32]));
	}
	
	Ok(SharedShape::trimesh(
		vertices,
		indices,
	))
}
