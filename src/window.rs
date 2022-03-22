
use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use wgpu;
use std::collections::{HashMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, Duration};
use egui;
use crate::ecs::*;
use specs::{WorldExt, Entity};




pub struct GameWindow {
	pub window: Window,
	pub surface: wgpu::Surface,
	pub surface_config: wgpu::SurfaceConfiguration,
	pub platform: egui_winit_platform::Platform,
	pub previous_frame_time: Option<f32>,
	pub cursor_inside: bool,
}
impl GameWindow {
	pub fn new(instance: &wgpu::Instance, adapter: &wgpu::Adapter, window: Window) -> Self {
		let surface = unsafe { instance.create_surface(&window) };
		let size = window.inner_size();
		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
			format: surface.get_preferred_format(&adapter).unwrap(),
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
		};
		info!("Created new game window with format {:?}", &surface_config.format);

		let platform = egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
			physical_width: size.width as u32,
			physical_height: size.height as u32,
			scale_factor: window.scale_factor(),
			font_definitions: egui::FontDefinitions::default(),
			style: egui::Style {
				visuals: egui::style::Visuals {
					widgets: egui::style::Widgets {
						noninteractive: egui::style::WidgetVisuals {
							// window background
							bg_fill: egui::Color32::TRANSPARENT, 
							// separators, indentation lines, windows outlines
							bg_stroke: egui::Stroke::none(),
							// normal text color
							fg_stroke: egui::Stroke::new(1.0, egui::Color32::WHITE), 
							corner_radius: 0.0,
							expansion: 0.0,
						},
						..Default::default()
					},
					..Default::default()
				},
				..Default::default()
			},
		});

		Self {
			window,
			surface,
			surface_config,
			platform,
			previous_frame_time: None,
			cursor_inside: false,
		}
	}

	// To be called by the game when there is a resize event in the queue
	pub fn resize(&mut self, device: &wgpu::Device, new_size: winit::dpi::PhysicalSize<u32>) {
		if new_size.width > 0 && new_size.height > 0 {
			self.surface_config.width = new_size.width;
			self.surface_config.height = new_size.height;
			self.surface.configure(&device, &self.surface_config);		
		}
	}

	fn render_ui(
		&mut self,
		mut encoder: &mut wgpu::CommandEncoder,
		render_resource: &mut RenderResource,
		destination_view: &wgpu::TextureView,
		world: &specs::World,
		entity: Entity,
	) {
		let egui_start = Instant::now();
		self.platform.begin_frame();

		let input = egui::RawInput::default();
		let (_output, shapes) = self.platform.context().run(input, |egui_ctx| {
			egui::SidePanel::left("info panel").min_width(300.0).resizable(false).show(egui_ctx, |ui| {
				// Some of these values are not from the same step so percentages will be inaccurate

				ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);

				// Transform info
				if let Some(transform) = world.read_component::<TransformComponent>().get(entity) {
					// Find camera for this window or something idk
					let pos = transform.position;
					ui.label(format!("pos: [{:.1}, {:.1}, {:.1}]", pos[0], pos[1], pos[2]));
					let world_pos = [
						pos[0].floor() as i32,
						pos[1].floor() as i32,
						pos[2].floor() as i32,
					];
					ui.label(format!("world: {:?}", world_pos));
					let cpos = [
						world_pos[0].div_euclid(16),
						world_pos[1].div_euclid(16),
						world_pos[2].div_euclid(16),
					];
					let mut vpos = [
						(world_pos[0] % 16 + 16) % 16,
						(world_pos[1] % 16 + 16) % 16,
						(world_pos[2] % 16 + 16) % 16,
					];
					vpos.iter_mut().zip([16; 3].iter()).for_each(|(v, cs)| {
						if *v < 0 {
							*v = *cs as i32 + *v;
						}
					});
					ui.label(format!("chunk: {:?} - {:?}", cpos, vpos));
				}

				// Lookat info
				if let Some(marker_stuff) = world.read_component::<MarkerComponent>().get(entity) {
					let look_pos= marker_stuff.look_pos;
					let normal = marker_stuff.look_normal;
					let normal_str = match normal.map(|f| f.round() as i32) {
						[1, 0, 0] => "xp",
						[0, 1, 0] => "yp",
						[0, 0, 1] => "zp",
						[-1, 0, 0] => "xn",
						[0, -1, 0] => "yn",
						[0, 0, -1] => "zn",
						_ => "unaligned",
					};
					ui.label(format!("look at: {:?} ({:?})", look_pos, marker_stuff.look_v));
					ui.label(format!("look normal: {:?} ({})", normal, normal_str));
				}

				let steptime = world.read_resource::<StepResource>().step_durations.latest().unwrap_or(Duration::ZERO);
				ui.label(format!("step time: {}ms", steptime.as_millis()));
				
				{
					let encodetime = render_resource.encode_durations.latest().unwrap_or(Duration::ZERO);
					let encodep = encodetime.as_secs_f32() / steptime.as_secs_f32() * 100.0;
					ui.label(format!("encode time: {:>2}ms (~{:.2}%)", encodetime.as_millis(), encodep));
					{
						let rupdate_time = render_resource.instance.update_durations.latest().unwrap_or(Duration::ZERO);
						let rupdate_p = rupdate_time.as_secs_f32() / steptime.as_secs_f32() * 100.0;
						ui.label(format!("rupdate time: {:>2}ms (~{:.2}%)", rupdate_time.as_millis(), rupdate_p));

						let rencode_time = render_resource.instance.encode_durations.latest().unwrap_or(Duration::ZERO);
						let rencode_p = rencode_time.as_secs_f32() / steptime.as_secs_f32() * 100.0;
						ui.label(format!("rencode time: {:>2}ms (~{:.2}%)", rencode_time.as_millis(), rencode_p));
					}

					let submit_time = render_resource.submit_durations.latest().unwrap_or(Duration::ZERO);
					let submit_p = submit_time.as_secs_f32() / steptime.as_secs_f32() * 100.0;
					ui.label(format!("submit time: {:>2}ms (~{:.2}%)", submit_time.as_millis(), submit_p));
				}

				let physics_time = {
					let physics_resource = world.read_resource::<PhysicsResource>();
					physics_resource.tick_durations.latest().unwrap_or(Duration::ZERO)
				};
				let physics_p = physics_time.as_secs_f32() / steptime.as_secs_f32() * 100.0;
				ui.label(format!("physics time: {:>2}ms (~{:.2}%)", physics_time.as_millis(), physics_p));

				

				// if ui.button("Clickme").clicked() {
				// 	panic!("Button click");
				// }
			});
			// egui::Area::new("centre cursor panel").interactable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(egui_ctx, |ui| { 
			// 	ui.label(format!("X"));
			// });
			// egui::TopBottomPanel::bottom("selected block panel").show(egui_ctx, |ui| { 
			// 	ui.label(format!("Selected: "));
			// });
		});

		let paint_jobs = self.platform.context().tessellate(shapes);

		let frame_time = (Instant::now() - egui_start).as_secs_f64() as f32;
		self.previous_frame_time = Some(frame_time);

		let device = render_resource.instance.device.clone();
		let queue = render_resource.instance.queue.clone();

		// GPU upload
		let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
			physical_width: self.window.outer_size().width,
			physical_height: self.window.outer_size().height,
			scale_factor: self.window.scale_factor() as f32,
		};
		render_resource.egui_rpass.update_texture(
			&device, 
			&queue, 
			&self.platform.context().font_image(),
		);
		render_resource.egui_rpass.update_user_textures(
			&device, 
			&queue,
		);
		render_resource.egui_rpass.update_buffers(
			&device, 
			&queue, 
			&paint_jobs, 
			&screen_descriptor,
		);

		render_resource.egui_rpass.execute(
			&mut encoder,
			destination_view,
			&paint_jobs,
			&screen_descriptor,
			None,
		).unwrap();
	}

	fn render_game<'b>(
		&self,
		mut encoder: &mut wgpu::CommandEncoder,
		camera: &CameraComponent,
		camera_transform: &TransformComponent,
		destination_texture: &wgpu::Texture,
		render_resource: &mut RenderResource,
	) {
		render_resource.instance.set_data(camera.render_data.clone());

		let render_camera = crate::render::Camera {
			position: camera_transform.position,
			rotation: camera_transform.rotation,
			fovy: camera.fovy,
			znear: camera.znear,
			zfar: camera.zfar,
		};

		let width = self.surface_config.width;
		let height = self.surface_config.height;
		
		render_resource.instance.render(
			&mut encoder,
			destination_texture, 
			width, 
			height, 
			&render_camera, 
			Instant::now(),
		);
	}

	pub fn draw(
		&mut self,
		world: &mut specs::World,
		entity: Entity,
	) {

		let mut render_resource = world.write_resource::<RenderResource>(); 

		self.surface.configure(&render_resource.instance.device, &self.surface_config);

		let frame = match self.surface.get_current_texture() {
			Ok(tex) => tex,
			Err(wgpu::SurfaceError::Outdated) => {
				// Apparently happens when minimized on Windows
				error!("Render to outdated texture for window");
				panic!();
			},
			Err(e) => {
				panic!("{}", e);
			},
		};
		let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = render_resource.instance.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Render Encoder"),
		});

		// Game
		{
			let encode_st = Instant::now();
			let ccs = world.read_component::<CameraComponent>();
			let camera = ccs.get(entity)
				.expect("Render point has no camera!");
			let tcs = world.read_component::<TransformComponent>();
			let camera_transform = tcs.get(entity)
				.expect("Render camera has no transform!");
			self.render_game(
				&mut encoder,
				camera,
				camera_transform,
				&frame.texture,
				&mut render_resource,
			);
			render_resource.encode_durations.record(Instant::now() - encode_st);
		}

		// Ui
		self.render_ui(
			&mut encoder,
			&mut render_resource,
			&frame_view,
			world,
			entity,
		);

		// Submit
		let submit_st = Instant::now();
		render_resource.instance.queue.submit(std::iter::once(encoder.finish()));
		render_resource.submit_durations.record(Instant::now() - submit_st);
		
		// Present
		frame.present();
	}
}



