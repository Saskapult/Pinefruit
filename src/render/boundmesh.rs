use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use crate::render::*;
use crate::mesh::*;
use generational_arena::{Arena, Index};




/// Vertex property data of a mesh
#[derive(Debug)]
pub struct BoundMesh {
	pub name: String,
	pub vertex_properties: Vec<VertexProperty>,
	// Todo: Combine vertex and index buffers, just slice to get individual
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub n_vertices: u32,	// Number of vertices (length of index buffer)
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
	
	format: Vec<VertexProperty>,
	meshes: Arena<BoundMesh>,
	index_map: HashMap<Index, Index>, // m -> bm
	i_index_map: HashMap<Index, Index>, // bm -> m
}
impl BoundMeshManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		Self {
			device: device.clone(), 
			queue: queue.clone(),
			format: Vec::new(),
			meshes: Arena::new(),
			index_map: HashMap::new(),
			i_index_map: HashMap::new(),
		}
	}

	/// True if stuff changed
	pub fn set_format(&mut self, format: &[VertexProperty]) -> bool {
		if self.format.as_slice() != format {
			self.format = Vec::from(format);
			todo!("Drop all meshes");
			// true
		} else {
			false
		}
	}

	pub fn index(&self, i: Index) -> Option<&BoundMesh> {
		self.meshes.get(i)
	}

	// It may be best to have a bind_multiple method which can get vertex data in parallel
	pub fn bind(&mut self, index: Index, meshes: &MeshManager) -> Index {
		let mesh = meshes.index(index).unwrap();

		info!("Binding mesh '{}' with properties '{:?}'", mesh, self.format);

		let vertices_bytes = mesh.vertex_data(&self.format).unwrap();

		let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} vertex buffer {:?}", &mesh.name, self.format)),
			contents: vertices_bytes.as_slice(),
			usage: wgpu::BufferUsages::VERTEX,
		});
		// Index doesn't depend on mesh format, this could be better
		let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} index buffer {:?}", &mesh.name, self.format)),
			contents: bytemuck::cast_slice(mesh.indices.as_ref().unwrap().as_slice()),
			usage: wgpu::BufferUsages::INDEX,
		});

		let idx = self.meshes.insert(BoundMesh {
			name: mesh.name.clone(), 
			vertex_properties: self.format.clone(), 
			vertex_buffer, 
			index_buffer, 
			n_vertices: mesh.indices.as_ref().unwrap().len() as u32,
		});
		self.index_map.insert(index, idx);
		self.i_index_map.insert(idx, index);
		idx
	}
}
