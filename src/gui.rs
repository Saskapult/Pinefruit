use std::{time::{Instant, Duration}, sync::mpsc::{Receiver, SyncSender}};
use bytemuck::{Pod, Zeroable};
use egui;
use egui_wgpu_backend::RenderPass;
use shipyard::*;
use std::sync::mpsc::sync_channel;
use crate::{render::*, gpu::GraphicsData, game::Game, input::ControlMap, ecs::InputComponent};
use crate::window::WindowSettings;
use crate::ecs::*;
use nalgebra::*;
use wgpu::util::DeviceExt;
use crate::input::*;
use std::path::PathBuf;




#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy)]
struct Camera {
	position: [f32; 4],
	rotation: [[f32; 4]; 4],
	near: f32,
}



#[derive(Debug)]
pub struct GameWidget {
	pub tracked_entity: Option<EntityId>,
	rgba_texture: Option<BoundTexture>,
	srgba_texture: Option<BoundTexture>,
	display_texture: Option<egui::TextureId>,
	last_size: [f32; 2],

	render_delay: Duration,
	last_render: Option<Instant>,

	aspect: Option<f32>, // Aspect ratio for the widget (4.0 / 3.0, 16.0 / 9.0, and so on)

	camera_buffer: Option<wgpu::Buffer>,
	unfiform_bg: Option<wgpu::BindGroup>,
	srgb_blitter: Option<Blitter>,
}
impl GameWidget {
	pub fn new(
		tracked_entity: Option<EntityId>,
	) -> Self {

		Self {
			tracked_entity,
			rgba_texture: None,
			srgba_texture: None,
			display_texture: None,
			last_size: [400.0; 2],

			render_delay: Duration::from_secs_f32(1.0 / 30.0),
			last_render: None,

			aspect: None,

			camera_buffer: None,
			unfiform_bg: None,
			srgb_blitter: None,
		}
	}

	pub fn pre_tick_stuff(&mut self, game: &mut Game, input_segment: InputSegment) {
		// Make entity if not exists
		let entity = *self.tracked_entity.get_or_insert_with(|| {
			info!("Creating game widgit entity");
			let mut cc = ControlMap::new(); // TEMPORARY
			game.world.add_entity((
				CameraComponent::new().with_fovy_degrees(75.0),
				TransformComponent::new()
					.with_position(Vector3::new(0.5, 0.5, 0.5)),
				MovementComponent::new(&mut cc),
				InputComponent::new(),
				MouseComponent::new(),
				// KeysComponent::new(),
				ControlComponent::from_map(cc),
				MapLoadingComponent::new(11),
				MapOctreeLoadingComponent::new(11),
				VoxelRenderingComponent::new(11),
				MapLookAtComponent::new(100.0),
			))
		});
		// Apply input
		let mut inputs = game.world.borrow::<ViewMut<InputComponent>>().unwrap();
		if let Ok((input,)) = (&mut inputs,).get(entity) {
			input.input = input_segment;
			// input.last_read = self.last_render.unwrap_or_else(|| input.get(0).and_then(|v| Some(v.1)).unwrap_or(Instant::now()));
			// input.last_feed = Instant::now();
		}
	}