#[derive(Debug)]
pub struct EventWhen {
	pub event: Event<'static, EventLoopEvent>,
	pub when: std::time::Instant,
}


// A custom event which can be injected into the event loop
#[derive(Debug)]
pub enum EventLoopEvent {
	Close,
	CreateWindow,
	RegisterWindow(Window),
	SupplyWindow(mpsc::Sender<Window>),
}


pub fn run_event_loop(
	event_loop: EventLoop<EventLoopEvent>, 
	event_queue: Arc<Mutex<Vec<EventWhen>>>,
) {
	event_loop.run(move |event, event_loop, control_flow| {
		match event {
			Event::UserEvent(event) => {
				match event {
					EventLoopEvent::Close => *control_flow = ControlFlow::Exit,
					EventLoopEvent::CreateWindow => {
						let window = WindowBuilder::new()
							.with_title("window title")
							.build(event_loop)
							.unwrap();
						let ew = EventWhen {
							event: Event::UserEvent(EventLoopEvent::RegisterWindow(window)),
							when: std::time::Instant::now(),
						};
						event_queue.lock().unwrap().push(ew);
					}
					EventLoopEvent::SupplyWindow(sender) => {
						let window = WindowBuilder::new().build(event_loop).unwrap();
						sender.send(window).expect("error sending window");
					}
					_ => {},
				}
			},
			_ => {
				// Not a memory leak because 'static implies that it *can* live forever, not that it does live forever
				if let Some(event) = event.to_static() {
					let ew = EventWhen {
						event,
						when: std::time::Instant::now(),
					};
					event_queue.lock().unwrap().push(ew);
				}
			},
		}
	});
	
}


