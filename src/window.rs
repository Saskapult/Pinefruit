use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use wgpu;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, Duration};
use egui;
use crate::ecs::*;
use crate::game::Game;
use crate::gui::{GameWidget, MessageWidget};
use generational_arena::{Arena, Index};
use crate::gpu::*;
use egui_wgpu_backend::RenderPass;
use crate::input::*;
use shipyard::*;




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



pub struct GameWindow {
	pub window: Window,
	pub surface: wgpu::Surface,
	pub surface_config: wgpu::SurfaceConfiguration,
	size_changed: bool,
	cursor_inside: bool,
	last_cursor_position: Option<winit::dpi::PhysicalPosition<f64>>,
	cursor_captured: bool,
	focused: bool,
	input: InputFilter,
	last_redraw: Option<Instant>,
	redraw_delay: Duration,

	pub platform: egui_winit_platform::Platform,
	start_time: Instant, // Used for egui animations
	test_texture: Option<egui::TextureHandle>,

	pub game_widget: GameWidget,
	message_widget: MessageWidget,
	game_times: crate::util::DurationHolder,

	pub settings: WindowSettings,
}
impl GameWindow {
	pub fn new(
		instance: &wgpu::Instance, 
		adapter: &wgpu::Adapter, 
		window: Window,
	) -> Self {
		let surface = unsafe { instance.create_surface(&window) };
		GameWindow::new_with_surface(instance, adapter, surface, window)
	}

