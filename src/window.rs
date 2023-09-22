use crossbeam_channel::Sender;
use egui::Context;
use egui_wgpu::renderer::ScreenDescriptor;
use egui_wgpu::{Renderer, preferred_framebuffer_format};
use parking_lot::{Mutex, MutexGuard};
// use shipyard::*;
use eks::prelude::*;
use slotmap::SlotMap;
use wgpu_profiler::GpuProfiler;
use winit::dpi::{PhysicalSize, PhysicalPosition};
use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use wgpu;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Instant, Duration};
use crate::ecs::{TransformComponent, SSAOComponent};
use crate::ecs::loading::ChunkLoadingResource;
use crate::ecs::octree::GPUChunksResource;
use crate::game::{Game, ContextResource, GameStatus};
use crate::gui::{GameWidget, MessageWidget, RenderProfilingWidget, SplineWidget, MapLoadingWidget, SSAOWidget};
use crate::input::*;
use crate::util::RingDataHolder;



/// Window settings (things you can modify)
#[derive(Debug)]
pub struct WindowSettings {
	pub cursor_captured: bool, 
}
impl WindowSettings {
	pub fn new() -> Self {
		Self {
			cursor_captured: false,
		}
	}
}


/// Window properties (stuff that's decided by external forces)
#[derive(Debug)]
pub struct WindowProperties {
	pub cursor_inside: bool,
	pub focused: bool,
}
impl WindowProperties {
	pub fn new() -> Self {
		Self {
			cursor_inside: false,
			focused: true,
		}
	}
}

/// Something passed to the egui widgets.
/// Allows for reading of properties and modification of settings.
/// I find myself wishing that egui would do this for me.
#[derive(Debug)]
pub struct WindowPropertiesAndSettings<'a> {
	window: &'a winit::window::Window,
	pub properties: &'a WindowProperties,
	settings: &'a mut WindowSettings,
}
impl<'a> WindowPropertiesAndSettings<'a> {
	pub fn set_cursor_grab(&mut self, grab: bool) {
		if grab {
			self.window.set_cursor_visible(false);
			// self.window.set_cursor_grab(winit::window::CursorGrabMode::Locked).unwrap();
		} else {
			self.window.set_cursor_visible(true);
			// self.window.set_cursor_grab(winit::window::CursorGrabMode::None).unwrap();
		}
		self.settings.cursor_captured = grab;
	}
}


pub struct GameWindow {
	pub window: winit::window::Window,
	
	surface: WindowSurface,
	
	properties: WindowProperties,
	settings: WindowSettings,

	last_update: Option<Instant>,
	update_delay: Duration, // Can have another for unfocused delay
	update_times: RingDataHolder<Duration>,

	context: egui::Context,
	state: egui_winit::State,
	egui_test_texture: Option<egui::TextureHandle>,

	pub game_widget: GameWidget,
	message_widget: MessageWidget,
	
	profiling_widget: RenderProfilingWidget,

	spline_widget: SplineWidget,

	show_profiler: bool,

	// Winit doesn't support locking the cursor on x11, only confining it
	// We need to do this manually (brings needless mess)
	manual_cursor_lock_last_position: Option<PhysicalPosition<f64>>,
}
impl GameWindow {
	pub fn new(
		instance: &wgpu::Instance, 
		adapter: &wgpu::Adapter, 
		window_builder: WindowBuilder,
		event_loop: &EventLoopWindowTarget::<WindowCommand>,
	) -> Self {
		let window = window_builder.build(event_loop).unwrap();
		let surface = unsafe { 
			instance.create_surface(&window) 
		}.unwrap();
		Self::new_from_window_surface(adapter, window, surface)
	}