pub fn new_event_loop() -> EventLoop<EventLoopEvent> {
	EventLoop::<EventLoopEvent>::with_user_event()
}


pub fn new_queue() -> Arc<Mutex<Vec<EventWhen>>> {
	Arc::new(Mutex::new(Vec::<EventWhen>::new()))
}



// Manages windows
// Creates/destroys windows and can read their events
const CLOSE_ON_NO_WINDOWS: bool = true;
pub struct WindowManager {
	pub windows: Vec<GameWindow>,
	id_idx: HashMap<WindowId, usize>,
	event_loop_sender: mpsc::SyncSender<EventLoopEvent>,
	event_thread_handle: thread::JoinHandle<i32>,
	instance: wgpu::Instance,
	adapter: wgpu::Adapter,
	event_queue: Arc<Mutex<Vec<EventWhen>>>,
	pub window_redraw_queue: BTreeSet<usize>,
	pub capturing_cursor: bool,
	last_update: Instant,
}
impl WindowManager {
	pub fn new(
		event_loop_proxy: EventLoopProxy<EventLoopEvent>,
		instance: wgpu::Instance,
		adapter: wgpu::Adapter,
		event_queue: Arc<Mutex<Vec<EventWhen>>>,
	) -> Self {
		let windows = Vec::new();
		let id_idx = HashMap::new();

		let (event_loop_sender, event_loop_receiver) = mpsc::sync_channel(10);

		// Event loop proxy needs to stay in its own thread because it cannot sync
		let event_thread_handle = thread::Builder::new()
			.name("winit event thread".into())
			.spawn(move || {
				let event_loop_proxy = event_loop_proxy.clone();
				loop {
					match event_loop_receiver.recv() {
						Ok(event) => {
							event_loop_proxy.send_event(event).expect("Failed to send window creation request!");
						},
						Err(_) => {
							error!("Failed to recv in event thread, sending close signal");
							event_loop_proxy.send_event(EventLoopEvent::Close).expect("Failed to send event loop close signal!");
							return 1
						}
					}
				}
			})
			.expect("Failed to spawn winit event thread!");

		Self {
			windows,
			id_idx,
			event_loop_sender,
			event_thread_handle,
			instance,
			adapter,
			event_queue,
			window_redraw_queue: BTreeSet::new(),
			capturing_cursor: false,
			last_update: Instant::now(),
		}
	}

