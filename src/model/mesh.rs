use wgpu::util::DeviceExt;
use crate::model::vertex::Vertex;



// A mesh is a series of vertices with index optimizations, it also has a material
#[derive(Debug)]
pub struct Mesh {
	pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
	pub material: usize,    // The material index to pull from
}
impl Mesh {
    pub fn new(
        name: &String, 
        m: tobj::Model,
		device: &wgpu::Device,
    ) -> Self {
        let mut vertices = Vec::new();
		for i in 0..m.mesh.positions.len() / 3 {
			vertices.push(Vertex {
				position: [
					m.mesh.positions[i * 3],
					m.mesh.positions[i * 3 + 1],
					m.mesh.positions[i * 3 + 2],
				],
				colour: [1.0, 1.0, 1.0],
				tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
				normal: [
					m.mesh.normals[i * 3],
					m.mesh.normals[i * 3 + 1],
					m.mesh.normals[i * 3 + 2],
				],
			});
		}

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{:?} Vertex Buffer", name)),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		}
		);
		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{:?} Index Buffer", name)),
				contents: bytemuck::cast_slice(&m.mesh.indices),
				usage: wgpu::BufferUsages::INDEX,
			}
		);

        let num_elements = m.mesh.indices.len() as u32;
        let material = m.mesh.material_id.unwrap_or(0);

        Self {
            name: name.to_string(),
            vertex_buffer,
            index_buffer,
            num_elements,
            material,
        }

    }
}

