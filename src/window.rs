use controls::{InputEvent, KeyKey};
use egui::{Context, ViewportId};
use egui_wgpu::{preferred_framebuffer_format, Renderer, ScreenDescriptor};
use ekstensions::prelude::*;
use parking_lot::{Mutex, RwLock};
use slotmap::SlotMap;
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings};
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
use crate::client::GameInstance;
use crate::game::Game;
use crate::gui::viewport::ViewportManager;
use crate::gui::GameWidget;
use crate::server::ServerCommand;
use crate::util::RingDataHolder;



/// Window settings (things you can modify). 
/// Mainly here becuase egui lacks a way to lock the cursor. 
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


/// Window properties (stuff that's decided by external forces). 
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


struct GameWindow {
	window_surface: WindowSurface,
	surface_config: SurfaceConfiguration,
	context: egui::Context,
	state: egui_winit::State,
	properties: WindowProperties,
	settings: WindowSettings,
	profiler: wgpu_profiler::GpuProfiler, // For egui renders

	// Winit doesn't support locking the cursor on x11, only confining it
	// We need to do this manually (brings needless mess)
	manual_cursor_lock_last_position: Option<PhysicalPosition<f64>>,

	last_update: Option<Instant>,
	update_period: Duration, // Can have another for unfocused delay
	update_times: RingDataHolder<Duration>,

	client: Arc<Mutex<GameInstance>>,

	viewports: ViewportManager,

	game_widget: GameWidget,
	// message_widget: MessageWidget,
	// profiling_widget: RenderProfilingWidget,
	// show_profiler: bool,
}
impl GameWindow {
	pub fn new(
		instance: &wgpu::Instance, 
		adapter: &wgpu::Adapter, 
		window_builder: WindowBuilder,
		event_loop: &EventLoopWindowTarget::<WindowCommand>,
		client: &Arc<Mutex<GameInstance>>,
	) -> Self {
		let window = window_builder.build(event_loop).unwrap();
		let window_surface = WindowSurface::new(instance, window);
		Self::new_from_window_surface(adapter, window_surface, client)
	}

	// Used when creating the first window because the GraphicsHandle needs to know the compatible surface 
	pub fn new_from_window_surface(
		adapter: &wgpu::Adapter, 
		window_surface: WindowSurface, 
		client: &Arc<Mutex<GameInstance>>,
	) -> Self {
		let client = client.clone();

		let surface = SurfaceConfiguration::new(adapter, &window_surface, 1);
		let egui_context = Context::default();
		egui_context.style_mut(|style| {
			style.override_text_style = Some(egui::TextStyle::Monospace);
		});
		let state = egui_winit::State::new(
			egui_context, 
			ViewportId::from_hash_of(window_surface.window.id()), 
			&window_surface.window,
			None,
			None,
		);

		let mut viewports = ViewportManager::default();

		// TODO: actual entity
		let game_widget = GameWidget::new(&mut client.lock().world, &mut viewports, Entity::default());

		Self {
			window_surface,
			surface_config: surface,
			context: Context::default(),
			state,
			properties: WindowProperties::new(),
			settings: WindowSettings::new(),
			profiler: GpuProfiler::new(GpuProfilerSettings::default()).unwrap(),

			manual_cursor_lock_last_position: None,
			
			last_update: None,
			update_period: Duration::from_secs_f32(1.0 / 60.0),
			update_times: RingDataHolder::new(30),

			client,

			viewports,

			game_widget,
			// message_widget: MessageWidget::new(),
			// profiling_widget: RenderProfilingWidget::new(),
			// show_profiler: false,
		}
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		self.surface_config.set_size([width, height]);
		self.last_update = None;
	}

