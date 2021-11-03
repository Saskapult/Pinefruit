
use crate::{
	model::{
		texture::*,
		model::{Model, DrawModel},
		material::Material,
		mesh::Mesh,
		vertex::Vertex,
	},
	render::{
		camera::{
			Camera, 
			CameraController,
		},
		uniforms::{
			CameraUniform, 
			LightUniform,
		},
		modelgroup::ModelGroup,
	},
	entity::instance::{
		Instance, 
		InstanceRaw,
	},
};
use winit::{
	event::*,
	window::Window,
};
use wgpu::util::DeviceExt;
use nalgebra::*;
use std::collections::HashMap;





pub struct Renderer {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
	pub depth_texture: Texture,
	pub dt: std::time::Duration,
	// render_pipeline: wgpu::RenderPipeline,
	//wireframe_mode: bool,
	// mouse_pressed: bool,
	// Lights
	// light_uniform: LightUniform,
    // light_buffer: wgpu::Buffer,
    // light_bind_group: wgpu::BindGroup,
    // //light_render_pipeline: wgpu::RenderPipeline,
	// // Camera
	// camera: Camera,
	// camera_uniform: CameraUniform,
	// camera_buffer: wgpu::Buffer,
    // camera_bind_group: wgpu::BindGroup,
	// camera_controller: CameraController,
	// // Objects
	// // If new instances are added, recreate instance_buffer and camera_bind_group or they will not appear
	// modelgroups: Vec<ModelGroup>,	// Mesh and materials to draw from
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


		// //
		// // Texture
		// //

		// println!("Texture!");

		// let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		// 	entries: &[
		// 		// Diffuse texture!
		// 		wgpu::BindGroupLayoutEntry {
		// 			binding: 0,
		// 			visibility: wgpu::ShaderStages::FRAGMENT,
		// 			ty: wgpu::BindingType::Texture {
		// 				multisampled: false,
		// 				view_dimension: wgpu::TextureViewDimension::D2,
		// 				sample_type: wgpu::TextureSampleType::Float { filterable: true },
		// 			},
		// 			count: None,
		// 		},
		// 		wgpu::BindGroupLayoutEntry {
		// 			binding: 1,
		// 			visibility: wgpu::ShaderStages::FRAGMENT,
		// 			ty: wgpu::BindingType::Sampler {
		// 				// This is only for TextureSampleType::Depth
		// 				comparison: false,
		// 				// This should be true if the sample_type of the texture is:
		// 				//     TextureSampleType::Float { filterable: true }
		// 				// Otherwise you'll get an error.
		// 				filtering: true,
		// 			},
		// 			count: None,
		// 		},
		// 		// Normal map!
		// 		wgpu::BindGroupLayoutEntry {
		// 			binding: 2,
		// 			visibility: wgpu::ShaderStages::FRAGMENT,
		// 			ty: wgpu::BindingType::Texture {
		// 				multisampled: false,
		// 				sample_type: wgpu::TextureSampleType::Float { filterable: true },
		// 				view_dimension: wgpu::TextureViewDimension::D2,
		// 			},
		// 			count: None,
		// 		},
		// 		wgpu::BindGroupLayoutEntry {
		// 			binding: 3,
		// 			visibility: wgpu::ShaderStages::FRAGMENT,
		// 			ty: wgpu::BindingType::Sampler { 
		// 				comparison: false,
		// 				filtering: true, 
		// 			},
		// 			count: None,
		// 		},
		
		// 	],
		// 	label: Some("texture_bind_group_layout"),
		// });

		// let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");


		// //
		// // Lights
		// //

		// println!("Lights!");

		// let light_uniform = LightUniform {
        //     position: [2.0, 2.0, 2.0],
        //     _padding: 0,
        //     color: [1.0, 1.0, 1.0],
        // };

        // let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("Light VB"),
        //     contents: bytemuck::cast_slice(&[light_uniform]),
        //     usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        // });

        // let light_bind_group_layout =
        //     device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        //         entries: &[wgpu::BindGroupLayoutEntry {
        //             binding: 0,
        //             visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        //             ty: wgpu::BindingType::Buffer {
        //                 ty: wgpu::BufferBindingType::Uniform,
        //                 has_dynamic_offset: false,
        //                 min_binding_size: None,
        //             },
        //             count: None,
        //         }],
        //         label: None,
        //     });

        // let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     layout: &light_bind_group_layout,
        //     entries: &[wgpu::BindGroupEntry {
        //         binding: 0,
        //         resource: light_buffer.as_entire_binding(),
        //     }],
        //     label: None,
        // });


		// //
		// // Camera
		// //

		// println!("Camera!");

		// let camera = Camera::new(
		// 	Vector3::new(0.0, 0.0, 0.0), 
		// 	UnitQuaternion::face_towards(
		// 			&Vector3::z(),
		// 			&Vector3::y(),
		// 		), 
		// 	config.width as f32 / config.height as f32, 
		// 	0.785, 0.1, 100.0);
		// let camera_controller = CameraController::new(4.0, 0.4);

		// let mut camera_uniform = CameraUniform::new();
		// camera_uniform.update(&camera);

		// let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		// 	label: Some("Camera Buffer"),
		// 	contents: bytemuck::cast_slice(&[camera_uniform]),
		// 	usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		// });

		// let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		// 	entries: &[
		// 		wgpu::BindGroupLayoutEntry {
		// 			binding: 0,
		// 			visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
		// 			ty: wgpu::BindingType::Buffer {
		// 				ty: wgpu::BufferBindingType::Uniform,
		// 				has_dynamic_offset: false,
		// 				min_binding_size: None,
		// 			},
		// 			count: None,
		// 		}
		// 	],
		// 	label: Some("camera_bind_group_layout"),
		// });