	// True if actually encoded anything
	pub fn maybe_encode_render(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		egui_rpass: &mut RenderPass,
		encoder: &mut wgpu::CommandEncoder,
		// Maybe take game instead?
		world: &shipyard::World,
		gpu_data: &GraphicsData,
	) -> bool {
		if let Some(t) = self.last_render {
			if t.elapsed() < self.render_delay {
				return false;
			}
		}
		self.last_render = Some(Instant::now());

		if let Some(entity) = self.tracked_entity {

			// Camera
			let ccs = world.borrow::<View<CameraComponent>>().unwrap();
			let camera_camera = ccs.get(entity)
				.expect("Render point has no camera!");
			let tcs = world.borrow::<View<TransformComponent>>().unwrap();
			let camera_transform = tcs.get(entity)
				.expect("Render camera has no transform!");
			let camera_data = camera_camera.rendercamera(&camera_transform);


			// Textures and stuff
			self.update_size(device);
			let rgba = self.rgba_texture.as_ref().unwrap();
			let srgba = self.srgba_texture.as_ref().unwrap();
			let blitter = self.srgb_blitter.get_or_insert_with(|| Blitter::new(device, wgpu::TextureFormat::Rgba8UnormSrgb));
			
			// Once I update wgpu and get pipeline.get_bind_group_layout(i) it's over for you
			let shader_idx = gpu_data.shaders.index_from_path(&PathBuf::from("resources/shaders/voxel_scene.ron"));
			if shader_idx.is_none() {
				error!("Thing not leaded");
				panic!();
			}
			let shader_idx = shader_idx.unwrap();
			let shader_pt = gpu_data.shaders.prototype(shader_idx).unwrap();

			// Camera
			let aspect = self.last_size[0] / self.last_size[1];
			let camera_buffer = self.camera_buffer.get_or_insert_with(|| {
				info!("Making camera buffer");
				camera_data.make_buffer(device, aspect)
			});
			camera_data.update_buffer(queue, &*camera_buffer, aspect);
			let uniform_bg = self.unfiform_bg.get_or_insert_with(|| {
				let shader_idx = gpu_data.shaders.index_from_path(&PathBuf::from("resources/shaders/voxel_scene.ron")).unwrap();
				let shader_pt = gpu_data.shaders.prototype(shader_idx).unwrap();
				let ubgl = gpu_data.shaders.layout(&shader_pt.bind_group_entries(0).unwrap()).unwrap();
				device.create_bind_group(&wgpu::BindGroupDescriptor {
					label: None,
					layout: ubgl,
					entries: &[
						wgpu::BindGroupEntry {
							binding: 0,
							resource: wgpu::BindingResource::Buffer(camera_buffer.as_entire_buffer_binding()),
						},
					],
				})
			});

			if let Ok(vrc) = world.borrow::<View<VoxelRenderingComponent>>().unwrap().get(entity) {
				let mut vrr = world.borrow::<UniqueViewMut<VoxelRenderingResource>>().unwrap();
				vrr.update_uniform(queue, vrc);
				// println!("{vrc:#?}");
				// panic!();
				let shader_pl = gpu_data.shaders.pipeline(vrr.scene_shader_index).unwrap();
				let destination_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
					label: None,
					layout: gpu_data.shaders.layout(&shader_pt.bind_group_entries(1).unwrap()).unwrap(),
					entries: &[
						wgpu::BindGroupEntry {
							binding: 0,
							resource: wgpu::BindingResource::TextureView(&rgba.view),
						},
					],
				});

				let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: Some("voxel scene pass"),
				});
				
				cp.set_pipeline(shader_pl.compute().unwrap());

				cp.set_bind_group(0, &uniform_bg, &[]);
				cp.set_bind_group(1, &destination_bg, &[]);
				cp.set_bind_group(2, &vrr.scene_bg, &[]);
				
				cp.dispatch_workgroups(rgba.size.width.div_ceil(16), rgba.size.height.div_ceil(16), 1);
			}

			blitter.blit(device, encoder, &rgba.view, &srgba.view);

			match self.display_texture {
				Some(id) => {
					egui_rpass.update_egui_texture_from_wgpu_texture(
						device, 
						&srgba.view, 
						wgpu::FilterMode::Nearest, 
						id,
					).unwrap();
				},
				None => {
					self.display_texture = Some(egui_rpass.egui_texture_from_wgpu_texture(
						device,
						&srgba.view,
						wgpu::FilterMode::Nearest, 
					));
				},
			};

		}

		true
	}

	/// Adjusts the texture size to the widget size
	pub fn update_size(&mut self, device: &wgpu::Device) {
		let update_size_internal = |texture: &mut Option<BoundTexture>, intended_size: [u32; 2], format: TextureFormat, usages: wgpu::TextureUsages| -> bool {
			if let Some(texture) = texture.as_ref() {
				let size = [texture.size.width, texture.size.height];
				if size == intended_size {
					return false;
				}
			}
			texture.replace(BoundTexture::new(
				device, 
				format,
				intended_size[0], 
				intended_size[1], 
				1,
				"GameWidgetTexture",
				usages,
			));
			true
		};

		let intended_size = self.last_size.map(|f| f.round() as u32);
		update_size_internal(
			&mut self.rgba_texture, intended_size, TextureFormat::Rgba8Unorm, 
			wgpu::TextureUsages::RENDER_ATTACHMENT 
				| wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::STORAGE_BINDING,
		);
		update_size_internal(
			&mut self.srgba_texture, intended_size, TextureFormat::Rgba8UnormSrgb,
			wgpu::TextureUsages::RENDER_ATTACHMENT 
				| wgpu::TextureUsages::TEXTURE_BINDING,
		);
	}
	
	pub fn update_display(
		&mut self,
		rpass: &mut egui_wgpu_backend::RenderPass, 
		device: &wgpu::Device,
	) {
		self.display_texture = self.srgba_texture.as_ref().and_then(|st| {
			Some(rpass.egui_texture_from_wgpu_texture(
				device,
				&st.view,
				wgpu::FilterMode::Nearest, 
			))
		});
	}

	pub fn display(&mut self, ui: &mut egui::Ui, window_settings: &mut WindowSettings) {

		if self.tracked_entity.is_none() {
			ui.label("Tracked entity not set!");
		} else if self.rgba_texture.is_none() {
			ui.label("RGBA texture not created!");
		} else if self.srgba_texture.is_none() {
			ui.label("SRGBA texture not created!");
		}
		
		if let Some(tid) = self.display_texture {
			let mut size = ui.available_size();
			if let Some(a) = self.aspect {
				size.y = size.x / a; 
			}
			self.last_size = size.into();
			
			let g = ui.image(tid, size);
			let f = g.interact(egui::Sense::click());
			if f.clicked() {
				println!("cap");
				window_settings.capture_mouse = true;
			};
			if f.secondary_clicked() {
				println!("decap");
				window_settings.capture_mouse = false;
			};
		} else {
			ui.label("Display texture not created!");
		}
	}
}


