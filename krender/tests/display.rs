use krender::prelude::Texture;
use log::info;
use winit::{event_loop::{EventLoop, ControlFlow, EventLoopBuilder}, window::WindowBuilder, event::{Event, WindowEvent}, dpi::PhysicalSize};
use winit::platform::x11::EventLoopBuilderExtX11;


pub fn show_image(
	instance: wgpu::Instance,
	adapter: wgpu::Adapter,
	device: wgpu::Device,
	queue: wgpu::Queue,
	image: &wgpu::Texture,
) {
	let width = image.width();
	let height = image.height();

	let mut new_image = Texture::new("g", image.format().into(), image.width(), image.height(), 1, false)
		.with_usages(wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST);
	new_image.rebind(&device, &queue, todo!());


	info!("Copy to new image");
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
		label: None,
	});
	encoder.copy_texture_to_texture(
		wgpu::ImageCopyTexture {
			texture: &image,
			mip_level: 0,
			origin: wgpu::Origin3d::default(),
			aspect: wgpu::TextureAspect::All,
		}, 
		wgpu::ImageCopyTexture {
			texture: &new_image.binding().unwrap().texture,
			mip_level: 0,
			origin: wgpu::Origin3d::default(),
			aspect: wgpu::TextureAspect::All,
		},  
		wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		},
	);
	queue.submit([encoder.finish()].into_iter());
	info!("Done that");

	let event_loop = EventLoopBuilder::new()
		.with_any_thread(true)
		.build();
	let window = WindowBuilder::new()
		.with_inner_size(PhysicalSize::new(image.width(), image.height()))
		.build(&event_loop)
		.unwrap();

	let surface = unsafe { instance.create_surface(&window) }.unwrap();
	let surface_caps = surface.get_capabilities(&adapter);
	let config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::COPY_DST,
		format: image.format(),
		width: image.width(),
		height: image.height(),
		present_mode: surface_caps.present_modes[0],
		alpha_mode: surface_caps.alpha_modes[0],
		view_formats: vec![],
	};
	surface.configure(&device, &config);

	let redraw = move || {
		info!("Redraw");
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: None,
		});
		let surface_texture = surface.get_current_texture().unwrap();
		encoder.copy_texture_to_texture(
			wgpu::ImageCopyTexture {
				texture: &new_image.binding().unwrap().texture,
				mip_level: 0,
				origin: wgpu::Origin3d::default(),
				aspect: wgpu::TextureAspect::All,
			}, 
			wgpu::ImageCopyTexture {
				texture: &surface_texture.texture,
				mip_level: 0,
				origin: wgpu::Origin3d::default(),
				aspect: wgpu::TextureAspect::All,
			},  
			wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
		);
		queue.submit([encoder.finish()].into_iter());
		surface_texture.present();
	};

	event_loop.run(move |event, _, control_flow| {
		*control_flow = ControlFlow::Wait;

		match event {
			Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				window_id,
			} if window_id == window.id() => *control_flow = ControlFlow::Exit,
			Event::RedrawRequested(window_id) if window_id == window.id() => {
				redraw();
			},
			_ => (),
		}
	});

}

