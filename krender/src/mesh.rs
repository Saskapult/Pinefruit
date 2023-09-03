//! 
//! Currently when a mesh is indexed it is indexed using u16s.
//! In the future there should be an option to use u32.
//! This brings into question where we should store the indices.
//! Maybe in data as "indices_u32" or something?
//! Should we store both until there are too many for u16 to index?
//! So confusing, I don't care for now.
//! Indexing will be disabled until you make a decision.
//! 
//! ¯\\_(ツ)_/¯
//! 
//! Oh also where should we put the indices?
//! In data? Idk what to call it.
//! 

use std::{collections::{HashMap, HashSet}, path::{PathBuf, Path}};
use bytemuck::{Pod, Zeroable};
use slotmap::{SecondaryMap, SlotMap};
use wgpu::util::DeviceExt;
use crate::{MeshKey, MeshFormatKey, vertex::VertexAttribute};



#[derive(Debug, Default)]
pub struct MeshBindings {
	pub index_buffer: Option<(wgpu::Buffer, wgpu::IndexFormat)>, // u16 indices
	pub vertex_buffers: SecondaryMap<MeshFormatKey, wgpu::Buffer>,
}


// Lazy loading? Hot reloading/updating? Who needs that! Can offload though
#[derive(Debug)]
pub struct Mesh {
	pub name: String,
	pub path: Option<PathBuf>,
	pub data: HashMap<String, Vec<u8>>,
	pub n_vertices: u32,
	pub indices: Option<Vec<u32>>,

	pub bindings: Option<MeshBindings>,
	pub pending_binds: HashSet<MeshFormatKey>,
}
impl Mesh {
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			path: None,
			data: HashMap::new(),
			n_vertices: 0,
			indices: None,
			bindings: None,
			pending_binds: HashSet::new(),
		}
	}

	fn vertex_data(
		&self, 
		vertex_attributes: &Vec<VertexAttribute>,
	) -> anyhow::Result<Vec<u8>> {
		// Find data sources
		let data = vertex_attributes.iter().map(|va| {
			if let Some(vas) = self.data.get(&va.source) {
				assert_ne!(0, vas.len());
				// trace!("{} bytes of {}", vas.len(), va.name);
				Some(vas)
			} else if let Some(d) = va.default.as_ref() {
				warn!("Mesh '{}' has no vertex data for '{}', using default", self.name, va.name);
				Some(d)
			} else {
				panic!("Can't get vertex data for '{}' (not provided and no default), available feilds are {:?}", va.name, self.data.keys());
			}
		}).zip(vertex_attributes.iter()).collect::<Vec<_>>();

		// Allocate vector to hold mesh data
		let total_size = vertex_attributes.iter().fold(0, |acc, va| acc + va.size());
		let mut vertices_bytes = Vec::with_capacity(self.n_vertices as usize * total_size as usize);

		for vertex_index in 0..self.n_vertices {
			for (data, va) in data.iter() {
				if let Some(data) = data {
					let st = vertex_index as usize * va.size() as usize;
					let en = (vertex_index+1) as usize * va.size() as usize;
					vertices_bytes.extend_from_slice(&data[st..en]);
				} else {
					vertices_bytes.extend_from_slice(va.default.as_ref().unwrap().as_slice());
				}
			}
		}
		
		Ok(vertices_bytes)
	}

	/// Inserts a data field (~~indices~~, positions, normals, etc).
	pub fn with_data<T: Pod + Zeroable>(mut self, name: impl Into<String>, data: &[T]) -> Self {
		self.data.insert(
			name.into(), 
			bytemuck::cast_slice(data).to_vec(),
		);
		self
	}

	pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
		self.indices = Some(indices);
		self
	}

	// This should probably be in the `new` fucntion
	pub fn with_vertex_count(mut self, count: u32) -> Self {
		self.n_vertices = count;
		self
	}

	/// Asserts that a mesh format is loaded, otherwise queues it.
	/// Doesn't handle duplicate requests but I'm not convinced that will be an issue.
	pub fn bind(
		&mut self, 
		index: MeshFormatKey,
	) {
		debug!("Binding (pending) mesh '{}'", self.name);
		if let Some(g) = self.bindings.as_ref() {
			if g.vertex_buffers.contains_key(index) {
				warn!("Hey that's already bound!");
				return
			}
		}
		self.pending_binds.insert(index);
	}

	/// Binds meshes that have been requested. 
	/// To be called after a bunch of stuff idk. 
	/// Return indicates if anything changed or not. 
	pub(self) fn bind_unbound(
		&mut self,
		device: &wgpu::Device,
		formats: &MeshFormatManager,
	) -> bool {
		if self.pending_binds.is_empty() {
			debug!("Mesh '{}' has nothing to bind", self.name);
			return false;
		}
		debug!("Mesh '{}' binds unbound formats", self.name);
		self.bindings.get_or_insert_with(|| {
			trace!("Init mesh bindings");
			let mut b = MeshBindings::default();
			if let Some(i) = self.indices.as_ref() {
				trace!("hey this one has index data");
				b.index_buffer = Some((device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&*format!("{} index buffer", self.name)),
					contents: bytemuck::cast_slice(i.as_slice()),
					usage: wgpu::BufferUsages::INDEX,
				}), wgpu::IndexFormat::Uint32));
			}
			b
		});

		for index in self.pending_binds.iter().copied() {
			let vertex_attributes = formats.vertex_formats.get(index).unwrap();
			let vertex_attribute_names = vertex_attributes.iter().map(|va| va.name.clone()).collect::<Vec<_>>();
			debug!("Binding mesh '{}' with properties {vertex_attribute_names:?}", self.name);
			let vertex_data = self.vertex_data(&vertex_attributes).unwrap();
			self.bindings.as_mut().unwrap().vertex_buffers.insert(
				index,
				device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&*format!("{} vertex buffer with properties {vertex_attribute_names:?}", self.name)),
					contents: vertex_data.as_slice(),
					usage: wgpu::BufferUsages::VERTEX,
				}),
			);
		}
		self.pending_binds.clear();

		true
	}

	pub fn read_obj(path: impl AsRef<Path>) -> Self {
		let load_options = tobj::LoadOptions {
			triangulate: true,
			single_index: true,
			..Default::default()
		};
		let (models, _materials) = tobj::load_obj(path.as_ref(), &load_options).unwrap();
		assert_eq!(1, models.len(), "only one model per obj file please");

		let mut s = Self::from_obj_model(&models[0]);
		s.path = Some(path.as_ref().into());

		s
	}

	pub fn from_obj_model(obj_model: &tobj::Model) -> Self {
		let obj_mesh = &obj_model.mesh;
		let mut mesh = Self::new(&obj_model.name);

		if obj_mesh.positions.len() > 0 {
			mesh = mesh.with_data("positions", obj_mesh.positions.as_slice());
			mesh.n_vertices = (obj_mesh.positions.len() / 3) as u32;
		} else {
			todo!("Figure out what to do when positions are not given");
		}
		if obj_model.mesh.normals.len() > 0 {
			mesh = mesh.with_data("normals", obj_mesh.normals.as_slice());
		}	
		if obj_model.mesh.texcoords.len() > 0 {
			mesh = mesh.with_data("uvs", obj_mesh.texcoords.as_slice());
		}
		if obj_model.mesh.indices.len() > 0 {
			// This will be u32 always
			// mesh = mesh.with_data("indices", obj_mesh.indices.as_slice());
			// Overwrites the count from positions
			mesh.indices = Some(obj_mesh.indices.clone());
		}

		mesh
	}
}