pub struct EntityMovementWidget;
impl EntityMovementWidget {
	pub fn display(&mut self, ui: &mut egui::Ui, movement_component: &mut MovementComponent) {
		// ui.text_edit_single_line();
		ui.add(egui::Slider::new(&mut movement_component.max_speed, 0.1..=20.0).text("Maximum Speed"));
	}
}

pub struct EntityMapLookComponent;
impl EntityMapLookComponent {
	pub fn display(&self, ui: &mut egui::Ui, looking_component: &mut MapLookAtComponent) {
		let n = looking_component.hit.as_ref()
			.and_then(|s| Some(format!("'{s}'")))
			.unwrap_or("None".to_string());
		ui.label(format!("Hit {n}"));
	}
}



#[derive(Debug)]
pub struct MessageWidget {
	messages: Vec<(String, Instant)>,
	receiver: Receiver<(String, Instant)>,
	sender: SyncSender<(String, Instant)>,
}
impl MessageWidget {
	pub fn new() -> Self {

		let (sender, receiver) = sync_channel(100);

		Self {
			messages: Vec::new(),
			receiver,
			sender,
		}
	}

	pub fn new_sender(&self) -> SyncSender<(String, Instant)> {
		self.sender.clone()
	}

	pub fn add_message(&mut self, message: impl Into<String>, remove_after: Instant) {
		self.messages.push((message.into(), remove_after));
	}

	pub fn display(&mut self, ui: &mut egui::Ui) {
		// Get new popups
		self.messages.extend(self.receiver.try_iter());

		// Remove expired popups
		let now = Instant::now();
		self.messages.drain_filter(|(_, t)| *t < now);

		// List popups
		ui.scope(|ui| {
			ui.visuals_mut().override_text_color = Some(egui::Color32::RED);
			ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
  			ui.style_mut().wrap = Some(false);
			ui.style_mut().text_styles.iter_mut().for_each(|(_, font_id)| font_id.size = 8.0);
			
			ui.vertical(|ui| {
				self.messages.iter().for_each(|(message, _)| {
					ui.label(message);
				});
			});
		});
		
	}
}


// Lua REPL
// Inventory
// Hotabar