	// Used for startup window because of contstruction order
	pub fn new_from_window_surface(
		adapter: &wgpu::Adapter, 
		window: winit::window::Window,
		surface: wgpu::Surface,
	) -> Self {
		let surface = WindowSurface::new(adapter, &window, surface, 1);
		let state = egui_winit::State::new(&window);
		Self {
			window,
			surface,

			properties: WindowProperties::new(),
			settings: WindowSettings::new(),
			
			last_update: None,
			update_delay: Duration::from_secs_f32(1.0 / 60.0),
			update_times: RingDataHolder::new(30),
			
			context: Context::default(),
			state,
			egui_test_texture: None,

			game_widget: GameWidget::new(),
			message_widget: MessageWidget::new(),
			profiling_widget: RenderProfilingWidget::new(),

			spline_widget: SplineWidget::new(64),

			show_profiler: false,

			manual_cursor_lock_last_position: None,
		}
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		self.surface.set_size([width, height]);
		self.last_update = None;
	}

	#[profiling::function]
	fn update_ui(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		profiler: &mut GpuProfiler,
		renderer: &mut Renderer,
		game: &mut Game,
	) -> (wgpu::SurfaceTexture, wgpu::CommandBuffer) {
		// info!("Begin UI frame");
		self.context.begin_frame(self.state.take_egui_input(&self.window));

		let mut setting_props = WindowPropertiesAndSettings {
			window: &mut self.window,
			settings: &mut self.settings,
			properties: &self.properties
		};
		
		if self.show_profiler {
			self.show_profiler = puffin_egui::profiler_window(&self.context);
		}
		egui::SidePanel::left("left panel")
			.resizable(false)
			.default_width(220.0)
			.max_width(220.0)
			.min_width(220.0)
			.show(&self.context, |ui| {
				ui.vertical(|ui| {
					ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);

					// Tracked entity information
					let contexts = game.world.borrow::<Res<ContextResource>>();
					if let Some(entity) = self.game_widget.context(&*contexts).and_then(|c| c.entity) {
						ui.label(format!("Entity: {entity:?}"));
						
						// Transform component information
						let tcs = game.world.borrow::<Comp<TransformComponent>>();
						if let Some((tc,)) = (&tcs,).storage_get(entity) {
							// Position
							let [px, py, pz] = tc.translation.to_array();
							ui.label(format!("Position: [{:.1}, {:.1}, {:.1}]", px, py, pz));

							// Rotation
							let (ex, ey, ez) = tc.rotation.to_euler(glam::EulerRot::XYZ);
							ui.label(format!("Rotation: [{:.1}, {:.1}, {:.1}]", ex, ey, ez));
							
							// Current chunk
							let [cx, cy, cz] = (tc.translation / 16.0).floor().as_ivec3().to_array();
							ui.label(format!("Chunk: [{}, {}, {}]", cx, cy, cz));
						}
					} else {
						ui.label("Tracked entity not set!");
					}
	
					// Test texture
					let texture: &egui::TextureHandle = self.egui_test_texture.get_or_insert_with(|| {
						// Load the texture only once.
						ui.ctx().load_texture("my-image", egui::ColorImage::example(), egui::TextureOptions::default())
					});
					ui.image(texture, texture.size_vec2());

					// Toggle profiler
					ui.toggle_value(&mut self.show_profiler, "Profiler");

					// Update rate for the UI
					let ui_update_rate = self.update_times.iter()
						.map(|d| d.as_secs_f32())
						.reduce(|a, v| a + v)
						.unwrap_or(f32::INFINITY) / (self.update_times.len() as f32);
					ui.label(format!("UI: {:>4.1}ms, {:.0}Hz", ui_update_rate * 1000.0, (1.0 / ui_update_rate).round()));

					// Update rate for the Game Widget
					let gw_update_rate = self.game_widget.update_times.iter()
						.map(|d| d.as_secs_f32())
						.reduce(|a, v| a + v)
						.unwrap_or(f32::INFINITY) / (self.game_widget.update_times.len() as f32);
					ui.label(format!("GW: {:>4.1}ms, {:.0}Hz", gw_update_rate * 1000.0, (1.0 / gw_update_rate).round()));

					// Update time (and more info) for the submitted gpu work
					if let Some(profile_data) = profiler.process_finished_frame() {
						self.profiling_widget.display(ui, &profile_data)
					}

					// Shows how much gpu memory is used by the octree chunks
					let g = game.world.borrow::<Res<GPUChunksResource>>().used_bytes();
					ui.label(format!("GPU chunks: {}kb", g as f32 / 1000.0));

					// Shows what chunks are being loaded
					let loading = game.world.borrow::<Res<ChunkLoadingResource>>();
					MapLoadingWidget::display(ui, &loading);
				});
			});
		egui::SidePanel::right("right panel")
			// .resizable(false)
			// .default_width(220.0)
			// .max_width(220.0)
			// .min_width(220.0)
			.show(&self.context, |ui| {
				ui.vertical(|ui| {
					// Message widget
					self.message_widget.display(ui);

					// Shader refresh button
					if ui.button("Refresh shaders").clicked() {
						self.message_widget.add_message("Todo: re-add shader reloading", Instant::now() + Duration::from_secs_f32(5.0));
					}

					let contexts = game.world.borrow::<Res<ContextResource>>();
					let mut ssaos = game.world.borrow::<CompMut<SSAOComponent>>();
					if let Some(ssao) = self.game_widget.context(&*contexts).and_then(|c| c.entity).and_then(|entity| ssaos.get_mut(entity)) {
						SSAOWidget::display(ui, ssao);
					}
				});
			});
		egui::CentralPanel::default()
			.show(&self.context, |ui| {
				ui.vertical_centered_justified(|ui| {
					// Game widget
					self.game_widget.display(ui, &mut setting_props);
				})
			});
		// egui::Window::new("Spline Editor")
		// 	.show(&self.context, |ui| {
		// 		self.spline_widget.display(ui);
		// 	});
		
