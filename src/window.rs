use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use wgpu;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, Receiver};
use std::time::{Instant, Duration};
use egui;
use crate::ecs::*;
use crate::gui::{GameWidget, MessageWidget};
use generational_arena::{Arena, Index};




/// Window settings designed to be modified by gui code
#[derive(Debug)]
pub struct WindowSettings {
	pub capture_mouse: bool,
}
impl WindowSettings {
	pub fn new() -> Self {
		Self {
			capture_mouse: false,
		}
	}
}



// Maps can be replaced with arrays of all possible values?
#[derive(Debug)]
pub struct WindowInput {
	pub board_keys: HashMap<VirtualKeyCode, Duration>, // Duration pressed since last read
	pub board_pressed: HashMap<VirtualKeyCode, Instant>, // What is pressed and since when
	pub mouse_keys: HashMap<MouseButton, Duration>,
	pub mouse_pressed: HashMap<MouseButton, Instant>,
	pub cursor_inside: bool, // For tracking if cursor movement should be registered
	pub mx: f64,
	pub my: f64,
	pub mdx: f64,
	pub mdy: f64,
	pub dscrollx: f32,
	pub dscrolly: f32,
	pub last_read: Instant, // When was this data last used
	pub last_feed: Instant,	// When was new data last fed in
}
impl WindowInput {
	pub fn new() -> Self {
		Self {
			board_keys: HashMap::new(),
			board_pressed: HashMap::new(),
			mouse_keys: HashMap::new(),
			mouse_pressed: HashMap::new(),
			cursor_inside: false,
			mx: 0.0,
			my: 0.0,
			mdx: 0.0, 
			mdy: 0.0,
			dscrollx: 0.0,
			dscrolly: 0.0,
			last_read: Instant::now(),
			last_feed: Instant::now(),
		}
	}

	/// Collects all key durations without unpressing keys
	pub fn end(&mut self) -> Duration {
		let now = self.last_feed;

		self.board_pressed.iter_mut().for_each(|(key, pressed)| {
			let kp = now - *pressed;
			*pressed = now;
			if let Some(dur) = self.board_keys.get_mut(key) {
				*dur += kp;
			} else {
				self.board_keys.insert(*key, kp);
			}
		});

		self.mouse_pressed.iter_mut().for_each(|(key, pressed)| {
			let kp = now - *pressed;
			*pressed = now;
			if let Some(dur) = self.mouse_keys.get_mut(key) {
				*dur += kp;
			} else {
				self.mouse_keys.insert(*key, kp);
			}
		});

		let dur = self.last_feed - self.last_read;
		self.last_read = Instant::now();

		dur
	}

	/// Clears duration (and other) data but not pressed keys
	pub fn reset(&mut self) {
		self.board_keys.clear();
		self.mouse_keys.clear();

		self.mdx = 0.0;
		self.mdy = 0.0;

		self.dscrollx = 0.0;
		self.dscrolly = 0.0;
	}

	/// Collects the input from other into self.
	/// end() must be called on other before it is sent here.
	pub fn apply(&mut self, other: &Self) {
		self.board_pressed = other.board_pressed.clone();
		other.board_keys.iter().for_each(|(k, &kd)| {
			if let Some(d) = self.board_keys.get_mut(k) {
				*d += kd;
			} else {
				self.board_keys.insert(*k, kd);
			}
		});

		self.mouse_pressed = other.mouse_pressed.clone();
		other.mouse_keys.iter().for_each(|(k, &kd)| {
			if let Some(d) = self.mouse_keys.get_mut(k) {
				*d += kd;
			} else {
				self.mouse_keys.insert(*k, kd);
			}
		});

		self.mdx += other.mdx;
		self.mdy += other.mdy;

		self.mx = other.mx;
		self.my = other.my;

		self.dscrollx += other.dscrollx;
		self.dscrolly += other.dscrolly;

		self.last_feed = other.last_feed;
	}
}



pub struct GameWindow {
	pub window: Window,
	pub surface: wgpu::Surface,
	pub surface_config: wgpu::SurfaceConfiguration,
	pub platform: egui_winit_platform::Platform,
	
	start_time: Instant, // Used for egui animations

	pub game_draw_rate: Duration,
	pub last_game_draw: Instant,
	size_changed: bool,

	test_texture: Option<egui::TextureHandle>,

	pub game_widget: GameWidget,

