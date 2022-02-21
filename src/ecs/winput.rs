use std::{collections::{HashMap, BTreeSet}, sync::{mpsc, Arc, Mutex}, thread};
use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use crate::window::*;
use crate::ecs::*;
use nalgebra::*;
use specs::prelude::*;
// use specs::{Component, VecStorage};




// Manages windows
// Creates/destroys windows and can read their events
const CLOSE_ON_NO_WINDOWS: bool = true;
pub struct WindowResource {
	pub windows: Vec<GameWindow>,
	id_idx: HashMap<WindowId, usize>,
	event_loop_sender: mpsc::SyncSender<EventLoopEvent>,
	event_thread_handle: thread::JoinHandle<i32>,
	instance: wgpu::Instance,
	adapter: wgpu::Adapter,
	event_queue: Arc<Mutex<Vec<EventWhen>>>,
	pub window_redraw_queue: BTreeSet<usize>,
	pub capturing_cursor: bool,
}
impl WindowResource {
	pub fn new(
		event_loop_proxy: EventLoopProxy<EventLoopEvent>,
		instance: wgpu::Instance,
		adapter: wgpu::Adapter,
		event_queue: Arc<Mutex<Vec<EventWhen>>>,
	) -> Self {
		let windows = Vec::new();
		let id_idx = HashMap::new();

		let (event_loop_sender, event_loop_receiver) = mpsc::sync_channel(10);

		// This needs to stay in its own thread because it cannot sync
		let event_thread_handle = thread::spawn(move || {
			let event_loop_proxy = event_loop_proxy.clone();
			loop {
				match event_loop_receiver.recv() {
					Ok(event) => {
						event_loop_proxy.send_event(event).expect("Could not send window creation request!");
					},
					Err(_) => {
						error!("Failed to recv in event thread, sending close signal");
						event_loop_proxy.send_event(EventLoopEvent::Close).expect("Could not send close signal!");
						return 1
					}
				}
			}
		});

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
			.expect("Could not send window creation request");
	}

	// Get a window fastly (in this iteration)
	pub fn request_window_immediate(&mut self) {
		let (sender, receiver) = mpsc::channel();
		self.event_loop_sender.send(EventLoopEvent::SupplyWindow(sender))
			.expect("Could not send window creation request");
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
			.expect("Could not send event loop close request");
		// Everything else *should* stop/drop when self is dropped (here)
	}
}



// Holds input data
pub struct InputResource {
	// The press percentages for all keys pressed during a timestep
	// It is possible for a percentage to be greater than 100%
	// This happends if startt is after the earliest queue value
	pub board_keys: HashMap<VirtualKeyCode, f32>,
	pub board_presscache: Vec<VirtualKeyCode>,
	pub mouse_keys: HashMap<MouseButton, f32>,
	pub mouse_presscache: Vec<MouseButton>,
	pub mx: f64,
	pub my: f64,
	pub mdx: f64,
	pub mdy: f64,
	// controlmap: HashMap<VirtualKeyCode, (some kind of enum option?)>
}
impl InputResource {
	pub fn new() -> Self {
		let board_keys = HashMap::new();
		let board_presscache = Vec::new();
		let mouse_keys = HashMap::new();
		let mouse_presscache = Vec::new();
		let mx = 0.0;
		let my = 0.0;
		let mdx = 0.0;
		let mdy = 0.0;
		Self {
			board_keys,
			board_presscache,
			mouse_keys,
			mouse_presscache,
			mx, 
			my, 
			mdx, 
			mdy
		}
	}
}



/// Handles window events and feeds the input resource
pub struct WindowEventSystem;
impl<'a> System<'a> for WindowEventSystem {
	type SystemData = (
		WriteExpect<'a, WindowResource>,
		WriteExpect<'a, InputResource>,
		ReadExpect<'a, StepResource>
	);