#[derive(Debug, Default)]
struct MeshFormatManager {
	// Each mesh contains an arena of bound meshes, each will be found at its corresponding index
	pub vertex_formats: SlotMap<MeshFormatKey, Vec<VertexAttribute>>,
	// Maps names! Do not map the actual data
	pub vertex_format_indices: HashMap<Vec<String>, MeshFormatKey>,
}
impl MeshFormatManager {
	pub fn new() -> Self {
		Self {
			vertex_formats: SlotMap::with_key(),
			vertex_format_indices: HashMap::new(),
		}
	}

	pub fn format_new_or_create(&mut self, attributes: &Vec<VertexAttribute>) -> MeshFormatKey {
		let names = attributes.iter().map(|a| a.name.clone()).collect::<Vec<_>>();
		if let Some(&k) = self.vertex_format_indices.get(&names) {
			debug!("Found mesh index for attributes {:?}", attributes);
			k
		} else {
			debug!("New mesh index for attributes {:?}", attributes);
			let k = self.vertex_formats.insert(attributes.clone());
			self.vertex_format_indices.insert(names, k);
			k
		}
	}
}


#[derive(Debug, Default)]
pub struct MeshManager {
	meshes: SlotMap<MeshKey, Mesh>,
	key_by_name: HashMap<String, MeshKey>,
	key_by_path: HashMap<PathBuf, MeshKey>,
	formats: MeshFormatManager,
}
impl MeshManager {
	pub fn new() -> Self {
		Self {
			meshes: SlotMap::with_key(), 
			key_by_name: HashMap::new(), 
			key_by_path: HashMap::new(), 
			formats: MeshFormatManager::new(),
		}
	}

	pub fn read_or(&mut self, path: impl Into<PathBuf>, f: fn() -> Mesh) -> MeshKey {
		let path = path.into().canonicalize().unwrap();
		if let Some(key) = self.key_by_path.get(&path).cloned() {
			return key;
		} else {
			self.insert(f())
		}
	}
	
	pub fn insert(&mut self, mesh: Mesh) -> MeshKey {
		let name = mesh.name.clone();
		let idx = self.meshes.insert(mesh);
		self.key_by_name.insert(name, idx);
		idx
	}

	pub fn remove(&mut self, key: MeshKey) -> Option<Mesh> {
		let m = self.meshes.remove(key);
		if let Some(m) = m.as_ref() {
			self.key_by_name.remove(&m.name);
		}
		m
	}

	pub fn get(&self, key: MeshKey) -> Option<&Mesh> {
		self.meshes.get(key)
	}

	pub fn get_mut(&mut self, key: MeshKey) -> Option<&mut Mesh> {
		self.meshes.get_mut(key)
	}

	pub fn format_new_or_create(&mut self, attributes: &Vec<VertexAttribute>) -> MeshFormatKey {
		self.formats.format_new_or_create(attributes)
	}

	pub fn bind_unbound(&mut self, device: &wgpu::Device) -> bool {
		// self.meshes.iter_mut().fold(false, |a, (_, m)| {
		// 	a || m.bind_unbound(device, &self.formats)
		// })
		for mesh in self.meshes.values_mut() {
			mesh.bind_unbound(device, &self.formats);
		}
		true
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