	message_widget: MessageWidget,

	game_times: crate::util::DurationHolder,

	pub redraw: bool,
	last_redraw: Instant,

	pub settings: WindowSettings,
	// UI input is fed into game input and then reset every time the ui is updated
	// It can be used for ui things and also maybe moving the camera in the render world
	// Game input is consumed by the simulation 
	pub ui_input: WindowInput,
	pub game_input: WindowInput,
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

		window.set_title("kkraft time");
		window.set_window_icon(None);

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
			game_draw_rate: Duration::from_millis(10),
			last_game_draw: Instant::now(),
			size_changed: true,

			test_texture: None,

			game_widget: GameWidget::new(None),
			message_widget: MessageWidget::new(),

			game_times: crate::util::DurationHolder::new(30),

			redraw: true,
			last_redraw: Instant::now(),

			settings: WindowSettings::new(),
			ui_input: WindowInput::new(),
			game_input: WindowInput::new(),
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
		world: &specs::World,
	) -> egui::TexturesDelta {

		use specs::WorldExt;

		self.ui_input.end();
		self.game_input.apply(&self.ui_input);

		self.platform.update_time(self.start_time.elapsed().as_secs_f64());
		self.platform.begin_frame();
		let ctx = self.platform.context();

		egui::CentralPanel::default().show(&ctx, |ui| {
			egui::Frame::none()
				.fill(egui::Color32::DARK_GRAY)
				.outer_margin(egui::style::Margin::same(0.0))
				.show(ui, |ui| {
			egui::SidePanel::left("left panel")
				.resizable(false)
				.default_width(200.0)
				.max_width(200.0)
				.min_width(200.0)
				.show_inside(ui, |ui| {
					ui.vertical(|ui| {
						ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);

						ui.label(format!("~{}gf/s", (1.0 / self.game_times.average().unwrap_or(Duration::ZERO).as_secs_f32().round())));

						if let Some(entity) = self.game_widget.tracked_entity {
							ui.label(format!("Entity: {entity:?}"));
							let tcs = world.read_component::<TransformComponent>();
							if let Some(tc) = tcs.get(entity) {
								ui.label(format!("Position: [{:.1}, {:.1}, {:.1}]", tc.position[0], tc.position[1], tc.position[2]));
								let (r, p, y) = tc.rotation.euler_angles();
								ui.label(format!("Rotation: [{:.1}, {:.1}, {:.1}]", r, p, y));
								
								let c = tc.position / 16.0;
								ui.label(format!("Chunk: [{}, {}, {}]", c[0].floor() as i32, c[1].floor() as i32, c[2].floor() as i32));
							}
						} else {
							ui.label("Tracked entity not set!");
						}
		
						let texture: &egui::TextureHandle = self.test_texture.get_or_insert_with(|| {
							// Load the texture only once.
							ui.ctx().load_texture("my-image", egui::ColorImage::example())
						});
		
						ui.label("TESTING TESTING TESTING");
						let g = ui.image(texture, texture.size_vec2());
						let f = g.interact(egui::Sense::click());
						if f.clicked() {
							println!("Testyy");
						}
						if ui.button("click me!").clicked() {
							println!("Hey!");
							self.message_widget.add_message("Hey!".to_string(), Instant::now() + Duration::from_secs_f32(5.0));
						}

					});
				});
			egui::SidePanel::right("right panel")
				.resizable(false)
				.default_width(200.0)
				.max_width(200.0)
				.min_width(200.0)
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
						self.game_widget.display(ui, &mut self.settings);
					})
				});
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

		self.ui_input.reset();

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
		self.redraw = self.last_redraw.elapsed() >= Duration::from_secs_f32(1.0 / 60.0);
		// Only redraw if a redraw was requested
		if !self.redraw {
			return
		}
		self.redraw = false;
		self.last_redraw = Instant::now();

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

		if self.settings.capture_mouse {
			self.window.set_cursor_grab(true).unwrap();
			self.window.set_cursor_visible(false);
		} else {
			self.window.set_cursor_grab(false).unwrap();
			self.window.set_cursor_visible(true);
		}

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

	pub fn handle_event(&mut self, event: &Event<EventLoopEvent>, when: Instant) {

		// Check with Egui
		self.platform.handle_event(&event);
		if self.platform.captures_event(&event) {
			return
		}

		match event {
			Event::WindowEvent { event: window_event, ..} => {
				match window_event {
					&WindowEvent::KeyboardInput { 
						input, 
						.. 
					} => {
						if let Some(key) = input.virtual_keycode {
							match input.state {
								ElementState::Pressed => {
									// If the cursor is not inside don't register the input
									if !self.ui_input.cursor_inside {
										return
									}
									// If this button was not already pressed, record the pressing
									if !self.ui_input.board_pressed.contains_key(&key) {
										self.ui_input.board_pressed.insert(key, when);
									}
								},
								ElementState::Released => {
									if self.ui_input.board_pressed.contains_key(&key) {
										let kp = when - self.ui_input.board_pressed[&key];
	
										if let Some(key_duration) = self.ui_input.board_keys.get_mut(&key) {
											*key_duration += kp;
										} else {
											self.ui_input.board_keys.insert(key, kp);
										}
		
										self.ui_input.board_pressed.remove(&key);
									}
								},
							}
						} else {
							warn!("Key input with no virtual key code ({input:?})");
						}
					},
					&WindowEvent::MouseInput {
						state, 
						button, 
						..
					} => {
						match state {
							ElementState::Pressed => {
								if !self.ui_input.mouse_pressed.contains_key(&button) {
									self.ui_input.mouse_pressed.insert(button, when);
								}
							},
							ElementState::Released => {
								if self.ui_input.mouse_pressed.contains_key(&button) {
									let mp = when - self.ui_input.mouse_pressed[&button];
	
									if let Some(mouse_duration) = self.ui_input.mouse_keys.get_mut(&button) {
										*mouse_duration += mp;
									} else {
										self.ui_input.mouse_keys.insert(button, mp);
									}
	
									self.ui_input.mouse_pressed.remove(&button);
								}
							},
						}
					},
					WindowEvent::MouseWheel {delta, ..} => {
						match delta {
							&winit::event::MouseScrollDelta::LineDelta(x, y) => {
								self.ui_input.dscrolly += y;
								self.ui_input.dscrollx += x;
							},
							_ => {},
						}
					},
					WindowEvent::CursorEntered {..} => {
						self.ui_input.cursor_inside = true;
					},
					WindowEvent::CursorLeft {..} => {
						self.ui_input.cursor_inside = false;
						// release all keys
						self.ui_input.board_pressed.drain().for_each(|(key, pressed)| {
							let kp = when - pressed;
							if let Some(dur) = self.ui_input.board_keys.get_mut(&key) {
								*dur += kp;
							} else {
								self.ui_input.board_keys.insert(key, kp);
							}
						});
					},
					WindowEvent::CursorMoved {position, ..} => {
						// Don't use this for camera control!
						// This can be used for ui stuff though
						self.ui_input.mx = position.x;
						self.ui_input.my = position.y;
					},
					WindowEvent::Resized (newsize) => {
						if newsize.width > 0 && newsize.height > 0 {
							self.resize(newsize.width, newsize.height);
						}
					},
					_ => {},
				}
			},
			Event::DeviceEvent { event: device_event, .. } => {
				match device_event {
					&DeviceEvent::MouseMotion {delta} => {
						if self.ui_input.cursor_inside {
							self.ui_input.mdx += delta.0;
							self.ui_input.mdy += delta.1;
						}
					},
					_ => {},
				}
			},
			_ => {},
		}

		self.ui_input.last_feed = Instant::now();
	}
}