	/// The code for displaying the UI. 
	/// Can be copied and pasted into `update()`. 
	#[profiling::function]
	fn ui(
		&mut self,
		graphics: &mut GraphicsHandle,
		world: &mut World,
	) {
		let mut setting_props = WindowPropertiesAndSettings {
			window: &mut self.window_surface.window,
			settings: &mut self.settings,
			properties: &self.properties
		};

		egui::SidePanel::left("left panel")
			.resizable(false)
			.default_width(220.0)
			.max_width(220.0)
			.min_width(220.0)
			.show(&self.context, |ui| {
				ui.vertical(|ui| {

					// Update rate for the UI
					let ui_update_rate = self.update_times.iter()
						.map(|d| d.as_secs_f32())
						.reduce(|a, v| a + v)
						.unwrap_or(f32::INFINITY) / (self.update_times.len() as f32);
					ui.label(format!("UI: {:>4.1}ms, {:.0}Hz", ui_update_rate * 1000.0, (1.0 / ui_update_rate).round()));
				});
			});
		egui::SidePanel::right("right panel")
			.show(&self.context, |ui| {
				ui.vertical(|ui| {
					
				});
			});
		egui::CentralPanel::default()
			.show(&self.context, |ui| {
				ui.vertical_centered_justified(|ui| {
					// self.game_widget.display(ui, &mut setting_props);
				});
			});
	}

	pub fn should_update(&self) -> bool {
		self.last_update.is_none() || self.last_update.unwrap().elapsed() >= self.update_period
	}

