
use crate::{
	geometry::*,
	render::*,
    world::Map,
    texturemanagers::BlockTexturesManager,
};
use winit::{
	event::*,
	window::Window,
};
use nalgebra::*;
use wgpu::util::DeviceExt;


const VERTICES: &[Vertex] = &[
    Vertex { 
        position: [-0.0868241, 0.49240386, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.4131759, 0.00759614], 
        normal: [0.0, 0.0, 0.0],
    },
    Vertex { 
        position: [-0.49513406, 0.06958647, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.0048659444, 0.43041354], 
        normal: [0.0, 0.0, 0.0],
    }, 
    Vertex { 
        position: [-0.21918549, -0.44939706, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.28081453, 0.949397], 
        normal: [0.0, 0.0, 0.0],
    },
    Vertex { 
        position: [0.35966998, -0.3473291, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.85967, 0.84732914], 
        normal: [0.0, 0.0, 0.0],
    },
    Vertex { 
        position: [0.44147372, 0.2347359, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.9414737, 0.2652641], 
        normal: [0.0, 0.0, 0.0],
    },
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, /* padding */ 0];



const QUAD_VERTICES: &[Vertex] = &[
    Vertex { // Top left
        position: [-0.5, 0.5, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.0, 0.0], 
        normal: [0.0, 0.0, 1.0],
    },
    Vertex { // Top right
        position: [0.5, 0.5, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [1.0, 0.0], 
        normal: [0.0, 0.0, 1.0],
    }, 
    Vertex { // Bottom left
        position: [-0.5, -0.5, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [0.0, 1.0], 
        normal: [0.0, 0.0, 1.0],
    },
    Vertex { // Bottom right
        position: [0.5, -0.5, 0.0], 
        colour: [0.0, 0.0, 0.0],
        tex_coords: [1.0, 1.0], 
        normal: [0.0, 0.0, 1.0],
    },
];



pub struct Renderer {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
	depth_texture: Texture,
    block_textures_manager: BlockTexturesManager,
    camera: Camera,
    map: Map,
    temp_vb: wgpu::Buffer,
    temp_ib: wgpu::Buffer,
    temp_ni: u32,
}
impl Renderer {
    pub async fn new(window: &Window) -> Self {
		let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all()); // Handle to our GPU with any backend
        let surface = unsafe { instance.create_surface(window) };
		let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(), // Dedicated GPU or low-power
                compatible_surface: Some(&surface), // Find an adapter for this surface
                force_fallback_adapter: false, // Use software renderer
            },
        ).await.unwrap();

        let adapter_info = adapter.get_info();
        println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);
        println!("{:?}", adapter.features());

		let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::TEXTURE_BINDING_ARRAY,
                limits: wgpu::Limits::default(),
                label: None,
            },
            None, // Trace path
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

        let block_textures_manager = BlockTexturesManager::new(&device, &queue);

        let camera = Camera::new(
            Vector3::new(0.0, 0.0, 0.0),
            UnitQuaternion::look_at_lh(
                &Vector3::new(0.0, 0.0, 1.0),
                &Vector3::new(0.0, 1.0, 0.0),
            ),
            config.width as f32 / config.height as f32,
            45.0,
            0.1,
            100.0,
            &device,
        );

        let map = Map::new(&device, &config, &camera, &block_textures_manager);


        let temp_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let temp_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let temp_ni = INDICES.len() as u32;

		Self {
            surface,
            device,
            queue,
            config,
            size,
			depth_texture,
            block_textures_manager,
            camera,
            map,
            temp_vb,
            temp_ib,
            temp_ni,
        }
	}

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
			self.size = new_size;
			self.config.width = new_size.width;
			self.config.height = new_size.height;
			self.surface.configure(&self.device, &self.config);

			self.camera.resize(new_size.width, new_size.height);

			self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
		}
    }

    pub fn input(&mut self, event: &DeviceEvent) -> bool {
		// Send to input handler?
        // match event {
        //     DeviceEvent::Key(
        //         KeyboardInput {
        //             virtual_keycode: Some(key),
        //             state,
        //             ..
        //         }
        //     ) => self.camera.camera_controller.process_keyboard(*key, *state),
        //     _ => false,
        // }
        true
    }

    pub fn update(&mut self, dt: std::time::Duration) {
		//self.dt = dt;
		// Send to input handler?

        //self.camera.update_camera(dt);

    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
		// The texture the surface displays
        let output = self.surface.get_current_texture()?;
		// A view for that texture
		let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
		
		// An encoder lets us send commands in gpu-readable encoding
		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Render Encoder"),
		});

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[wgpu::RenderPassColorAttachment {
					view: &view, // Texture to save to
					resolve_target: None, // Same as view unless using multisampling
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							r: 0.1,
							g: 0.2,
							b: 0.3,
							a: 1.0,
						}),
						store: true,
					},
				}],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &self.depth_texture.view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: true,
					}),
					stencil_ops: None,
				}),
			});
            
            render_pass.set_pipeline(&self.map.chunk_render_pipeline);
            render_pass.set_bind_group(0, &self.block_textures_manager.texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera.camera_bind_group, &[]);
            
            render_pass.set_vertex_buffer(0, self.temp_vb.slice(..));
            render_pass.set_index_buffer(self.temp_ib.slice(..), wgpu::IndexFormat::Uint16);
            
            render_pass.draw_indexed(0..self.temp_ni, 0, 0..1);

		}
	
		// submit will accept anything that implements IntoIter
		self.queue.submit(std::iter::once(encoder.finish()));
		output.present();
	
		Ok(())
    }

}



