
use winit::{
	event::*,
	window::Window,
};
use wgpu::util::DeviceExt;
use crate::texture;
use crate::camera;
use crate::model;
use crate::model::Vertex;
use crate::model::DrawModel;
use nalgebra::*;
use glam;


// An instance of an object
struct Instance {
    position: Vector3<f32>,
    rotation: UnitQuaternion<f32>,
}
impl Instance {
    fn to_raw(&self) -> InstanceRaw {
		let model = glam::Mat4::IDENTITY;
        //let model = self.rotation.to_homogeneous() * Matrix4::new_translation(&self.position);
        InstanceRaw {
            model: model.to_cols_array_2d(),
        }
    }
}

// Raw matrix representation of an instance
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
}
impl model::Vertex for InstanceRaw {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We don't have to do this in code though.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}



//
// Uniforms
//

// Shared camera data
#[repr(C)] // Make compatible with shaders
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)] // Be able to store in buffer
struct CameraUniform {
    // Mat doesn't implement Zeroable so we need to convert to array literals
	pos: [f32; 4],
    vp: [[f32; 4]; 4],
	ip: [[f32; 4]; 4],
}
impl CameraUniform {
    fn new() -> Self {
        Self {
			pos: [0.0, 0.0, 0.0, 0.0],
            vp: Matrix4::identity().into(),
			ip: Matrix4::identity().into(),
        }
    }
    fn update(&mut self, camera: &camera::Camera) {
        self.pos = camera.position.to_homogeneous().into();
		let p = camera.proj_matrix();
        self.vp = (p * camera.cam_matrix()).into();
    }

}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    color: [f32; 3],
}


//
// Good Stuff
//

pub struct Render {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
	render_pipeline: wgpu::RenderPipeline,
	depth_texture: texture::Texture,
	mouse_pressed: bool,
	// Lights
	light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    //light_render_pipeline: wgpu::RenderPipeline,
	// Camera
	camera: camera::Camera,
	camera_uniform: CameraUniform,
	camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
	camera_controller: camera::CameraController,
	// Objects
	// If new instances are added, recreate instance_buffer and camera_bind_group or they will not appear
	instances: Vec<Instance>, 
    instance_buffer: wgpu::Buffer,
	obj_model: model::Model,
}
impl Render {
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

		let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
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


		//
		// Texture
		//

		println!("Texture!");

