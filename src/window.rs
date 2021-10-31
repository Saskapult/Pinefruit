//use render::Render;

use winit::{
	event::*,
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};
use crate::render::Render;



pub fn windowmain() {
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new().build(&event_loop).unwrap();

	let mut render = pollster::block_on(Render::new(&window));
	let mut last_render_time = std::time::Instant::now();
	event_loop.run(move |event, _, control_flow| 
		match event {
			Event::DeviceEvent {
                ref event,
                .. // We're not using device_id currently
            } => {
                render.input(event);
            }
			Event::WindowEvent {
				ref event,
				window_id,
			} if window_id == window.id() => {
				match event {
					// Close if close requested for 'esc' pressed
					WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
						input:
							KeyboardInput {
								state: ElementState::Pressed,
								virtual_keycode: Some(VirtualKeyCode::Escape),
								..
							},
						..
					} => *control_flow = ControlFlow::Exit,
					// Resize if resized
					WindowEvent::Resized(physical_size) => {
						render.resize(*physical_size);
					}
					// Resize if scaled
					WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
						// new_inner_size is &&mut so we have to dereference it twice
						render.resize(**new_inner_size);
					}
					// Else nothing
					_ => {},
				}
			}
			Event::RedrawRequested(_) => {
				let now = std::time::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;
				render.update(dt);
				match render.render() {
					Ok(_) => {}
					// Reconfigure the surface if lost
					Err(wgpu::SurfaceError::Lost) => render.resize(render.size),
					// The system is out of memory, we should probably quit
					Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
					// All other errors (Outdated, Timeout) should be resolved by the next frame
					Err(e) => eprintln!("{:?}", e),
				}
			}
			Event::MainEventsCleared => {
				// RedrawRequested will only trigger once, unless we manually
				// request it.
				window.request_redraw();
			}
			_ => {}
	});


}
