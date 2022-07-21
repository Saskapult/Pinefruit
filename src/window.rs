use specs::Entity;
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
use crate::gui::{GameWidget, MessageWidget};
use crate::render::*;




pub struct GameWindow {
	pub window: Window,
	pub surface: wgpu::Surface,
	pub surface_config: wgpu::SurfaceConfiguration,
	pub platform: egui_winit_platform::Platform,
	pub start_time: Instant,
	pub cursor_inside: bool,
	pub game_draw_rate: Duration,
	pub last_game_draw: Instant,
	pub last_update: Instant,
	size_changed: bool,

	test_texture: Option<egui::TextureHandle>,

	pub game_widget: GameWidget,
	game_render_texture: Option<BoundTexture>,
	sampy: Option<wgpu::Sampler>,
	fug_buffer: Option<wgpu::Buffer>,

	message_widget: MessageWidget,

	game_times: crate::util::DurationHolder,
}
impl GameWindow {
	pub fn new(
		instance: &wgpu::Instance, 
		adapter: &wgpu::Adapter, 
		window: Window,
	) -> Self {
		let surface = unsafe { instance.create_surface(&window) };
		let size = window.inner_size();
		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
			format: surface.get_supported_formats(&adapter)[0],
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
							rounding: egui::Rounding::none(),
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
			start_time: Instant::now(),
			cursor_inside: false,
			game_draw_rate: Duration::from_millis(10),
			last_game_draw: Instant::now(),
			last_update: Instant::now(),
			size_changed: true,

			test_texture: None,

			game_widget: GameWidget::new(None),
			game_render_texture: None,
			sampy: None,
			fug_buffer: None,

			message_widget: MessageWidget::new(),

			game_times: crate::util::DurationHolder::new(30),
		}
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		self.surface_config.width = width;
		self.surface_config.height = height;
		self.size_changed = true;
	}

	fn encode_ui(
		&mut self,
		mut encoder: &mut wgpu::CommandEncoder,
		gpu_resource: &mut GPUResource,
		destination_view: &wgpu::TextureView,
		_world: &specs::World,
	) -> egui::TexturesDelta {

		self.platform.update_time(self.start_time.elapsed().as_secs_f64());
		self.platform.begin_frame();
		let ctx = self.platform.context();

		egui::CentralPanel::default().show(&ctx, |ui| {
			egui::SidePanel::left("left panel")
				.resizable(true)
				.default_width(200.0)
				.show_inside(ui, |ui| {
					ui.vertical(|ui| {
						ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);

						ui.label(format!("~{}gf/s", (1.0 / self.game_times.average().unwrap_or(Duration::ZERO).as_secs_f32().round())));
		
						let texture: &egui::TextureHandle = self.test_texture.get_or_insert_with(|| {
							// Load the texture only once.
							ui.ctx().load_texture("my-image", egui::ColorImage::example())
						});
		
						ui.label("TESTING TESTING TESTING");
						ui.image(texture, texture.size_vec2());
						if ui.button("click me!").clicked() {
							println!("Hey!");
							self.message_widget.add_message("Hey!".to_string(), Instant::now() + Duration::from_secs_f32(5.0));
						}

						for i in 0..14 {
							ui.label(format!("{i}"));
						}
					});
				});
			egui::SidePanel::right("right panel")
				.resizable(true)
				.default_width(200.0)
				.show_inside(ui, |ui| {
					ui.vertical(|ui| {
						ui.label("Right panel");

						self.message_widget.display(ui);

						if ui.button("Refresh shaders").clicked() {
							if let Err(e) = gpu_resource.data.shaders.check_reload() {
								error!("Error in refresh shaders: {e:?}");
								self.message_widget.add_message(e.to_string(), Instant::now() + Duration::from_secs_f32(5.0));
							}
						}
					});
				});
			egui::CentralPanel::default()
				.show_inside(ui, |ui| {
					ui.vertical_centered_justified(|ui| {
						self.game_widget.display(ui);
					})
				});
		});
		
		let full_output = self.platform.end_frame(Some(&self.window));
		let paint_jobs = self.platform.context().tessellate(full_output.shapes);

		// GPU uploads
		let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
			physical_width: self.window.outer_size().width,
			physical_height: self.window.outer_size().height,
			scale_factor: self.window.scale_factor() as f32,
		};
		let tdelta = full_output.textures_delta;
		gpu_resource.egui_rpass.add_textures(
			&gpu_resource.device, &gpu_resource.queue, &tdelta,
		).expect("Failed to add egui textures!");
		gpu_resource.egui_rpass.update_buffers(
			&gpu_resource.device, 
			&gpu_resource.queue, 
			&paint_jobs, 
			&screen_descriptor,
		);

		// GPU executions
		gpu_resource.egui_rpass.execute(
			&mut encoder,
			destination_view,
			&paint_jobs,
			&screen_descriptor,
			None,
		).unwrap();

		tdelta
	}

	/// Encodes and executes an update to this window's display.
	/// UI should be redrawn if it is dirty.
	/// Game should be readrawn if the window's frame is dirty.
	/// 
	/// Game frames should be drawn as an element of the GUI.
	pub fn update(
		&mut self,
		gpu_resource: &mut GPUResource,
		world: &specs::World,
	) {
		// Do nothing if it is not time to do things
		let now = Instant::now();
		if now - self.last_update < Duration::from_millis(10) {
			return
		}
		self.last_update = now;

		// Decide if game texture must be redrawn
		let redraw_game = Instant::now() - self.last_game_draw >= self.game_draw_rate;
		if redraw_game {
			self.last_game_draw = Instant::now();
		}

		// If size changed then reconfigure
		if self.size_changed {
			self.surface.configure(&gpu_resource.device, &self.surface_config);
			self.size_changed = false;
		}		

		let frame = match self.surface.get_current_texture() {
			Ok(tex) => tex,
			Err(wgpu::SurfaceError::Outdated) => {
				// Apparently happens when minimized on Windows
				panic!("Render to outdated texture for window");
			},
			Err(e) => {
				panic!("{}", e);
			},
		};
		let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = gpu_resource.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("Window Encoder"),
		});

		if redraw_game {		
			self.game_widget.encode_render(
				&mut encoder,
				world,
				gpu_resource,
			);
		}
		
		// Ui
		let tdelta = self.encode_ui(
			&mut encoder,
			gpu_resource,
			&frame_view,
			world,
		);

		// Submit
		let submit_st = Instant::now();
		gpu_resource.queue.submit(std::iter::once(encoder.finish()));
		let submit_en = Instant::now() - submit_st;

		if redraw_game {
			self.game_times.record(submit_en);
		}

		// Show
		frame.present();

		// More egui stuff
		gpu_resource.egui_rpass.remove_textures(tdelta)
			.expect("Failed to remove egui textures!");
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
							.with_inner_size(winit::dpi::PhysicalSize {
								width: 1280,
								height: 720,
							})
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
							// Sometimes it is not fast enough and the cursor escapes
							// This makes an error
							// Maybe add a timer and only capture after 5ms of in window time
							// Otherwise just be faster?
							window.cursor_inside = true;
							// window.window.set_cursor_grab(true).unwrap();
							// window.window.set_cursor_visible(false);
							self.capturing_cursor = true;
						},
						WindowEvent::CursorLeft {..} => {
							window.cursor_inside = false;
							// self.capturing_cursor = false;
						},
						WindowEvent::CursorMoved {position, ..} => {
							// Don't use this for camera control!
							// This can be used for ui stuff though
							mx = position.x;
							my = position.y;
						},
						WindowEvent::Resized (newsize) => {
							if newsize.width > 0 && newsize.height > 0 {
								let win = self.windows.get_mut(self.id_idx[&window_id]).unwrap();
								win.resize(newsize.width, newsize.height);
							}
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