	pub fn new_with_surface(
		_instance: &wgpu::Instance, 
		adapter: &wgpu::Adapter, 
		surface: wgpu::Surface,
		window: Window,
	) -> Self {
		window.set_title("window title");
		window.set_inner_size(winit::dpi::PhysicalSize {
			width: 1280,
			height: 720,
		});

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
			size_changed: true,
			cursor_inside: false,
			last_cursor_position: None,
			cursor_captured: false,
			focused: true,
			input: InputFilter::new(),
			last_redraw: None,
			redraw_delay: Duration::from_secs_f32(1.0 / 60.0),
			
			platform,
			start_time: Instant::now(),
			test_texture: None,

			game_widget: GameWidget::new(None),
			message_widget: MessageWidget::new(),
			game_times: crate::util::DurationHolder::new(30),

			settings: WindowSettings::new(),
		}
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		self.surface_config.width = width;
		self.surface_config.height = height;
		self.size_changed = true;
		self.last_redraw = None;
	}

	fn encode_ui(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		egui_rpass: &mut RenderPass,
		mut encoder: &mut wgpu::CommandEncoder,
		gpu_data: &mut GpuData,
		destination_view: &wgpu::TextureView,
		world: &shipyard::World,
	) -> egui::TexturesDelta {
		self.platform.update_time(self.start_time.elapsed().as_secs_f64());
		self.platform.begin_frame();
		let ctx = self.platform.context();

		self.game_times.get_things();

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

						ui.label(format!("projected ~{}Hz", (1.0 / self.game_times.average().unwrap_or(Duration::ZERO).as_secs_f32().round())));

						if let Some(entity) = self.game_widget.tracked_entity {
							ui.label(format!("Entity: {entity:?}"));
							let tcs = world.borrow::<View<TransformComponent>>().unwrap();
							if let Ok((tc,)) = (&tcs,).get(entity) {
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
						ui.image(texture, texture.size_vec2());
		
						if ui.button("Send test message!").clicked() {
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
							if let Err(e) = gpu_data.shaders.check_reload() {
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

		let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
			physical_width: self.window.outer_size().width,
			physical_height: self.window.outer_size().height,
			scale_factor: self.window.scale_factor() as f32,
		};
		let tdelta = full_output.textures_delta;
		egui_rpass.add_textures(
			device, queue, &tdelta,
		).expect("Failed to add egui textures!");
		egui_rpass.update_buffers(
			device, 
			queue, 
			&paint_jobs, 
			&screen_descriptor,
		);
		egui_rpass.execute(
			&mut encoder,
			destination_view,
			&paint_jobs,
			&screen_descriptor,
			None,
		).unwrap();

		tdelta
	}

	/// Encodes and executes an update to this window's display.
	pub fn update(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		egui_rpass: &mut RenderPass,
		gpu_data: &mut GpuData,
		world: &shipyard::World,
	) {
		if let Some(t) = self.last_redraw {
			// Increase redraw delay if window not focused
			let redraw_delay = if !self.focused {
				self.redraw_delay * 10
			} else {
				self.redraw_delay
			};
			if t.elapsed() < redraw_delay {
				return;
			}
		}
		self.last_redraw = Some(Instant::now());

		// If size changed then reconfigure
		if self.size_changed {
			self.surface.configure(device, &self.surface_config);
			self.size_changed = false;
		}		

		let frame = match self.surface.get_current_texture() {
			Ok(tex) => tex,
			Err(wgpu::SurfaceError::Outdated) => {
				// Apparently happens when minimized on Windows
				panic!("Render to outdated texture for window!");
			},
			Err(e) => {
				panic!("{}", e);
			},
		};
		let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("a window encoder"),
		});

		// Game?
		let redraw_game = self.game_widget.maybe_encode_render(
			device,
			queue,
			egui_rpass,
			&mut encoder,
			world,
			&*gpu_data,
		);
		
		// Ui
		let tdelta = self.encode_ui(
			device,
			queue,
			egui_rpass,
			&mut encoder,
			gpu_data,
			&frame_view,
			world,
		);

		// Mouse capture
		if self.settings.capture_mouse {
			if !self.cursor_captured {
				println!("Hide cursor");
				self.cursor_captured = true;
				self.window.set_cursor_grab(true).unwrap();
				self.window.set_cursor_visible(false);
			}
		} else {
			if self.cursor_captured {
				println!("Show cursor");
				self.cursor_captured = false;
				self.window.set_cursor_grab(false).unwrap();
				self.window.set_cursor_visible(true);
			}
		}

		// Submit to GPU
		let submit_st = Instant::now();
		queue.submit(std::iter::once(encoder.finish()));
		if redraw_game {
			let gts = self.game_times.sender.clone();
			queue.on_submitted_work_done(move || {
				gts.send(submit_st.elapsed()).unwrap()
			});
		}

		// Show
		frame.present();

		// Egui cleanup
		egui_rpass.remove_textures(tdelta)
			.expect("Failed to remove egui textures!");
	}

	pub fn handle_event(&mut self, event: &Event<WindowCommand>, when: Instant) {

		// Check with Egui
		self.platform.handle_event(&event);
		if self.platform.captures_event(&event) {
			return
		}

		match event {
			Event::WindowEvent { event: window_event, ..} => {
				match window_event {
					&WindowEvent::KeyboardInput { input, .. } => {
						if let Some(key) = input.virtual_keycode {
							let state = input.state.into();
							if !self.cursor_inside && state != KeyState::Released {
								return
							}
							self.input.event((
								InputEvent::KeyEvent((KeyKey::BoardKey(key), state)),
								when,
							));
						} else {
							warn!("Key input with no virtual key code ({input:?})");
						}
					},
					&WindowEvent::MouseInput {
						state, 
						button, 
						..
					} => {
						self.input.event((
							InputEvent::KeyEvent((KeyKey::MouseKey(button), state.into())),
							when,
						));
					},
					WindowEvent::MouseWheel { delta, .. } => {
						match delta {
							&winit::event::MouseScrollDelta::LineDelta(x, y) => {
								self.input.event((
									InputEvent::Scroll([x, y]),
									when,
								));
							},
							_ => warn!("detected strange scrolling hours"),
						}
					},
					WindowEvent::CursorEntered {..} => {
						self.cursor_inside = true;
					},
					WindowEvent::CursorLeft {..} => {
						self.cursor_inside = false;
						// release all keys
						self.input.event((
							InputEvent::ReleaseKeys,
							when,
						));
					},
					&WindowEvent::CursorMoved { position, .. } => {
						self.input.event((
							InputEvent::CursorMoved([position.x, position.y]),
							when,
						));
						// Set cursor position or record new position
						if self.cursor_captured {
							if let Some(p) = self.last_cursor_position {
								self.window.set_cursor_position(p)
									.expect("Failed to set cursor position!");
							}
						} else {
							self.last_cursor_position = Some(position);
						}
					},
					WindowEvent::Resized (newsize) => {
						if newsize.width > 0 && newsize.height > 0 {
							self.resize(newsize.width, newsize.height);
						}
					},
					&WindowEvent::Focused(focused) => {
						self.focused = focused;
					},
					_ => {},
				}
			},
			Event::DeviceEvent { event: device_event, .. } => {
				match device_event {
					&DeviceEvent::MouseMotion { delta: (dx, dy) } => {
						if self.cursor_inside {
							self.input.event((
								InputEvent::MouseMotion([dx, dy]),
								when,
							));
						}
					},
					_ => {},
				}
			},
			_ => {},
		}
	}
}


// A custom event which can be injected into the event loop
#[derive(Debug)]
pub enum WindowCommand {
	Shutdown,
	NewWindow,
}