		let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				// Diffuse texture!
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler {
						// This is only for TextureSampleType::Depth
						comparison: false,
						// This should be true if the sample_type of the texture is:
						//     TextureSampleType::Float { filterable: true }
						// Otherwise you'll get an error.
						filtering: true,
					},
					count: None,
				},
				// Normal map!
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 3,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler { 
						comparison: false,
						filtering: true, 
					},
					count: None,
				},
		
			],
			label: Some("texture_bind_group_layout"),
		});

		let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");


		//
		// Lights
		//

		println!("Lights!");

		let light_uniform = LightUniform {
            position: [2.0, 2.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
        };

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                label: None,
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });


		//
		// Camera
		//

		println!("Camera!");

		let camera = camera::Camera::new(
			Vector3::new(0.0, 0.0, 0.0), 
			UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0), 
			config.width as f32 / config.height as f32, 
			0.785, 0.1, 100.0);
		let camera_controller = camera::CameraController::new(4.0, 0.4);

		let mut camera_uniform = CameraUniform::new();
		camera_uniform.update(&camera);

		let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera Buffer"),
			contents: bytemuck::cast_slice(&[camera_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}
			],
			label: Some("camera_bind_group_layout"),
		});

		let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &camera_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: camera_buffer.as_entire_binding(),
				}
			],
			label: Some("camera_bind_group"),
		});


		//
		// Render Pipeline
		//

		println!("Action! (render pipeline)");

		// All bind group layouts must be described here
		let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Render Pipeline Layout"),
			bind_group_layouts: &[
				&texture_bind_group_layout,
				&camera_bind_group_layout,
				&light_bind_group_layout,
				],
			push_constant_ranges: &[],
    	});

		let render_pipeline = {
			println!("Shader!");
			let shader = wgpu::ShaderModuleDescriptor {
				label: Some("Normal Shader"),
				source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
			};
			println!("Pipeline!");
			create_render_pipeline(
				&device,
				&render_pipeline_layout,
				config.format,
				Some(texture::Texture::DEPTH_FORMAT),
				&[model::ModelVertex::desc(), InstanceRaw::desc()],
				shader,
			)
		};

		// println!("Action again! (lights pipeline)");

		// let light_render_pipeline = {
        //     let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        //         label: Some("Light Pipeline Layout"),
        //         bind_group_layouts: &[
		// 			&camera_bind_group_layout, 
		// 			&light_bind_group_layout,
		// 			],
        //         push_constant_ranges: &[],
        //     });
        //     let shader = wgpu::ShaderModuleDescriptor {
        //         label: Some("Light Shader"),
        //         source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
        //     };
        //     create_render_pipeline(
        //         &device,
        //         &layout,
        //         config.format,
        //         Some(texture::Texture::DEPTH_FORMAT),
        //         &[model::ModelVertex::desc()],
        //         shader,
        //     )
        // };
		
	
		//
		// Instances
		//

		println!("Instances!");

		let mut instances = Vec::new();
		instances.push(Instance {
            position: Vector3::new(0.0, 0.0, 0.0), 
			rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0),
        });
		instances.push(Instance {
            position: Vector3::new(10.0, 0.0, 0.0), 
			rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0),
        });
		instances.push(Instance {
            position: Vector3::new(0.0, 10.0, 0.0), 
			rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0),
        });
		instances.push(Instance {
            position: Vector3::new(0.0, 0.0, 10.0), 
			rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0),
        });

		let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
		let instance_buffer = device.create_buffer_init(
			&wgpu::util::BufferInitDescriptor {
				label: Some("Instance Buffer"),
				contents: bytemuck::cast_slice(&instance_data),
				usage: wgpu::BufferUsages::VERTEX,
			}
		);

		let res_dir = std::path::Path::new(env!("OUT_DIR")).join("res");
		let obj_model = model::Model::load(
			&device,
			&queue,
			&texture_bind_group_layout,
			res_dir.join("cube.obj"),
		).unwrap();


		// let _debug_material = {
        //     let diffuse_bytes = include_bytes!("../res/cobble-diffuse.png");
        //     let normal_bytes = include_bytes!("../res/cobble-normal.png");

        //     let diffuse_texture = texture::Texture::from_bytes(
        //         &device,
        //         &queue,
        //         diffuse_bytes,
        //         "res/alt-diffuse.png",
        //         false,
        //     )
        //     .unwrap();
        //     let normal_texture = texture::Texture::from_bytes(
        //         &device,
        //         &queue,
        //         normal_bytes,
        //         "res/alt-normal.png",
        //         true,
        //     )
        //     .unwrap();

        //     model::Material::new(
        //         &device,
        //         "alt-material",
        //         diffuse_texture,
        //         normal_texture,
        //         &texture_bind_group_layout,
        //     )
        // };


		Self {
            surface,
            device,
            queue,
            config,
            size,
			render_pipeline,
			depth_texture,
			mouse_pressed: false,
			light_uniform,
  			light_buffer,
  			light_bind_group,
  			//light_render_pipeline,
			camera,
			camera_uniform,
			camera_buffer,
	        camera_bind_group,
			camera_controller,
			instances,
    		instance_buffer,
			obj_model,
        }
	}

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
			self.size = new_size;
			self.config.width = new_size.width;
			self.config.height = new_size.height;
			self.surface.configure(&self.device, &self.config);

			self.camera.resize(new_size.width, new_size.height);

			self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
		}
    }

    pub fn input(&mut self, event: &DeviceEvent) -> bool {
        match event {
			DeviceEvent::Key(
				KeyboardInput {
					virtual_keycode: Some(key),
					state,
					..
				}
			) => self.camera_controller.process_keyboard(*key, *state),
			DeviceEvent::MouseWheel { delta, .. } => {
				self.camera_controller.process_scroll(&*delta);
				true
			}
			DeviceEvent::Button {
				button: 1, // Left Mouse Button
				state,
			} => {
				self.mouse_pressed = *state == ElementState::Pressed;
				true
			}
			DeviceEvent::MouseMotion { delta } => {
				if self.mouse_pressed {
					self.camera_controller.process_mouse(delta.0, delta.1);
				}
				true
			}
			_ => false,
		}
	
    }

    pub fn update(&mut self, dt: std::time::Duration) {
		self.camera_controller.update_camera(&mut self.camera, dt);
	    self.camera_uniform.update(&self.camera);
	    self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
		let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
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

			// Set instance buffer
			render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..)); 
			
			render_pass.set_pipeline(&self.render_pipeline);
			render_pass.draw_model_instanced(
				&self.obj_model,
				0..self.instances.len() as u32,
				&self.camera_bind_group,
				&self.light_bind_group,
			);


		}
	
		// submit will accept anything that implements IntoIter
		self.queue.submit(std::iter::once(encoder.finish()));
		output.present();
	
		Ok(())
    }
}


// Creates a new render pipeline
fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(&shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
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
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
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
    })
}