	pub fn get_window_index(&self, id: &WindowId) -> usize {
		self.id_idx[id]
	}

	// Request a window (to be processed in the next iteration)
	pub fn request_window(&mut self) {
		// Insert an event to create a new window
		// A RegisterWindow event with the window should be inserted into the queue
		// It should be processed in the next iteration
		self.event_loop_sender.send(EventLoopEvent::CreateWindow)
			.expect("Failed to send window creation request!");
	}

	// Get a window fastly (in this iteration)
	pub fn request_window_immediate(&mut self) {
		let (sender, receiver) = mpsc::channel();
		self.event_loop_sender.send(EventLoopEvent::SupplyWindow(sender))
			.expect("Failed to send window creation request");
		let window = receiver.recv().unwrap();
		self.register_window(window);
	}

	pub fn register_window(&mut self, window: Window) -> usize {
		let gamewindow = GameWindow::new(&self.instance, &self.adapter, window);

		let idx = self.windows.len();
		self.id_idx.insert(gamewindow.window.id(), idx);
		self.windows.push(gamewindow);
		idx
	}

	pub fn close_window(&mut self, idx: usize) {
		let wid = self.windows[idx].window.id();
		self.id_idx.remove(&wid);
		self.windows.remove(idx);
		// Dropping the value should cause the window to close

		if CLOSE_ON_NO_WINDOWS && self.windows.len() == 0 {
			self.shutdown();
		}
	}

