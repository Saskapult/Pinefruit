use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use crate::render::*;
use std::sync::RwLock;
use crate::mesh::*;
use generational_arena::{Arena, Index};



/*
In the future we could let a bound mesh specify whether its indices should use wgpu::IndexFormat::Uint16 or wgpu::IndexFormat::Uint32 based on the length of the mesh's indices vec but 2^16 is fairly big so I don't really care right now

Oh also please combine vertices and indices into one buffer and just slice it 
*/



pub type MeshInputFormat = (VertexProperties, InstanceProperties);



/// Vertex property data of a mesh
#[derive(Debug)]
pub struct BoundMesh {
	pub name: String,
	pub mesh_idx: Index,
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
	meshes: Arena<BoundMesh>, // These should be stored in an arena
	meshes_index_from_name_properties: HashMap<(String, Vec<VertexProperty>), Index>,
	meshes_index_from_index_properties: HashMap<(Index, Vec<VertexProperty>), Index>,
	pub data_manager: Arc<RwLock<MeshManager>>,
	// Add a channel or something for unloaded data mesh indices to prevent garbage from building up in meshes_index_from_index_properties
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
			meshes: Arena::new(),
			meshes_index_from_name_properties: HashMap::new(),
			meshes_index_from_index_properties: HashMap::new(),
			data_manager: mesh_manager.clone(),
		}
	}

	pub fn index(&self, i: Index) -> Option<&BoundMesh> {
		self.meshes.get(i)
	}

	pub fn insert(&mut self, bound_mesh: BoundMesh) -> Index {
		let index_key = (bound_mesh.mesh_idx, bound_mesh.vertex_properties.clone());
		let name_key = (bound_mesh.name.clone(), bound_mesh.vertex_properties.clone());
		let idx = self.meshes.insert(bound_mesh);
		self.meshes_index_from_index_properties.insert(index_key, idx);
		self.meshes_index_from_name_properties.insert(name_key, idx);
		idx
	}

	pub fn bind_by_index(&mut self, mesh_idx: Index, vertex_properties: &Vec<VertexProperty>) -> Index {
		let mm = self.data_manager.read().unwrap();
		let mesh = mm.index(mesh_idx).unwrap();

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

	pub fn index_from_name_properites(&self, name: &String, vertex_properties: &Vec<VertexProperty>) -> Option<Index> {
		let key = (name.clone(), vertex_properties.clone());
		if self.meshes_index_from_name_properties.contains_key(&key) {
			Some(self.meshes_index_from_name_properties[&key])
		} else {
			None
		}
	}

	pub fn index_from_name_properites_bind(&mut self, name: &String, vertex_properties: &Vec<VertexProperty>) -> Index {
		let mm = self.data_manager.read().unwrap();
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

	pub fn index_from_index_properites(&self, i: Index, vertex_properties: &Vec<VertexProperty>) -> Option<Index> {
		let key = (i, vertex_properties.clone());
		if self.meshes_index_from_index_properties.contains_key(&key) {
			Some(self.meshes_index_from_index_properties[&key])
		} else {
			None
		}
	}

	pub fn index_from_index_properites_bind(&mut self, mesh_idx: Index, vertex_properties: &Vec<VertexProperty>) -> Index {
		let key = (mesh_idx, vertex_properties.clone());
		if self.meshes_index_from_index_properties.contains_key(&key) {
			self.meshes_index_from_index_properties[&key]
		} else {
			self.bind_by_index(mesh_idx, vertex_properties)
		}
	}
}