	/// Encodes and executes an update to this window's display.
	#[profiling::function]
	pub fn update(
		&mut self,
		graphics: &mut GraphicsHandle,
		world: &mut World,
	) {
		if let Some(t) = self.last_update {
			self.update_times.insert(t.elapsed());
		}
		self.last_update = Some(Instant::now());

		// Do egui frame
		self.context.begin_frame(self.state.take_egui_input(&self.window_surface.window));
		self.ui(graphics, world);
		let full_output = self.context.end_frame();

		// Create command buffers for any viewports
		// Todo: tick game here, potentially in parallel! 
		let (mut command_buffers, mut profilers): (Vec<_>, Vec<_>) = self.viewports.update_viewports(graphics, world).into_iter().unzip();

		let device = &graphics.device;
		let queue = &graphics.queue;
		let renderer = &mut graphics.egui_renderer;

		// Collect egui output
		self.state.handle_platform_output(&self.window_surface.window, full_output.platform_output);
		let textures_delta = full_output.textures_delta;
		let paint_jobs = self.context.tessellate(full_output.shapes, full_output.pixels_per_point);

		let screen_descriptor = ScreenDescriptor {
			size_in_pixels: self.window_surface.window.inner_size().into(),
			pixels_per_point: full_output.pixels_per_point,
		};
		self.surface_config.set_size(self.window_surface.window.inner_size().into());

		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("a window encoder"),
		});

		// Render egui
		let window_query = self.profiler.begin_query(&*format!("window '{}' ({:?}) egui", self.window_surface.window.title(), self.window_surface.window.id()), &mut encoder, device);

		// Update textures and buffers
		for (id, image_delta) in textures_delta.set {
			renderer.update_texture(device, queue, id, &image_delta);
		}
		let user_buffers = renderer.update_buffers(device, queue, &mut encoder, paint_jobs.as_slice(), &screen_descriptor);
		assert_eq!(0, user_buffers.len(), "there shouldn't have been any user-defined command buffers, yet there were user-defined command buffers!");
		
		// Render frame
		let (surface, frame) = self.surface_config.frame(device, &self.window_surface);
		{
			let mut egui_render_pass = frame.renderpass(&mut encoder);
			renderer.render(&mut egui_render_pass, paint_jobs.as_slice(), &screen_descriptor);
		}

		// Free textures
		for id in textures_delta.free.iter() {
			renderer.free_texture(id);
		}

		self.profiler.end_query(&mut encoder, window_query);

		// Add egui command buffer to game command buffers
		command_buffers.push(encoder.finish());

		queue.submit(command_buffers);

		self.profiler.end_frame().unwrap();
		profilers.iter_mut().for_each(|p| p.end_frame().unwrap());

		surface.present();

		profiling::finish_frame!();
	}

	pub fn handle_event(&mut self, event: &Event<WindowCommand>, when: Instant) {
		match event {
			Event::WindowEvent { event: window_event, ..} => {
				// Check with Egui
				let r = self.state.on_window_event(&self.window_surface.window, window_event);
				if r.repaint { self.last_update.take(); }
				if r.consumed { return }
				// Event was not consumed by egui
				match window_event {
					WindowEvent::KeyboardInput { event, .. } => {
						if self.properties.cursor_inside && !event.repeat {
							self.game_widget.input(
								(event.physical_key, event.state),
								when,
							);
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
							_ => warn!("only MouseScrollDelta::LineDelta is recognized by the application"),
						}
					},
					WindowEvent::CursorEntered {..} => {
						self.properties.cursor_inside = true;
					},
					WindowEvent::CursorLeft {..} => {
						self.properties.cursor_inside = false;
						// If we want to deduplicate events in the game widget, then we should release all keys here
						warn!("Should release all keys");
						// self.game_widget.release_keys();
					},
					&WindowEvent::CursorMoved { position, .. } => {
						if self.settings.cursor_captured {
							if let Some(last_position) = self.manual_cursor_lock_last_position {
								self.window_surface.window.set_cursor_position(last_position).unwrap();
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


/// A custom event which is used to allow the game to shut down the window manager and spawn new windows. 
#[derive(Debug)]
pub enum WindowCommand {
	Shutdown,
	NewWindow, // Don't add WindowBuilder, it isn't send
}


/// Commands sent from the window to the game. 
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
	pub fn new(instance: wgpu::Instance, compatible_surface: &wgpu::Surface) -> Result<Self, wgpu::RequestDeviceError> {

		info!("Available adapters:");
		for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
			let info = adapter.get_info();
			info!("{}: {} ({}, {:?})", info.device, info.name, info.backend.to_str(), info.device_type);
		}

		let adapter = pollster::block_on(instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::HighPerformance,
				compatible_surface: Some(compatible_surface),
				force_fallback_adapter: false,
			},
		)).unwrap();
		let info = adapter.get_info();
		info!("Using adapter {} ({:?})", info.name, info.backend);

		let mut required_features = adapter.features();
		if required_features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS) {
			warn!("Adapter has feature {:?} and I don't like that so I am removing it from the feature set", wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
			required_features = required_features.difference(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
		}

		let required_limits = wgpu::Limits::downlevel_defaults();

		let (device, queue) = pollster::block_on(adapter.request_device(
			&wgpu::DeviceDescriptor {
				required_features, required_limits,
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

		let profiler = GpuProfiler::new(GpuProfilerSettings {
			max_num_pending_frames: 5, 
			..Default::default()
		}).unwrap();

		Ok(Self { instance, adapter, device, queue, egui_renderer, profiler })
	}
}


pub struct WindowManager {
	event_loop_proxy: EventLoopProxy<WindowCommand>,

	windows: SlotMap<WindowKey, GameWindow>,
	window_id_key: HashMap<WindowId, WindowKey>,

	close_when_no_windows: bool,

	graphics: GraphicsHandle,

	// Also in client and server 
	// extensions: Arc<RwLock<ExtensionRegistry>>,
	client: Arc<Mutex<GameInstance>>,
	// Server should have its own extensions beucase of the way I set up ekstensions
	// Just sync changes between client and server extensions
	// Also this should be an Arc<Option<...>>
	// That way we can pass it to the windows
	server: Arc<Option<(Arc<Mutex<World>>, crossbeam_channel::Sender<ServerCommand>, JoinHandle<anyhow::Result<()>>)>>,
}
impl WindowManager {
	pub fn run() {
		let event_loop = EventLoopBuilder::<WindowCommand>::with_user_event().build().unwrap();
		let event_loop_proxy = event_loop.create_proxy();

		trace!("Creating initial window");
		let initial_window = WindowBuilder::new()
			.with_title("initial window")
			.with_window_icon(None)
			.with_inner_size(PhysicalSize::new(1280, 720))
			.build(&event_loop)
			.unwrap();
		
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());

		let window_surface = WindowSurface::new(&instance, initial_window);

		info!("Initializing graphics");
		let graphics = GraphicsHandle::new(instance, &window_surface.surface).unwrap();

		info!("Creating client");
		let client = Arc::new(Mutex::new(GameInstance::new()));

		// info!("Creating internal server");
		// info!("Attaching client to internal server");

		let mut s = Self {
			event_loop_proxy,
			windows: SlotMap::with_key(),
			window_id_key: HashMap::new(),
			close_when_no_windows: true,
			graphics,
			client,
			server: Arc::new(None),
		};
		
		let gw = GameWindow::new_from_window_surface(&s.graphics.adapter, window_surface, &s.client);
		s.register_gamewindow(gw);

		event_loop.run(move |event, event_loop| {
			let when = Instant::now();
			match event {
				Event::Resumed => {
					info!("Resume");
				},
				Event::UserEvent(event) => {
					match event {
						WindowCommand::Shutdown => event_loop.exit(),
						WindowCommand::NewWindow => {
							todo!("Create new GameWindow");
							// let window_builder = WindowBuilder::new();
							// self.register_gamewindow(GameWindow::new(&self.graphics.as_ref().unwrap().instance, &self.graphics.as_ref().unwrap().adapter, window_builder, event_loop));
						},
					}
				},
				Event::WindowEvent {event: ref window_event, window_id} => {
					if let Some(window_idx) = s.window_id_key.get(&window_id) {
						let window = s.windows.get_mut(*window_idx).unwrap();
						window.handle_event(&event, when);
						
						if window_event == &WindowEvent::CloseRequested {
							s.close_window(*window_idx);
						}
					}					
				},
				Event::DeviceEvent {event: ref device_event, ..} => {
					match device_event {
						DeviceEvent::MouseMotion { .. } => {
							for (_, window) in s.windows.iter_mut() {
								window.handle_event(&event, when);
							}
						},
						_ => {},
					}
				},
				Event::LoopExiting => {
					info!("Loop destroy, shutting down");					
					s.window_id_key.drain();
					for (_, _window) in s.windows.drain() {
						// It may be wise to do per-window shutdown code here
						info!("Closing a window");
					}
				},
				_ => {},
			}
		}).unwrap();
	}

	fn register_gamewindow(&mut self, gamewindow: GameWindow) -> WindowKey {
		let id = gamewindow.window_surface.window.id();
		let key = self.windows.insert(gamewindow);
		self.window_id_key.insert(id, key);
		key
	}

	pub fn close_window(&mut self, key: WindowKey) {
		let wid = self.windows.get(key).unwrap().window_surface.window.id();
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


struct SurfaceConfiguration {
	surface_config: wgpu::SurfaceConfiguration,
	dirty: bool, // flag to reconfigure the surface
	msaa_levels: u32,
	msaa: Option<(wgpu::Texture, wgpu::TextureView)>,
	depth: Option<(wgpu::Texture, wgpu::TextureView)>,
}
impl SurfaceConfiguration {
	pub fn new(
		adapter: &wgpu::Adapter, 
		window: &WindowSurface,
		msaa_levels: u32,
	) -> Self {

		let surface_caps = window.surface.get_capabilities(adapter);
		let format = preferred_framebuffer_format(&surface_caps.formats).unwrap();
		let size = window.window.inner_size();
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
			desired_maximum_frame_latency: 2,
		};

		assert!(msaa_levels != 0, "msaa levels cannot be zero");

		info!("Created new WindowSurface with format {format:?}");

		Self {
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
		window: &WindowSurface,
	) -> (wgpu::SurfaceTexture, SurfaceFrame<'a>) {
		if self.dirty {
			// Expensive (17ms expensive!), so we don't want to do it every time
			window.surface.configure(device, &self.surface_config);
			self.dirty = false;
		}
		
		let frame = match window.surface.get_current_texture() {
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


/// A window and a surface for that window. 
/// 
/// Uses unsafe code, but is safe because the surface is always dropped before the window. 
struct WindowSurface {
	pub window: winit::window::Window,
	pub surface: wgpu::Surface<'static>,
}
impl WindowSurface {
	pub fn new(
		instance: &wgpu::Instance,
		window: winit::window::Window,
	) -> Self {
		let s = instance.create_surface(&window).unwrap();
		let surface = unsafe { std::mem::transmute(s) };
		Self { window, surface, }
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
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: self.depth,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			timestamp_writes: None,
			occlusion_query_set: None,
		})
	}
}