	fn run(
		&mut self, 
		(
			mut window_resource, 
			mut input_resource, 
			step_resource
		): Self::SystemData
	) {
		let startt = step_resource.last_step;
		let endt = step_resource.this_step;
		let dt = (endt - startt).as_secs_f32();

		// Keyboard buttons
		let mut board_pressmap = HashMap::new();
		for key in &input_resource.board_presscache {
			board_pressmap.insert(*key, startt);
		}
		let mut kpmap = HashMap::new();
		
		// Mouse buttons
		let mut mouse_pressmap = HashMap::new();
		// Unlike key presses, mouse button presses are not constantly resubmitted
		for button in &input_resource.mouse_presscache {
			mouse_pressmap.insert(*button, startt);
		}
		let mut mpmap = HashMap::new();
		
		// Mouse position
		let mut mx = input_resource.mx;
		let mut my = input_resource.my;
		
		// Mouse movement
		let mut mdx = 0.0;
		let mut mdy = 0.0;
		
		// Drain items not passed the start of the step
		let events: Vec<EventWhen> = window_resource.event_queue.lock().unwrap().drain_filter(|e| e.when < endt).collect();
		for event_when in events {
			let ts = event_when.when;
			match event_when.event {
				Event::RedrawRequested(window_id) => {
					let window_idx = window_resource.id_idx[&window_id];
					window_resource.window_redraw_queue.insert(window_idx);
				}
				Event::UserEvent(event) => {
					match event {
						EventLoopEvent::RegisterWindow(window) => {
							window_resource.register_window(window);
						},
						_ => {},
					}
				},
				Event::WindowEvent {event: ref window_event, window_id} => {
					
					// Egui input stuff
					// Egui only uses window events so filtering by window events is fine
					if !window_resource.id_idx.contains_key(&window_id) {
						warn!("input for old window");
						continue
					}
					let window_idx = window_resource.id_idx[&window_id];
					let window = window_resource.windows.get_mut(window_idx).unwrap();
					window.platform.handle_event(&event_when.event);
					
					// Check if egui wants me to not handle this
					if window.platform.captures_event(&event_when.event) {
						continue
					}

					// My input stuff
					match window_event {
						WindowEvent::KeyboardInput {input, ..} => {
							// If the cursor is not inside of this window don't register the input
							if !window.cursor_inside {
								continue
							}

							if let Some(key) = input.virtual_keycode {
								
								// Make sure we are always able to free the cursor
								match key {
									winit::event::VirtualKeyCode::Escape => {
										// let modifiers = ModifiersState::default();
										window.window.set_cursor_grab(false).unwrap();
										window.window.set_cursor_visible(true);
										window_resource.capturing_cursor = false;
									},
									_ => {},
								}

								match input.state {
									ElementState::Pressed => {
										// If this button was not already pressed, record the pressing
										if !board_pressmap.contains_key(&key) {
											board_pressmap.insert(key, ts);
										}
									},
									ElementState::Released => {
										// Only do something if this key had been pressed in the first place
										if board_pressmap.contains_key(&key) {
											let mut kp = (ts - board_pressmap[&key]).as_secs_f32() / dt;
			
											// If this key had been pressed and released, account for that
											if kpmap.contains_key(&key) {
												kp += kpmap[&key];
											}
											// Send the percent of time pressed to input
											kpmap.insert(key, kp);
			
											// Remove key from pressed keys
											board_pressmap.remove(&key);
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
									if !mouse_pressmap.contains_key(&button) {
										mouse_pressmap.insert(button, ts);
									}
								},
								ElementState::Released => {
									if mouse_pressmap.contains_key(&button) {
										let mut mp = (ts - mouse_pressmap[&button]).as_secs_f32() / dt;
										if mpmap.contains_key(&button) {
											mp += mpmap[&button];
										}
										mpmap.insert(button, mp);
										mouse_pressmap.remove(&button);
									}
								},
							}
						},
						WindowEvent::MouseWheel {delta, phase, ..} => {
							let _d = delta;
							let _p = phase;
						},
						WindowEvent::CursorEntered {..} => {
							window.cursor_inside = true;
							window.window.set_cursor_grab(true).unwrap();
							window.window.set_cursor_visible(false);
							window_resource.capturing_cursor = true;
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
							let idx = window_resource.id_idx[&window_id];
							window_resource.close_window(idx);
						},
						_ => {},
					}
				},
				Event::DeviceEvent {event, ..} => {
					match event {
						DeviceEvent::MouseMotion {delta} => {
							if window_resource.capturing_cursor {
								mdx += delta.0;
								mdy += delta.1;
							}
						},
						_ => {},
					}
				}
				_ => {},
			}
		}

		// Process the keys which are still pressed
		for (key, t) in &board_pressmap {
			let mut kp = (endt - *t).as_secs_f32() / dt;
			// If this key had been pressed and released, account for that
			if kpmap.contains_key(&key) {
				kp += kpmap[&key];
			}
			// Send the percent of time pressed to input
			kpmap.insert(*key, kp);
		}
		let board_stillpressed = board_pressmap.keys().map(|x| x.clone()).collect();
		// Again for mouse keys
		for (button, t) in &mouse_pressmap {
			let mut mp = (endt - *t).as_secs_f32() / dt;
			if mpmap.contains_key(&button) {
				mp += mpmap[&button];
			}
			mpmap.insert(*button, mp);
		}
		let mouse_stillpressed = mouse_pressmap.keys().map(|x| x.clone()).collect();

		// Update input resource
		input_resource.board_keys = kpmap;
		input_resource.board_presscache = board_stillpressed;
		input_resource.mouse_keys = mpmap;
		input_resource.mouse_presscache = mouse_stillpressed;
		input_resource.mx = mx;
		input_resource.my = my;
		input_resource.mdx = mdx;
		input_resource.mdy = mdy;
	}
}



/// Reads input resource queue and decides what to do with it
pub struct InputSystem;
impl<'a> System<'a> for InputSystem {
	type SystemData = (
		ReadExpect<'a, InputResource>,
		ReadExpect<'a, StepResource>,
		WriteStorage<'a, TransformComponent>,
		ReadStorage<'a, MovementComponent>,
	);

	fn run(
		&mut self, 
		(
			input_resource, 
			step_resource,
			mut transform, 
			movement
		): Self::SystemData
	) { 
		let secs = step_resource.step_diff.as_secs_f32();

		let rx = input_resource.mdx as f32 * secs * 0.04;
		let ry = input_resource.mdy as f32 * secs * 0.04;

		let mut displacement = Vector3::from_element(0.0);
		for (key, kp) in &input_resource.board_keys {
			match key {
				VirtualKeyCode::W => {
					displacement.z += kp;
				},
				VirtualKeyCode::S => {
					displacement.z -= kp;
				},
				VirtualKeyCode::D => {
					displacement.x += kp;
				},
				VirtualKeyCode::A => {
					displacement.x -= kp;
				},
				VirtualKeyCode::Space => {
					displacement.y += kp;
				},
				VirtualKeyCode::LShift => {
					displacement.y -= kp;
				},
				_ => {},
			}
		}

		for (transform_c, movement_c) in (&mut transform, &movement).join() {

			let quat_ry = UnitQuaternion::from_euler_angles(ry, 0.0, 0.0);
			let quat_rx = UnitQuaternion::from_euler_angles(0.0, rx, 0.0);
			transform_c.rotation = quat_rx * transform_c.rotation * quat_ry;

			transform_c.position += transform_c.rotation * displacement * movement_c.speed * secs;
		}
	}
}