#[derive(Debug)]
pub enum GameCommand {
	Shutdown,
}



// Manages windows
// Creates/destroys windows and can read their events
const CLOSE_ON_NO_WINDOWS: bool = true;
pub struct WindowManager {
	pub event_loop: Option<EventLoop<WindowCommand>>,
	pub event_loop_proxy: EventLoopProxy<WindowCommand>,

	pub windows: Arena<GameWindow>,
	id_idx: HashMap<WindowId, Index>,

	pub instance: wgpu::Instance,
	pub adapter: wgpu::Adapter,
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
	pub egui_rpass: RenderPass,

	pub game: Game,
}
impl WindowManager {
	pub fn new() -> Self {
		let event_loop = EventLoop::<WindowCommand>::with_user_event();
		let event_loop_proxy = event_loop.create_proxy();
		let window = WindowBuilder::new().build(&event_loop).unwrap();

		let instance = wgpu::Instance::new(wgpu::Backends::all());
		let surface = unsafe { instance.create_surface(&window) };
		let adapter = pollster::block_on(instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::HighPerformance, // Dedicated GPU
				compatible_surface: Some(&surface),
				force_fallback_adapter: false, // Don't use software renderer
			},
		)).unwrap();
		
		let (device, queue) = acquire_device(&adapter, DeviceOptions::Maximum).unwrap();
		let egui_rpass = RenderPass::new(
			&device, 
			surface.get_supported_formats(&adapter)[0],
			1,
		);

		let game = Game::new(
			&device,
			&queue,
			event_loop_proxy.clone(),
		);

		let first_window = GameWindow::new_with_surface(&instance, &adapter, surface, window);
		let mut selfy = Self {
			event_loop: Some(event_loop),
			event_loop_proxy,
			windows: Arena::new(),
			id_idx: HashMap::new(),
			instance,
			adapter,
			device,
			queue,
			egui_rpass,
			game,
		};
		selfy.register_gamewindow(first_window);

		selfy
	}

	pub fn run(mut self) {
		let el = self.event_loop.take().unwrap();
		el.run(move |event, event_loop, control_flow| {
			let when = Instant::now();
			match event {
				Event::MainEventsCleared => {
					if self.game.should_tick() {
						// Submit input to game
						for (_, window) in self.windows.iter_mut() {
							window.input.finish();
							window.game_widget.pre_tick_stuff(&mut self.game, window.input.input_segment.clone());
							window.input.start();
						}
						// Tick game
						self.game.tick();
					}

					for (_, window) in self.windows.iter_mut() {
						window.update(
							&self.device,
							&self.queue,
							&mut self.egui_rpass,
							&mut self.game.gpu_data,
							&self.game.world,
						);

						window.window.request_redraw();
					}
				},
				Event::UserEvent(event) => {
					match event {
						WindowCommand::Shutdown => *control_flow = ControlFlow::Exit,
						WindowCommand::NewWindow => {
							let window = WindowBuilder::new()
								.build(event_loop)
								.unwrap();
							self.register_window(window);
						},
					}
				},
				Event::WindowEvent {event: ref window_event, window_id} => {
					if let Some(window_idx) = self.id_idx.get(&window_id) {
						let window = self.windows.get_mut(*window_idx).unwrap();
						window.handle_event(&event, when);
						
						if window_event == &WindowEvent::CloseRequested {
							self.close_window(*window_idx);
						}
					}					
				},
				Event::DeviceEvent {event: ref device_event, ..} => {
					match device_event {
						DeviceEvent::MouseMotion { .. } => {
							for (_, window) in self.windows.iter_mut() {
								window.handle_event(&event, when);
							}
						},
						_ => {},
					}
				},
				Event::LoopDestroyed => {
					info!("Loop destroy, shutting down");
					self.shutdown();
				},
				_ => {},
			}
		});
	}

	pub fn get_window_index(&self, id: &WindowId) -> Index {
		self.id_idx[id]
	}

	pub fn register_window(&mut self, window: Window) -> Index {
		let gamewindow = GameWindow::new(&self.instance, &self.adapter, window);
		
		let id = gamewindow.window.id();
		let idx = self.windows.insert(gamewindow);
		self.id_idx.insert(id, idx);
		idx
	}

	pub fn register_gamewindow(&mut self, gamewindow: GameWindow) -> Index {
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
		self.event_loop_proxy.send_event(WindowCommand::Shutdown)
			.expect("Failed to send event loop close request");
		// Due to this aborting the event loop, the game should also be dropped
	}
}