		let full_output = self.context.end_frame();
		self.state.handle_platform_output(&self.window, &self.context, full_output.platform_output);
		let textures_delta = full_output.textures_delta;
		let paint_jobs = self.context.tessellate(full_output.shapes);

		let screen_descriptor = ScreenDescriptor {
			size_in_pixels: self.window.inner_size().into(),
			pixels_per_point: self.state.pixels_per_point(),
		};
		self.surface.set_size(self.window.inner_size().into());

		// trace!("Create encoder");
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("a window encoder"),
		});
		profiler.begin_scope(&*format!("window '{}' ({:?}) egui", self.window.title(), self.window.id()), &mut encoder, device);

		// trace!("Update textures");
		for (id, image_delta) in textures_delta.set {
			renderer.update_texture(device, queue, id, &image_delta);
		}

		// trace!("Update buffers");
		let user_buffers = renderer.update_buffers(device, queue, &mut encoder, paint_jobs.as_slice(), &screen_descriptor);
		assert_eq!(0, user_buffers.len(), "there shouldn't have been any user-defined command buffers, yet there were user-defined command buffers!");
		
		// trace!("Get frame");
		let (surface, frame) = self.surface.frame(device);
		{
			// trace!("Get render pass");
			let mut egui_render_pass = frame.renderpass(&mut encoder);
			// trace!("Render");
			renderer.render(&mut egui_render_pass, paint_jobs.as_slice(), &screen_descriptor);
		}

		// trace!("Free textures_delta.free");
		for id in textures_delta.free.iter() {
			renderer.free_texture(id);
		}

		// trace!("Finish encoder");
		profiler.end_scope(&mut encoder);

		(surface, encoder.finish())
	}

	pub fn should_update(&self) -> bool {
		self.last_update.is_none() || self.last_update.unwrap().elapsed() >= self.update_delay
	}

	/// Encodes and executes an update to this window's display.
	#[profiling::function]
	pub fn update(
		&mut self,
		graphics: &mut GraphicsHandle,
		game: &mut Game,
	) -> (wgpu::SurfaceTexture, wgpu::CommandBuffer, Option<wgpu::CommandBuffer>) {
		if let Some(t) = self.last_update {
			self.update_times.insert(t.elapsed());
		}
		self.last_update = Some(Instant::now());

		// Game widget
		let render = self.game_widget.should_update().then(|| self.game_widget.update(graphics, game));

		// Ui
		let (surface, ui) = self.update_ui(
			&graphics.device,
			&graphics.queue,
			&mut graphics.profiler,
			&mut graphics.egui_renderer,
			game,
		);
		
		(surface, ui, render)
	}

	pub fn handle_event(&mut self, event: &Event<WindowCommand>, when: Instant) {
		match event {
			Event::WindowEvent { event: window_event, ..} => {
				// Check with Egui
				let r = self.state.on_event(&self.context, window_event);
				if r.repaint {
					self.last_update.take();
				}
				if r.consumed {
					return
				}
				match window_event {
					&WindowEvent::KeyboardInput { input, .. } => {
						if let Some(key) = input.virtual_keycode {
							let state = input.state.into();
							if !self.properties.cursor_inside && state != ActiveState::Inactive {
								return
							}
							self.game_widget.input(
								InputEvent::KeyEvent((key.into(), state)), 
								when,
							);
						} else {
							warn!("Key input with no virtual key code ({input:?})");
						}
					},
					&WindowEvent::MouseInput {
						state, 
						button, 
						..
					} => {
						self.game_widget.input(
							InputEvent::KeyEvent((KeyKey::MouseKey(button), state.into())), 
							when, 
						);
					},
					WindowEvent::MouseWheel { delta, .. } => {
						match delta {
							&winit::event::MouseScrollDelta::LineDelta(x, y) => {
								self.game_widget.input(
									InputEvent::Scroll([x, y]),
									when, 
								);
							},
							_ => warn!("detected strange scrolling hours"),
						}
					},
					WindowEvent::CursorEntered {..} => {
						self.properties.cursor_inside = true;
					},
					WindowEvent::CursorLeft {..} => {
						self.properties.cursor_inside = false;
						// release all keys
						warn!("Should release all keys");
						// self.game_widget.release_keys();
					},
					&WindowEvent::CursorMoved { position, .. } => {
						if self.settings.cursor_captured {
							if let Some(last_position) = self.manual_cursor_lock_last_position {
								self.window.set_cursor_position(last_position).unwrap();
							} else {
								self.manual_cursor_lock_last_position = Some(position);
							}
						} else {
							self.manual_cursor_lock_last_position.take();
							self.game_widget.input(
								InputEvent::CursorMoved([position.x, position.y]),
								when,
							);
						}
					},
					WindowEvent::Resized (newsize) => {
						if newsize.width > 0 && newsize.height > 0 {
							self.resize(newsize.width, newsize.height);
						}
					},
					&WindowEvent::Focused(focused) => {
						self.properties.focused = focused;
					},
					_ => {},
				}
			},
			Event::DeviceEvent { event: device_event, .. } => {
				match device_event {
					&DeviceEvent::MouseMotion { delta: (dx, dy) } => {
						if self.properties.cursor_inside && self.settings.cursor_captured {
							self.game_widget.input(
								InputEvent::MouseMotion([dx, dy]),
								when,
							);
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
	NewWindow, // Don't add WindowBuilder, it isn't send
}

#[derive(Debug)]
pub enum GameCommand {
	Shutdown,
}

slotmap::new_key_type! {
	pub struct WindowKey;
}


/// Android doesn't let an application request this stuff until it is [winit::event::Event::Resumed]. 
/// This means that all of this needs to be stored in an option. 
/// Also it gives me an excuse to not feel bad about it. 
pub struct GraphicsHandle {
	pub instance: wgpu::Instance,
	pub adapter: wgpu::Adapter,
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
	pub egui_renderer: Renderer,
	pub profiler: GpuProfiler,
}
impl GraphicsHandle {
	// Use instance to make surface, then 
	pub fn acquire(instance: wgpu::Instance, compatible_surface: &wgpu::Surface) -> Result<Self, wgpu::RequestDeviceError> {
		let adapter = pollster::block_on(instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::HighPerformance,
				compatible_surface: Some(compatible_surface),
				force_fallback_adapter: false,
			},
		)).unwrap();
		let info = adapter.get_info();
		info!("Using adapter {} ({:?})", info.name, info.backend);

		let mut features = adapter.features();
		if features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS) {
			warn!("Adapter has feature {:?} and I don't like that so I am removing it from the feature set", wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
			features = features.difference(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
		}

		let limits = wgpu::Limits::downlevel_defaults();

		let (device, queue) = pollster::block_on(adapter.request_device(
			&wgpu::DeviceDescriptor {
				features, limits,
				label: Some("kkraft device descriptor"),
			},
			None,
		))?;
		let device = Arc::new(device);
		let queue = Arc::new(queue);

		let surface_caps = compatible_surface.get_capabilities(&adapter);
		let output_color_format = preferred_framebuffer_format(&surface_caps.formats).unwrap();
		let egui_renderer = Renderer::new(
			&device,
			output_color_format,
			// These things affect how WindowSurface should be
			Some(wgpu::TextureFormat::Depth32Float),
			1,
		);

		let profiler = GpuProfiler::new(
			5, 
			queue.get_timestamp_period(), 
			features,
		);

		Ok(Self { instance, adapter, device, queue, egui_renderer, profiler })
	}


}


struct WindowGameThing {
	pub game: Arc<Mutex<Game>>,
	pub commands: Sender<GameCommand>,
	pub join_handle: JoinHandle<i32>,
}


pub struct WindowManager {
	event_loop_proxy: EventLoopProxy<WindowCommand>,

	windows: SlotMap<WindowKey, GameWindow>,
	window_id_key: HashMap<WindowId, WindowKey>,

	close_when_no_windows: bool,

	graphics: Option<GraphicsHandle>,

	game: Option<WindowGameThing>,
}
impl WindowManager {
	pub fn new(event_loop: &EventLoop::<WindowCommand>) -> Self {
		let event_loop_proxy = event_loop.create_proxy();

		Self {
			event_loop_proxy,
			windows: SlotMap::with_key(),
			window_id_key: HashMap::new(),
			close_when_no_windows: true,
			graphics: None,
			game: None,
		}
	}

	pub fn start_game(&mut self) {
		let (commands, commands_receiver) = crossbeam_channel::unbounded();

		let device = self.graphics.as_ref().unwrap().device.clone();
		let queue = self.graphics.as_ref().unwrap().queue.clone();

		let game = Arc::new(Mutex::new(Game::new(device, queue, commands_receiver, self.event_loop_proxy.clone())));
		
		let join_handle = {
			let game = game.clone();
			std::thread::spawn(move || {
				profiling::register_thread!("Game Thread");

				// Panic the window thread when game thread panics
				let orig_hook = std::panic::take_hook();
				std::panic::set_hook(Box::new(move |panic_info| {
					orig_hook(panic_info);
					std::process::exit(1);
				}));

				// I tried locking this before sending it to the game thread, but the type isn't Send
				// We just need to hope that this happens before the main thread can lock it
				let mut game_lock = game.lock();
				game_lock.initialize();
				drop(game_lock);

				loop {
					let mut game = game.lock();
					let status = game.tick();
					MutexGuard::unlock_fair(game);

					match status {
						GameStatus::Exit(status) => return status,
						GameStatus::Continue(next_tick) => {
							let to_next_tick = next_tick - Instant::now();
							info!("Next tick in {}ms", to_next_tick.as_millis());
							std::thread::sleep(to_next_tick);
						},
					}
				}
			})
		};

		self.game = Some(WindowGameThing { game, commands, join_handle })
	}

	pub fn run(mut self, event_loop: EventLoop<WindowCommand>) {
		// let initial_window = WindowBuilder::new()
		// 	.with_title("initial window")
		// 	.with_window_icon(None)
		// 	.with_inner_size(PhysicalSize::new(1280, 720))
		// 	.build(&event_loop)
		// 	.unwrap();
		// let instance = wgpu::Instance::new(wgpu::Backends::all());
		// let initial_surface = unsafe { instance.create_surface(&initial_window) };

		event_loop.run(move |event, event_loop, control_flow| {
			let when = Instant::now();
			match event {
				Event::Resumed => {
					info!("Resume!");
					// panic!("Resume!");
					if self.graphics.is_none() {
						trace!("Initial window");
						let initial_window = WindowBuilder::new()
							.with_title("initial window")
							.with_window_icon(None)
							.with_inner_size(PhysicalSize::new(1280, 720))
							.build(event_loop)
							.unwrap();
						
						trace!("Get instance");
						let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
							backends: wgpu::Backends::all(),
							dx12_shader_compiler: wgpu::Dx12Compiler::default(),
						});

						trace!("Make surface");
						let initial_surface = unsafe { instance.create_surface(&initial_window) }.unwrap();

						info!("Initializing graphics");
						let graphics = self.graphics.get_or_insert(GraphicsHandle::acquire(instance, &initial_surface).unwrap());
						
						info!("Creating first window");
						let gw = GameWindow::new_from_window_surface(&graphics.adapter, initial_window, initial_surface);
						self.register_gamewindow(gw);

						info!("Starting game!");
						self.start_game();

						// Put all of this in an initialize() function?
					}
				},
				Event::UserEvent(event) => {
					match event {
						WindowCommand::Shutdown => control_flow.set_exit_with_code(0),
						WindowCommand::NewWindow => {
							let window_builder = WindowBuilder::new();
							self.register_gamewindow(GameWindow::new(&self.graphics.as_ref().unwrap().instance, &self.graphics.as_ref().unwrap().adapter, window_builder, event_loop));
						},
					}
				},
				Event::WindowEvent {event: ref window_event, window_id} => {
					if let Some(window_idx) = self.window_id_key.get(&window_id) {
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
				Event::MainEventsCleared => {
					// info!("Main events cleared");
					// todo!("Query windows for redraw events")
				},
				Event::RedrawRequested(_window_id) => {
					// info!("Redraw requested for id {window_id:?}");
				},
				Event::RedrawEventsCleared => {
					// info!("Redraw events cleared");
					// Can do other things if the game hasn't loaded
					// Like a loading screen!
					let to_update = self.windows.values_mut()
						.filter(|w| w.should_update())
						.collect::<Vec<_>>();

					if to_update.is_empty() {
						return;
					}
					let st = Instant::now();

					if let Some(game_thing) = self.game.as_ref() {
						let mut game = game_thing.game.lock();
						
						let mut textures = Vec::with_capacity(to_update.len());
						let mut command_buffers = Vec::with_capacity(to_update.len() * 2 + 1);
						for window in to_update {
							let (t, ui, game) = window.update(
								self.graphics.as_mut().unwrap(),
								&mut game,
							);
							textures.push(t);
							if let Some(game) = game {
								command_buffers.push(game);
							}
							command_buffers.push(ui);
						}

						let mut encoder = self.graphics.as_mut().unwrap().device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
							label: Some("Profiler resolve"),
						});
						self.graphics.as_mut().unwrap().profiler.resolve_queries(&mut encoder);
						command_buffers.push(encoder.finish());


						let _index = self.graphics.as_ref().unwrap().queue.submit(command_buffers);

						self.graphics.as_mut().unwrap().profiler.end_frame().unwrap();

						for surface in textures {
							surface.present();
						}
						profiling::finish_frame!();
					} else {
						error!("Couldn't redraw becuase game is dispareau");
						panic!()
					}

					info!("Encoded window update in {}ms", st.elapsed().as_millis());
				},
				Event::LoopDestroyed => {
					info!("Loop destroy, shutting down");					
					self.window_id_key.drain();
					for (_, _window) in self.windows.drain() {
						// It may be wise to do per-window shutdown code here
						info!("Closing a window");
					}
				},
				_ => {},
			}
		});
	}

	pub fn register_gamewindow(&mut self, gamewindow: GameWindow) -> WindowKey {
		let id = gamewindow.window.id();
		let key = self.windows.insert(gamewindow);
		self.window_id_key.insert(id, key);
		key
	}

	pub fn close_window(&mut self, key: WindowKey) {
		let wid = self.windows.get(key).unwrap().window.id();
		self.window_id_key.remove(&wid);
		self.windows.remove(key);
		// Dropping the value should cause the window to close

		if self.close_when_no_windows && self.windows.len() == 0 {
			info!("Shutting down due to lack of windows");
			self.event_loop_proxy.send_event(WindowCommand::Shutdown)
				.expect("Failed to send event loop close request");
		}
	}

	fn shutdown(&self) {
		self.event_loop_proxy.send_event(WindowCommand::Shutdown)
			.expect("Failed to send event loop close request");
	}
}


struct WindowSurface {
	surface: wgpu::Surface,
	surface_config: wgpu::SurfaceConfiguration,
	dirty: bool, // flag to reconfigure the surface
	msaa_levels: u32,
	msaa: Option<(wgpu::Texture, wgpu::TextureView)>,
	depth: Option<(wgpu::Texture, wgpu::TextureView)>,
}
impl WindowSurface {
	pub fn new(
		adapter: &wgpu::Adapter, 
		window: &winit::window::Window, 
		surface: wgpu::Surface,
		msaa_levels: u32,
	) -> Self {

		let surface_caps = surface.get_capabilities(adapter);
		let format = preferred_framebuffer_format(&surface_caps.formats).unwrap();
		let size = window.inner_size();
		let width = size.width;
		let height = size.height;
		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
			format,
			width,
			height,
			present_mode: wgpu::PresentMode::Fifo,
			alpha_mode: wgpu::CompositeAlphaMode::Auto,
			view_formats: vec![format],
		};

		assert!(msaa_levels != 0, "msaa levels cannot be zero");

		info!("Created new WindowSurface with format {format:?}");

		Self {
			surface,
			surface_config,
			dirty: true,
			msaa_levels,
			msaa: None,
			depth: None,
		}
	}

	pub fn set_size(&mut self, new_size: [u32; 2]) {
		let [width, height] = new_size;
		if width != self.surface_config.width || height != self.surface_config.height {
			self.surface_config.width = width;
			self.surface_config.height = height;
			self.msaa.take();
			self.depth.take();
			self.dirty = true;
		}
	}

	pub fn frame<'a>(
		&'a mut self, 
		device: &wgpu::Device, 
	) -> (wgpu::SurfaceTexture, SurfaceFrame<'a>) {
		if self.dirty {
			// Expensive (17ms expensive!), so we don't want to do it every time
			self.surface.configure(device, &self.surface_config);
			self.dirty = false;
		}
		
		let frame = match self.surface.get_current_texture() {
			Ok(tex) => tex,
			// Apparently this happens when minimized on Windows
			Err(wgpu::SurfaceError::Outdated) => panic!("Render to outdated texture for window!"),
			Err(e) => panic!("{}", e),
		};
		let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

		self.depth.get_or_insert_with(|| {
			trace!("Create surface depth");
			let size = wgpu::Extent3d {
				width: self.surface_config.width,
				height: self.surface_config.height,
				depth_or_array_layers: 1,
			};
			let depth = device.create_texture(&wgpu::TextureDescriptor {
				label: Some("egui depth"),
				size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Depth32Float,
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
				view_formats: &[wgpu::TextureFormat::Depth32Float],
			});
			let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
			(depth, depth_view)
		});

		if self.msaa.is_none() && self.msaa_levels > 1 {
			trace!("Create surface msaa");
			self.msaa = Some({
				let size = wgpu::Extent3d {
					width: self.surface_config.width,
					height: self.surface_config.height,
					depth_or_array_layers: 1,
				};
				let msaa = device.create_texture(&wgpu::TextureDescriptor {
					label: Some("egui msaa"),
					size,
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: self.surface_config.format,
					usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
					view_formats: &[self.surface_config.format],
				});
				let msaa_view = msaa.create_view(&wgpu::TextureViewDescriptor::default());
				(msaa, msaa_view)
			});
		}

		(
			frame,
			SurfaceFrame {
				frame_view, 
				msaa: self.msaa.as_ref().and_then(|(_, v)| Some(v)),
				depth: &self.depth.as_ref().unwrap().1,
			},
		)
		
	}
}

struct SurfaceFrame<'s> {
	frame_view: wgpu::TextureView,
	msaa: Option<&'s wgpu::TextureView>,
	depth: &'s wgpu::TextureView,
}
impl<'s> SurfaceFrame<'s> {
	pub fn renderpass(&'s self, encoder: &'s mut wgpu::CommandEncoder) -> wgpu::RenderPass<'s> {
		encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some(&*format!("egui renderpass")),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &self.frame_view,
				resolve_target: self.msaa,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color {
						r: 0.0,
						g: 0.0,
						b: 0.0,
						a: 0.0,
					}),
					store: true,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: self.depth,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: true,
				}),
				stencil_ops: None,
			}),
		})
	}
}
