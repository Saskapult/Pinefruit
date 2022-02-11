use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use crate::render::*;
use std::sync::RwLock;
use anyhow::*;


/*
In the future we could let a bound mesh specify whether its indices should use wgpu::IndexFormat::Uint16 or wgpu::IndexFormat::Uint32 based on the length of the mesh's indices vec but 2^16 is fairly big so I don't really care right now
*/



pub type MeshInputFormat = (VertexProperties, InstanceProperties);



#[derive(Debug, Clone)]
pub struct Mesh {
	pub name: String,
	pub positions: Option<Vec<[f32; 3]>>,
	pub uvs: Option<Vec<[f32; 2]>>,
	pub normals: Option<Vec<[f32; 3]>>,
	pub tangents: Option<Vec<[f32; 3]>>,
	pub bitangents: Option<Vec<[f32; 3]>>,
	pub colours: Option<Vec<[f32; 3]>>,
	pub indices: Option<Vec<u16>>,
	pub path: Option<PathBuf>,
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
		})
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



/// Vertex property data of a mesh
#[derive(Debug)]
pub struct BoundMesh {
	pub name: String,
	pub mesh_idx: usize,
	pub vertex_properties: Vec<VertexProperty>,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub n_vertices: u32,	// Number of vertices (length of index buffer)
	// pub n_vertices_unique: u32	// Number of unique vertices (length of vertex buffer)
}
impl std::fmt::Display for BoundMesh {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "BoundMesh {} [", &self.name)?;
		if self.vertex_properties.len() != 0 {
			if self.vertex_properties.len() > 1 {
				for bsi in 0..(self.vertex_properties.len()-1) {
					write!(f, "{:?}, ", self.vertex_properties[bsi])?;
				}
			}
			write!(f, "{:?}", self.vertex_properties[self.vertex_properties.len()-1])?
		}
		write!(f, "]")
	}
}



#[derive(Debug)]
pub struct BoundMeshManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	meshes: Vec<BoundMesh>, // These should be stored in an arena
	meshes_index_from_name_properties: HashMap<(String, Vec<VertexProperty>), usize>,
	meshes_index_from_index_properties: HashMap<(usize, Vec<VertexProperty>), usize>,
	mesh_manager: Arc<RwLock<MeshManager>>,
}
impl BoundMeshManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		mesh_manager: &Arc<RwLock<MeshManager>>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(),
			meshes: Vec::new(),
			meshes_index_from_name_properties: HashMap::new(),
			meshes_index_from_index_properties: HashMap::new(),
			mesh_manager: mesh_manager.clone(),
		}
	}

	pub fn index(&self, i: usize) -> &BoundMesh {
		&self.meshes[i]
	}

	pub fn insert(&mut self, bound_mesh: BoundMesh) -> usize {
		let idx = self.meshes.len();
		let index_key = (bound_mesh.mesh_idx, bound_mesh.vertex_properties.clone());
		self.meshes_index_from_index_properties.insert(index_key, idx);
		let name_key = (bound_mesh.name.clone(), bound_mesh.vertex_properties.clone());
		self.meshes_index_from_name_properties.insert(name_key, idx);
		self.meshes.push(bound_mesh);
		idx
	}

	pub fn bind_by_index(&mut self, mesh_idx: usize, vertex_properties: &Vec<VertexProperty>) -> usize {
		let mm = self.mesh_manager.read().unwrap();
		let mesh = mm.index(mesh_idx);

		info!("Binding mesh '{}' with properties '{:?}'", mesh, vertex_properties);

		let vertices_bytes = mesh.vertex_data(vertex_properties).unwrap();

		let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} vertex buffer {:?}", &mesh.name, vertex_properties)),
			contents: vertices_bytes.as_slice(),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} index buffer {:?}", &mesh.name, vertex_properties)),
			contents: bytemuck::cast_slice(mesh.indices.as_ref().unwrap().as_slice()),
			usage: wgpu::BufferUsages::INDEX,
		});

		let bound_mesh = BoundMesh {
			name: mesh.name.clone(), 
			mesh_idx,
			vertex_properties: vertex_properties.clone(), 
			vertex_buffer, 
			index_buffer, 
			n_vertices: mesh.indices.as_ref().unwrap().len() as u32,
		};
		
		drop(mm);
		self.insert(bound_mesh)
	}

	pub fn index_from_name_properites(&self, name: &String, vertex_properties: &Vec<VertexProperty>) -> Option<usize> {
		let key = (name.clone(), vertex_properties.clone());
		if self.meshes_index_from_name_properties.contains_key(&key) {
			Some(self.meshes_index_from_name_properties[&key])
		} else {
			None
		}
	}

	pub fn index_from_name_properites_bind(&mut self, name: &String, vertex_properties: &Vec<VertexProperty>) -> usize {
		let mm = self.mesh_manager.read().unwrap();
		let key = (name.clone(), vertex_properties.clone());
		if self.meshes_index_from_name_properties.contains_key(&key) {
			self.meshes_index_from_name_properties[&key]
		} else if let Some(mesh_idx) = mm.index_name(name) {
			drop(mm);
			self.bind_by_index(mesh_idx, vertex_properties)
		} else {
			panic!("Tried to access a nonexistent mesh!")
		}
	}

	pub fn index_from_index_properites(&self, i: usize, vertex_properties: &Vec<VertexProperty>) -> Option<usize> {
		let key = (i, vertex_properties.clone());
		if self.meshes_index_from_index_properties.contains_key(&key) {
			Some(self.meshes_index_from_index_properties[&key])
		} else {
			None
		}
	}

	pub fn index_from_index_properites_bind(&mut self, mesh_idx: usize, vertex_properties: &Vec<VertexProperty>) -> usize {
		let key = (mesh_idx, vertex_properties.clone());
		if self.meshes_index_from_index_properties.contains_key(&key) {
			self.meshes_index_from_index_properties[&key]
		} else {
			self.bind_by_index(mesh_idx, vertex_properties)
		}
	}
}