	// Due to this aborting the event loop, the game will also be dropped
	pub fn shutdown(&mut self) {
		// Drop all windows
		for i in 0..self.windows.len() {
			self.close_window(i);
		}
		// Shut down event loop
		self.event_loop_sender.send(EventLoopEvent::Close)
			.expect("Failed to send event loop close request");
		// Everything else *should* stop/drop when self is dropped
	}

	/// Updates the windows and input resource
	// Todo: split window updates and input updates?
	pub fn update(&mut self, input_resource: &mut InputResource) {

		// Find duration since last update
		let startt = self.last_update;
		let endt = Instant::now();
		let since_last_update = endt - startt;
		// Only update every 5ms
		if since_last_update < Duration::from_millis(5) {
			return
		}
		// If the previous input was used, remove it
		if input_resource.last_read > input_resource.last_updated {
			input_resource.mdx = 0.0;
			input_resource.mdy = 0.0;
			input_resource.board_keys.clear();
			input_resource.mouse_keys.clear();

			input_resource.dscrollx = 0.0;
			input_resource.dscrolly = 0.0;
		}

		// Keyboard buttons
		// A temporary thing which holds when a key was pressed in this window step
		let mut board_pressedmap = HashMap::new();
		for key in &input_resource.board_presscache {
			board_pressedmap.insert(*key, startt);
		}
		// How long a key was pressed for in this window step
		let mut key_durations = HashMap::new();
		
		// Mouse buttons
		let mut mouse_pressedmap = HashMap::new();
		// Unlike key presses, mouse button presses are not constantly resubmitted
		for button in &input_resource.mouse_presscache {
			mouse_pressedmap.insert(*button, startt);
		}
		let mut mouse_durations = HashMap::new();
		
		// Mouse position
		let mut mx = input_resource.mx;
		let mut my = input_resource.my;
		
		// Mouse movement
		let mut mdx = input_resource.mdx;
		let mut mdy = input_resource.mdy;
		
		// Drain items not passed the start of the step
		let events: Vec<EventWhen> = self.event_queue.lock().unwrap().drain_filter(|e| e.when < endt).collect();
		for event_when in events {
			let ts = event_when.when;
			match event_when.event {
				Event::RedrawRequested(window_id) => {
					let window_idx = self.id_idx[&window_id];
					self.window_redraw_queue.insert(window_idx);
				},
				Event::UserEvent(event) => {
					match event {
						EventLoopEvent::RegisterWindow(window) => {
							self.register_window(window);
						},
						_ => {},
					}
				},
				Event::WindowEvent {event: ref window_event, window_id} => {
					
					// Egui input stuff
					// Egui only uses window events so filtering by window events is fine
					if !self.id_idx.contains_key(&window_id) {
						warn!("found input for old window");
						continue
					}
					let window_idx = self.id_idx[&window_id];
					let window = self.windows.get_mut(window_idx).unwrap();
					window.platform.handle_event(&event_when.event);
					
					// Check if egui wants me to not handle this
					if window.platform.captures_event(&event_when.event) {
						continue
					}

					// My input stuff
					match window_event {
						WindowEvent::KeyboardInput {input, ..} => {
							if let Some(key) = input.virtual_keycode {
								
								// Make sure we are always able to free the cursor
								match key {
									winit::event::VirtualKeyCode::Escape => {
										// let modifiers = ModifiersState::default();
										window.window.set_cursor_grab(false).unwrap();
										window.window.set_cursor_visible(true);
										self.capturing_cursor = false;
									},
									_ => {},
								}

								match input.state {
									ElementState::Pressed => {
										// If the cursor is not inside of this window don't register the input
										if !window.cursor_inside {
											continue
										}
										// If this button was not already pressed, record the pressing
										if !board_pressedmap.contains_key(&key) {
											board_pressedmap.insert(key, ts);
										}
									},
									ElementState::Released => {
										// Only do something if this key had been pressed in the first place
										if board_pressedmap.contains_key(&key) {
											let kp = ts - board_pressedmap[&key];

											// Add press duration to map
											if let Some(key_duration) = key_durations.get_mut(&key) {
												*key_duration += kp;
											} else {
												key_durations.insert(key, kp);
											}
			
											// Remove key from pressed keys
											board_pressedmap.remove(&key);
										}
									},
								}
							}
							else {
								warn!("Key input with no virtual key code");
							}
						},
						WindowEvent::MouseInput {state, button, ..} => {
							let button = *button;
							match state {
								ElementState::Pressed => {
									if !mouse_pressedmap.contains_key(&button) {
										mouse_pressedmap.insert(button, ts);
									}
								},
								ElementState::Released => {
									if mouse_pressedmap.contains_key(&button) {
										let mp = ts - mouse_pressedmap[&button];

										if let Some(mouse_duration) = mouse_durations.get_mut(&button) {
											*mouse_duration += mp;
										} else {
											mouse_durations.insert(button, mp);
										}

										mouse_pressedmap.remove(&button);
									}
								},
							}
						},
						WindowEvent::MouseWheel {delta, ..} => {
							match delta {
								winit::event::MouseScrollDelta::LineDelta(x, y) => {
									input_resource.dscrolly += *y;
									input_resource.dscrollx += *x;
								},
								_ => {},
							}
						},
						WindowEvent::CursorEntered {..} => {
							window.cursor_inside = true;
							window.window.set_cursor_grab(true).unwrap();
							window.window.set_cursor_visible(false);
							self.capturing_cursor = true;
						},
						WindowEvent::CursorLeft {..} => {
							window.cursor_inside = false;
						},
						WindowEvent::CursorMoved {position, ..} => {
							// Don't use this for camera control!
							// This can be used for ui stuff though
							mx = position.x;
							my = position.y;
						},
						WindowEvent::Resized (newsize) => {
							let _ns = newsize;
							// window_resource.windows[
							// 	window_resource.id_idx[&window_id]
							// ].resize(
							// 	&render_resource.renderer.device, 
							// 	newsize.clone(),
							// );
						},
						WindowEvent::CloseRequested => {
							let idx = self.id_idx[&window_id];
							self.close_window(idx);
						},
						_ => {},
					}
				},
				Event::DeviceEvent {event, ..} => {
					match event {
						DeviceEvent::MouseMotion {delta} => {
							if self.capturing_cursor {
								mdx += delta.0;
								mdy += delta.1;
							}
						},
						_ => {},
					}
				},
				_ => {},
			}
		}

		// Process the keys which are still pressed
		for (&key, &t) in &board_pressedmap {
			let kp = endt - t;

			if let Some(key_duration) = key_durations.get_mut(&key) {
				*key_duration += kp;
			} else {
				key_durations.insert(key, kp);
			}
		}
		let board_stillpressed = board_pressedmap.keys().map(|x| x.clone()).collect();
		// Again for mouse keys
		for (&button, &t) in &mouse_pressedmap {
			let mp = endt - t;

			if let Some(mouse_duration) = mouse_durations.get_mut(&button) {
				*mouse_duration += mp;
			} else {
				mouse_durations.insert(button, mp);
			}
		}
		let mouse_stillpressed = mouse_pressedmap.keys().map(|x| x.clone()).collect();

		// Update input resource
		input_resource.board_keys = key_durations;
		input_resource.board_presscache = board_stillpressed;
		input_resource.mouse_keys = mouse_durations;
		input_resource.mouse_presscache = mouse_stillpressed;
		input_resource.mx = mx;
		input_resource.my = my;
		input_resource.mdx = mdx;
		input_resource.mdy = mdy;
		input_resource.last_updated = endt;

		// Record ending time
		self.last_update = endt;
	}
}