// A custom event which can be injected into the event loop
#[derive(Debug)]
pub enum EventLoopEvent {
	Shutdown,
	NewWindow,
}

#[derive(Debug)]
pub enum ResponseFeed {
	Shutdown,
	Event((Event<'static, EventLoopEvent>, Instant)),
	Window(Window),
}


pub fn run_event_loop(
	event_loop: EventLoop<EventLoopEvent>, 
	sender: Sender<ResponseFeed>,
) {
	event_loop.run(move |event, event_loop, control_flow| {
		match event {
			Event::UserEvent(event) => {
				match event {
					EventLoopEvent::Shutdown => *control_flow = ControlFlow::Exit,
					EventLoopEvent::NewWindow => {
						let window = WindowBuilder::new()
							.with_title("window title")
							.with_inner_size(winit::dpi::PhysicalSize {
								width: 1280,
								height: 720,
							})
							.build(event_loop)
							.unwrap();
						sender.send(ResponseFeed::Window(window)).unwrap();
					},
					_ => {},
				}
			},
			_ => {
				// Not a memory leak because 'static implies that it *can* live forever, not that it does live forever
				if let Some(event) = event.to_static() {
					sender.send(ResponseFeed::Event((event, Instant::now()))).unwrap();
				}
			},
		}
	});
	
}



// Manages windows
// Creates/destroys windows and can read their events
const CLOSE_ON_NO_WINDOWS: bool = true;
pub struct WindowManager {
	pub windows: Arena<GameWindow>,
	id_idx: HashMap<WindowId, Index>,

	instance: wgpu::Instance,
	adapter: wgpu::Adapter,

	event_loop_receiver: Receiver<ResponseFeed>,
	event_loop_proxy: EventLoopProxy<EventLoopEvent>,

	pub capturing_cursor: bool,
	last_update: Instant,
}
impl WindowManager {
	pub fn new(
		instance: wgpu::Instance,
		adapter: wgpu::Adapter,
		event_loop_proxy: EventLoopProxy<EventLoopEvent>,
		event_loop_receiver: Receiver<ResponseFeed>,
	) -> Self {

		Self {
			windows: Arena::new(),
			id_idx: HashMap::new(),
			instance,
			adapter,
			event_loop_proxy,
			event_loop_receiver,
			capturing_cursor: false,
			last_update: Instant::now(),
		}
	}

	pub fn get_window_index(&self, id: &WindowId) -> Index {
		self.id_idx[id]
	}

	pub fn request_new_window(&mut self) {
		self.event_loop_proxy.send_event(EventLoopEvent::NewWindow)
			.expect("Failed to send window creation request!");
	}

	pub fn register_window(&mut self, window: Window) -> Index {
		let gamewindow = GameWindow::new(&self.instance, &self.adapter, window);
		
		let id = gamewindow.window.id();
		let idx = self.windows.insert(gamewindow);
		self.id_idx.insert(id, idx);
		idx
	}

	pub fn close_window(&mut self, idx: Index) {
		let wid = self.windows[idx].window.id();
		self.id_idx.remove(&wid);
		self.windows.remove(idx);
		// Dropping the value should cause the window to close

		if CLOSE_ON_NO_WINDOWS && self.windows.len() == 0 {
			self.shutdown();
		}
	}

	pub fn shutdown(&mut self) {
		// Drop all windows
		let indices = self.windows.iter().map(|(i, _)| i).collect::<Vec<_>>();
		for i in indices {
			let wid = self.windows[i].window.id();
			self.id_idx.remove(&wid);
			self.windows.remove(i);
		}
		// Shut down event loop
		self.event_loop_proxy.send_event(EventLoopEvent::Shutdown)
			.expect("Failed to send event loop close request");
		// Due to this aborting the event loop, the game should also be dropped
	}

	pub fn read_input(&mut self) {

		// Limit update rate
		if self.last_update.elapsed() < Duration::from_millis(1) {
			return
		}

		let responses = self.event_loop_receiver.try_iter().collect::<Vec<_>>();
		for response in responses {
			match response {
				ResponseFeed::Shutdown => {
					self.shutdown();
				},
				ResponseFeed::Window(window) => {
					self.register_window(window);
				},
				ResponseFeed::Event((ref event, when)) => {
					match event {
						Event::RedrawRequested(window_id) => {
							let window_idx = self.id_idx[&window_id];
							self.windows[window_idx].redraw = true;
						},
						Event::WindowEvent {event: window_event, window_id} => {
							if !self.id_idx.contains_key(&window_id) {
								warn!("found input for old window");
								continue
							}
		
							let window_idx = self.id_idx[&window_id];
							let window = self.windows.get_mut(window_idx).unwrap();
							window.handle_event(event, when);
							
							match window_event {
								WindowEvent::CloseRequested => {
									let idx = self.id_idx[&window_id];
									self.close_window(idx);
								},
								_ => {},
							}
						},
						Event::DeviceEvent {event: device_event, ..} => {
							match device_event {
								DeviceEvent::MouseMotion { .. } => {
									for (_, window) in self.windows.iter_mut() {
										window.handle_event(event, when);
									}
								},
								_ => {},
							}
						},
						Event::LoopDestroyed => {
							info!("Loop destroy, shutting down");
							self.shutdown();
							return
						}
						_ => {},
					}
				},
			}
		}

		self.last_update = Instant::now();
	}
}
