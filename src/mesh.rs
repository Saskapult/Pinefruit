use std::{path::PathBuf, collections::HashMap};
use anyhow::*;
use rapier3d::prelude::*;
use crate::render::vertex::*;
// use thiserror::Error;
use generational_arena::{Arena, Index};




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
	pub collider_trimesh: Option<SharedShape>,
	#[derivative(Debug="ignore")]
	pub collider_convexhull: Option<SharedShape>,
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
			collider_trimesh: None,
			collider_convexhull: None,
		}
	}

	pub fn with_positions(self, positions: Vec<[f32; 3]>) -> Self {
		Self {
			positions: Some(positions),
			..self
		}
	}

	pub fn with_uvs(self, uvs: Vec<[f32; 2]>) -> Self {
		Self {
			uvs: Some(uvs),
			..self
		}
	}

	pub fn with_normals(self, normals: Vec<[f32; 3]>) -> Self {
		Self {
			normals: Some(normals),
			..self
		}
	}

	pub fn with_indices(self, indices: Vec<u16>) -> Self {
		Self {
			indices: Some(indices),
			..self
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
				Some((0..obj_model.mesh.texcoords.len() / 2).map(|i| {
					[
						obj_model.mesh.texcoords[i * 2 + 0],
						obj_model.mesh.texcoords[i * 2 + 1],
					]
				}).collect::<Vec<_>>())
			},
			false => None,
		};

		let indices = obj_model.mesh.indices.chunks_exact(3).map(|v| {
			[v[2] as u16, v[1] as u16, v[0] as u16]
		}).collect::<Vec<_>>().concat();
		
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
			collider_trimesh: None,
			collider_convexhull: None,
		})
	}

	pub fn make_trimesh(&self) -> Result<SharedShape> {
		let (vertices, indices) = mesh_rapier_convert(&self)?;
		Ok(SharedShape::trimesh(vertices, indices))
	}

	pub fn make_convexhull(&self) -> Result<SharedShape> {
		let (vertices, _indices) = mesh_rapier_convert(&self)?;
		Ok(SharedShape::convex_hull(vertices.as_slice()).unwrap())
	}
}
impl std::fmt::Display for Mesh {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		//Todo: Mesh {} [positions, normals, indices]
		write!(f, "Mesh {}", &self.name)
	}
}



#[derive(Debug, Default)]
pub struct MeshManager {
	meshes: Arena<Mesh>,
	index_name: HashMap<String, Index>,
	index_path: HashMap<PathBuf, Index>,
}
impl MeshManager {
	pub fn new() -> Self {
		Self {
			meshes: Arena::new(), 
			index_name: HashMap::new(), 
			index_path: HashMap::new(),
		}
	}
	
	pub fn insert(&mut self, mesh: Mesh) -> Index {
		let name = mesh.name.clone();
		let path = mesh.path.clone();
		let idx = self.meshes.insert(mesh);
		self.index_name.insert(name, idx);
		if let Some(path) = path {
			self.index_path.insert(path, idx);
		}
		idx
	}

	pub fn remove(&mut self, i: Index) -> Option<Mesh> {
		let m = self.meshes.remove(i);
		if let Some(m) = m.as_ref() {
			if let Some(p) = m.path.as_ref() {
				self.index_path.remove(p);
			}
			self.index_name.remove(&m.name);
		}
		m
	}

	pub fn index(&self, i: Index) -> Option<&Mesh> {
		self.meshes.get(i)
	}

	pub fn index_name(&self, name: &String) -> Option<Index> {
		if self.index_name.contains_key(name) {
			Some(self.index_name[name])
		} else {
			None
		}
	}

	pub fn index_path(&self, path: &PathBuf) -> Option<Index> {
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



fn mesh_rapier_convert(mesh: &Mesh) -> Result<(Vec<Point<Real>>, Vec<[u32; 3]>)> {
	use nalgebra::Point3;
	if mesh.indices.as_ref().unwrap().len() % 3 != 0 {
		warn!("This mesh ('{}') has a number of indices not divisible by three", &mesh.name);
	}
	let vertices = mesh.positions.as_ref().unwrap().iter().map(|pos| Point3::new(pos[0], pos[1], pos[2])).collect::<Vec<_>>();
	let indices = mesh.indices.as_ref().unwrap().chunks_exact(3).map(|i| [i[0] as u32, i[1] as u32, i[2] as u32]).collect::<Vec<_>>();

	Ok((vertices, indices))
}



/// Create a trimesh from multiple meshes
pub fn meshes_trimesh(
	meshes: Vec<&Mesh>
) -> Result<SharedShape> {
	let mut vertices = Vec::new();
	let mut indices = Vec::new();
	for mesh in meshes {
		let (mesh_vertices, mut mesh_indices) = mesh_rapier_convert(mesh)?;
		let vl = vertices.len() as u32;
		vertices.extend(mesh_vertices);
		indices.extend(mesh_indices.drain(..).map(|[i, j ,k]| [i + vl, j + vl, k + vl]));
	}
	
	Ok(SharedShape::trimesh(
		vertices,
		indices,
	))
}
