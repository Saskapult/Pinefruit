use wgpu::{self, util::DeviceExt};


#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tex_id: u32,
}
impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Texture coordinates
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Texture ID
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}


#[derive(Debug)]
pub struct Mesh {
	pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
}
impl Mesh {
	pub fn quad(device: &wgpu::Device) -> Self {
		let name = "quad".to_string();

		let num_elements = QUAD_INDICES.len() as u32;

		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} vertex buffer", &name)),
			contents: bytemuck::cast_slice(QUAD_VERTICES),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} index buffer", &name)),
			contents: bytemuck::cast_slice(QUAD_INDICES),
			usage: wgpu::BufferUsages::INDEX,
		});

		Self {
			name,
			vertex_buffer,
			index_buffer,
			num_elements,
		}
	}

	pub fn pentagon(device: &wgpu::Device) -> Self {
		let name = "pentagon".to_string();

		let num_elements = QUAD_INDICES.len() as u32;

		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} vertex buffer", &name)),
			contents: bytemuck::cast_slice(VERTICES),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{} index buffer", &name)),
			contents: bytemuck::cast_slice(INDICES),
			usage: wgpu::BufferUsages::INDEX,
		});

		Self {
			name,
			vertex_buffer,
			index_buffer,
			num_elements,
		}
	}
}




const VERTICES: &[Vertex] = &[
	Vertex { 
		position: [-0.0868241, 0.49240386, 0.0], 
		tex_coords: [0.4131759, 0.00759614], 
		normal: [0.0, 0.0, 0.0],
		tex_id: 0,
	},
	Vertex { 
		position: [-0.49513406, 0.06958647, 0.0], 
		tex_coords: [0.0048659444, 0.43041354], 
		normal: [0.0, 0.0, 0.0],
		tex_id: 0,
	}, 
	Vertex { 
		position: [-0.21918549, -0.44939706, 0.0], 
		tex_coords: [0.28081453, 0.949397], 
		normal: [0.0, 0.0, 0.0],
		tex_id: 0,
	},
	Vertex { 
		position: [0.35966998, -0.3473291, 0.0], 
		tex_coords: [0.85967, 0.84732914], 
		normal: [0.0, 0.0, 0.0],
		tex_id: 0,
	},
	Vertex { 
		position: [0.44147372, 0.2347359, 0.0], 
		tex_coords: [0.9414737, 0.2652641], 
		normal: [0.0, 0.0, 0.0],
		tex_id: 0,
	},
];

const INDICES: &[u16] = &[
	0, 1, 4, 
	1, 2, 4, 
	2, 3, 4, 
	/* padding */ 
	0,
];



const QUAD_VERTICES: &[Vertex] = &[
	Vertex { // Top left
		position: [-0.5, 0.5, 0.0], 
		tex_coords: [1.0, 1.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	},
	Vertex { // Bottom left
		position: [-0.5, -0.5, 0.0], 
		tex_coords: [1.0, 0.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	},
	Vertex { // Bottom right
		position: [0.5, -0.5, 0.0], 
		tex_coords: [0.0, 0.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	},
	Vertex { // Top right
		position: [0.5, 0.5, 0.0], 
		tex_coords: [0.0, 1.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	}, 
];

const QUAD_INDICES: &[u16] = &[
	0, 1, 2, 
	2, 3, 0,
];