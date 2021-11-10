use crate::{
	render::*,
	texturemanagers::BlockTexturesManager,
	geometry::*,
};
use nalgebra::*;



const CHUNKSIZE: usize = 32;
const CHUNKSIZE_SQUARED: usize = CHUNKSIZE * CHUNKSIZE;
const CHUNKSIZE_CUBED: usize = CHUNKSIZE * CHUNKSIZE * CHUNKSIZE;
const CHUNKSIZE_PLUS_ONE: usize = CHUNKSIZE + 1;
const CHUNKSIZE_PLUS_ONE_SQUARED: usize = CHUNKSIZE_PLUS_ONE * CHUNKSIZE_PLUS_ONE;

pub struct Map {
	pub chunks: Vec<Chunk>,
	pub chunk_render_pipeline: wgpu::RenderPipeline,
}
impl Map {
	pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, camera: &Camera, texture_manager: &BlockTexturesManager) -> Self {
		let chunk_render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Chunk Render Pipeline Layout"),
			bind_group_layouts: &[
				&texture_manager.texture_bind_group_layout,
				&camera.camera_bind_group_layout,
				],
			push_constant_ranges: &[],
		});

		let chunk_shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
			label: Some("Chunk Shader"),
			source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk_shader.wgsl").into()),
		});

		let chunk_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Chunk Render Pipeline"),
			layout: Some(&chunk_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &chunk_shader,
                entry_point: "vs_main",
                buffers: &[
					Vertex::desc(), 
				],
            },
            fragment: Some(wgpu::FragmentState {
                module: &chunk_shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLAMPING
                clamp_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

		let chunks = Vec::new();

		Self {
			chunks,
			chunk_render_pipeline,
		}
	}

	// Takes a world position, finds its chunk, and finds the block in that chunk
	pub fn block_at_pos(position: Vector3<f32>) {
		let chunk_pos = [
			(position[0] / (CHUNKSIZE as f32)).floor(),
			(position[1] / (CHUNKSIZE as f32)).floor(),
			(position[2] / (CHUNKSIZE as f32)).floor(),
		];
		let relative_block_pos = [
			(position[0] % (CHUNKSIZE as f32)),
			(position[1] % (CHUNKSIZE as f32)),
			(position[2] % (CHUNKSIZE as f32)),
		];
		// Check if chunk exists
	}
}




// Should be hashed in a z-order curve
pub struct Chunk {
	location: [i32; 3],			// Chunk location in chunk coordinates
	blocks: Vec<i32>,			// Len is CHUNKSIZE^3
	// Blocks to tick
	vertex_buf: wgpu::Buffer,
	index_buf: wgpu::Buffer,
	index_count: usize,
}
impl Chunk {
	
	fn meshme(&self) {
		let worldposition = [
			(self.location[0] * (CHUNKSIZE as i32)) as f32,
			(self.location[1] * (CHUNKSIZE as i32)) as f32,
			(self.location[2] * (CHUNKSIZE as i32)) as f32,
		];

		// let mesh_vertices = Vec::new();
		// let mesh_indices = Vec::new();
		// let mesh_texture_coordinates = Vec::new();

		for x in 0..CHUNKSIZE {
			for y in 0..CHUNKSIZE {
				for z in 0..CHUNKSIZE {
					
					let block_position = [
						worldposition[0] + (x as f32),
						worldposition[1] + (y as f32),
						worldposition[2] + (z as f32),
					];

					// Add block to mesh
				}
			}
		}

	}
}


struct Block {
	block_type: String,
}


struct BlockRun {
	block_type: String,
	length: u32,
}
struct ChunkMesher {
	runs: Vec<BlockRun>,
	sides: [[bool; CHUNKSIZE_SQUARED]; 6],
}


