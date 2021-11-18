
use crate::{
	geometry::*,
	texture::Texture,
	render::*,
	render::Instance,
	world::Map,
	texturemanagers::BlockTexturesManager,
};
use winit::{
	event::*,
	window::Window,
};
use nalgebra::*;
use wgpu::util::DeviceExt;
use wgpu::*;


pub struct RenderData {
	pub mesh_id: u32,
	pub texture_id: u32,
	pub pipeline_id: u32,
	pub instance: Instance,
}




// The renderer represents a ???
pub struct Renderer {
	pub surface: wgpu::Surface,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub config: wgpu::SurfaceConfiguration,
	pub size: winit::dpi::PhysicalSize<u32>,
	depth_texture: Texture,
	camera_uniform: CameraUniform,
	camera_uniform_buffer: Buffer,
	camera_bind_group_layout: BindGroupLayout,
	camera_bind_group: BindGroup,

}
impl Renderer {
	pub async fn new(window: &Window) -> Self {
		let size = window.inner_size();

		let instance = wgpu::Instance::new(wgpu::Backends::all()); // Handle to GPU
		let surface = unsafe { instance.create_surface(window) };
		let adapter = instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(), // Dedicated GPU or low-power
				compatible_surface: Some(&surface), // Use an adapter compatible with surface
				force_fallback_adapter: false, // Don't use software renderer
			},
		).await.unwrap();

		let adapter_info = adapter.get_info();
		println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);
		println!("Features: {:?}", adapter.features());

		let (device, queue) = adapter.request_device(
			&wgpu::DeviceDescriptor {
				features: wgpu::Features::empty(), //wgpu::Features::TEXTURE_BINDING_ARRAY,
				limits: wgpu::Limits::default(),
				label: None,
			},
			None,
		).await.unwrap();

		// Initial surface configuration
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface.get_preferred_format(&adapter).unwrap(),
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
		};
		surface.configure(&device, &config);

		let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

		let camera_uniform = CameraUniform::new();
		let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Camera Bind Group Layout"),
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
            label: Some("Camera Bind Group"),
        });
		

		Self {
			surface,
			device,
			queue,
			config,
			size,
			depth_texture,
			camera_uniform,
			camera_uniform_buffer,
			camera_bind_group_layout,
			camera_bind_group,
		}
	}

	pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
		if new_size.width > 0 && new_size.height > 0 {
			self.size = new_size;
			self.config.width = new_size.width;
			self.config.height = new_size.height;
			self.surface.configure(&self.device, &self.config);

			self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
		}
	}

	// pub fn render(&self, view: &TextureView, camera: &Camera, data: &Vec<RenderData>) {

	// 	// Sort render data into buckets?
	// 	// Fetch based on id so we don't have to do it later

	// 	let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
	// 	let instance_buffer = self.device.create_buffer_init(
	// 		&wgpu::util::BufferInitDescriptor {
	// 			label: Some("Instance Buffer"),
	// 			contents: bytemuck::cast_slice(&instance_data),
	// 			usage: wgpu::BufferUsages::VERTEX,
	// 		}
	// 	);
		
	// 	// Resize camera so that there is not distortion?
	// 	self.camera_uniform.update(&camera);
		
	// 	let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
	// 		label: Some("Render Encoder"),
	// 	});

	// 	// Opaque pass
	// 	{
	// 		let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
	// 			label: Some("Opaque Pass"),
	// 			color_attachments: &[wgpu::RenderPassColorAttachment {
	// 				view: &view, // Texture to save to
	// 				resolve_target: None, // Same as view unless using multisampling
	// 				ops: wgpu::Operations {
	// 					load: wgpu::LoadOp::Clear(wgpu::Color {
	// 						r: 0.1,
	// 						g: 0.2,
	// 						b: 0.3,
	// 						a: 1.0,
	// 					}),
	// 					store: true,
	// 				},
	// 			}],
	// 			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
	// 				view: &self.depth_texture.view,
	// 				depth_ops: Some(wgpu::Operations {
	// 					load: wgpu::LoadOp::Clear(1.0),
	// 					store: true,
	// 				}),
	// 				stencil_ops: None,
	// 			}),
	// 		});

	// 		render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
			
	// 		// For shader
	// 		// render_pass.set_pipeline(&pipeline);

	// 		// For texture
	// 		// render_pass.set_bind_group(0, &tbg, &[]);

	// 		// For mesh
	// 		// render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
	// 		// render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

	// 		// render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
	// 		// Draw indexed with instance

	// 	}

	// 	// Transparent pass
	// 	// Enable alpha blends

	// 	self.queue.submit(std::iter::once(encoder.finish()));
	// }
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

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, /* padding */ 0];



const QUAD_VERTICES: &[Vertex] = &[
	Vertex { // Top left
		position: [-0.5, 0.5, 0.0], 
		tex_coords: [0.0, 0.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	},
	Vertex { // Top right
		position: [0.5, 0.5, 0.0], 
		tex_coords: [1.0, 0.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	}, 
	Vertex { // Bottom left
		position: [-0.5, -0.5, 0.0], 
		tex_coords: [0.0, 1.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	},
	Vertex { // Bottom right
		position: [0.5, -0.5, 0.0], 
		tex_coords: [1.0, 1.0], 
		normal: [0.0, 0.0, 1.0],
		tex_id: 0,
	},
];