		// let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		// 	layout: &camera_bind_group_layout,
		// 	entries: &[
		// 		wgpu::BindGroupEntry {
		// 			binding: 0,
		// 			resource: camera_buffer.as_entire_binding(),
		// 		}
		// 	],
		// 	label: Some("camera_bind_group"),
		// });


		// //
		// // Render Pipeline
		// //

		// println!("Action! (render pipeline)");

		// // All bind group layouts must be described here
		// let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		// 	label: Some("Render Pipeline Layout"),
		// 	bind_group_layouts: &[
		// 		&texture_bind_group_layout,
		// 		&camera_bind_group_layout,
		// 		&light_bind_group_layout,
		// 		],
		// 	push_constant_ranges: &[],
    	// });

		// let render_pipeline = {
		// 	println!("Shader!");
		// 	let shader = wgpu::ShaderModuleDescriptor {
		// 		label: Some("Normal Shader"),
		// 		source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
		// 	};
		// 	println!("Pipeline!");
		// 	create_render_pipeline(
		// 		&device,
		// 		&render_pipeline_layout,
		// 		config.format,
		// 		Some(Texture::DEPTH_FORMAT),
		// 		&[Vertex::desc(), InstanceRaw::desc()],
		// 		shader,
		// 	)
		// };

		// // println!("Action again! (lights pipeline)");

		// // let light_render_pipeline = {
        // //     let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        // //         label: Some("Light Pipeline Layout"),
        // //         bind_group_layouts: &[
		// // 			&camera_bind_group_layout, 
		// // 			&light_bind_group_layout,
		// // 			],
        // //         push_constant_ranges: &[],
        // //     });
        // //     let shader = wgpu::ShaderModuleDescriptor {
        // //         label: Some("Light Shader"),
        // //         source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
        // //     };
        // //     create_render_pipeline(
        // //         &device,
        // //         &layout,
        // //         config.format,
        // //         Some(texture::Texture::DEPTH_FORMAT),
        // //         &[model::ModelVertex::desc()],
        // //         shader,
        // //     )
        // // };
		

		// let instances = [
		// 	Instance {
        //     	position: Vector3::new(0.0, 0.0, 0.0), 
		// 		rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0),
        // 	},
		// ].to_vec();

		// let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
		// let instance_buffer = device.create_buffer_init(
		// 	&wgpu::util::BufferInitDescriptor {
		// 		label: Some("Instance Buffer"),
		// 		contents: bytemuck::cast_slice(&instance_data),
		// 		usage: wgpu::BufferUsages::VERTEX,
		// 	}
		// );

		// let resources_dir = std::path::Path::new(env!("OUT_DIR")).join("res");


		
		// let t_red = Texture::load(&device, &queue, &resources_dir.join("cube-diffuse-red.jpg"), TextureType::DiffuseTexture).expect("f");
		// let t_blue = Texture::load(&device, &queue, &resources_dir.join("cube-diffuse-blue.jpg"), TextureType::DiffuseTexture).expect("f");
		// let t_green = Texture::load(&device, &queue, &resources_dir.join("cube-diffuse-green.jpg"), TextureType::DiffuseTexture).expect("f");
		// let t_n = Texture::load(&device, &queue, &resources_dir.join("cube-normal.png"), TextureType::NormalTexture).expect("f");

		// let m_red = Material::new("m_red".to_string(), &t_red, &t_n, &device, &texture_bind_group_layout);
		// let m_blue = Material::new("m_blue".to_string(), &t_blue, &t_n, &device, &texture_bind_group_layout);
		// let m_green = Material::new("m_green".to_string(), &t_green, &t_n, &device, &texture_bind_group_layout);

		// let cube_mesh = Model::from_obj(&resources_dir.join("cube.obj"), &device, &queue, &texture_bind_group_layout);

		// // let cube_model = Model {
		// // 	meshes: cube_mesh,
		// // 	materials: ma,
		// // }
		// let cube_model = Model::from_obj(&resources_dir.join("cube.obj"), &device, &queue, &texture_bind_group_layout);

		// let mut modelgroups = Vec::new();


		// let mut cube_model_group = ModelGroup::new(&device, "cube_mg".to_string(), cube_model);
		// cube_model_group.add_instance(&device, Instance {
		// 	position: Vector3::new(0.0, 0.0, 0.0), 
		// 	rotation: UnitQuaternion::from_euler_angles(0.0, 0.0, 0.0),
		// });

		// modelgroups.push(cube_model_group);


		Self {
            surface,
            device,
            queue,
            config,
            size,
			// render_pipeline,
			depth_texture,
			mouse_pressed: false,
			// light_uniform,
  			// light_buffer,
  			// light_bind_group,
  			//light_render_pipeline,
			// camera,
			// camera_uniform,
			// camera_buffer,
	        // camera_bind_group,
			// camera_controller,
			// modelgroups,
        }
	}

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
			self.size = new_size;
			self.config.width = new_size.width;
			self.config.height = new_size.height;
			self.surface.configure(&self.device, &self.config);

			//self.camera.resize(new_size.width, new_size.height);

			self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
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
		self.dt = dt;
		// self.camera_controller.update_camera(&mut self.camera, dt);
	    // self.camera_uniform.update(&self.camera);
	    // self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
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



			render_pass.set_pipeline(&self.render_pipeline);



			// for mg in &self.modelgroups {
			// 	render_pass.set_vertex_buffer(1, mg.instance_buffer.slice(..)); 
			// 	render_pass.draw_model_instanced(
			// 		&mg.model,
			// 		0..mg.instances.len() as u32,
			// 		&self.camera_bind_group,
			// 		&self.light_bind_group,
			// 	);
			// }



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
