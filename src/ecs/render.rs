use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Instant, Duration};
use crate::render::*;
use crate::mesh::*;
use crate::material::*;
use crate::texture::*;
use specs::prelude::*;
use specs::{Component, VecStorage};
use crate::ecs::*;
use nalgebra::*;
use crate::window::*;




#[derive(Debug)]
enum RenderTarget {
	Window(usize),
	Texture(usize),
}



pub struct RenderResource {
	pub render_instance: RenderInstance,
	pub materials_manager: Arc<RwLock<MaterialManager>>,
	pub textures_manager: Arc<RwLock<TextureManager>>,
	pub meshes_manager: Arc<RwLock<MeshManager>>,
	egui_rpass: egui_wgpu_backend::RenderPass,
	pub submit_durations: crate::util::DurationHolder,
	pub encode_durations: crate::util::DurationHolder,
}
impl RenderResource {
	pub fn new(adapter: &wgpu::Adapter) -> Self {

		let textures_manager = Arc::new(RwLock::new(TextureManager::new()));

		let materials_manager = Arc::new(RwLock::new(MaterialManager::new()));

		let meshes_manager = Arc::new(RwLock::new(MeshManager::new()));

		let renderer = pollster::block_on(
			crate::render::RenderInstance::new(
				adapter,
				&textures_manager,
				&meshes_manager,
				&materials_manager,
			)
		);

		let egui_rpass = egui_wgpu_backend::RenderPass::new(
			&renderer.device, 
			wgpu::TextureFormat::Bgra8UnormSrgb, 
			1,
		);

		Self {
			render_instance: renderer,
			materials_manager,
			textures_manager,
			meshes_manager,
			egui_rpass,
			submit_durations: crate::util::DurationHolder::new(32),
			encode_durations: crate::util::DurationHolder::new(32),
		}
	}	
}



#[derive(Debug)]
pub struct LineData {
	pub start: Point3<f32>,
	pub end: Point3<f32>,
	pub colour: [f32; 3],
	pub remove_after: Instant,
}



/// Holds lines to be rendered.
/// Lines could easily be made into components but are stored here because I said so.
#[derive(Debug)]
pub struct LinesResource {
	pub lines: Vec<LineData>,
}
impl LinesResource {
	pub fn new() -> Self {
		Self {
			lines: Vec::new(),
		}
	}

	pub fn prune(&mut self, t: Instant) {
		self.lines = self.lines.drain(..).filter(|line| {
			line.remove_after < t
		}).collect::<Vec<_>>();
	}
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct CameraComponent {
	target: RenderTarget,
	fovy: f32,
	znear: f32,
	zfar: f32,
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			target: RenderTarget::Window(0),
			fovy: 45.0,
			znear: 0.1,
			zfar: 100.0,
		}
	}
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct ModelComponent {
	pub mesh_idx: usize,
	pub material_idx: usize,
}
impl ModelComponent {
	pub fn new(
		mesh_idx: usize,
		material_idx: usize,
	) -> Self {
		Self {
			mesh_idx,
			material_idx,
		}
	}
}



// pub struct NonFunctionalRepaintSignal;
// impl epi::backend::RepaintSignal for NonFunctionalRepaintSignal {
//     fn request_repaint(&self) {}
// }
pub struct RenderSystem;
impl RenderSystem {
	fn render_ui(
		mut encoder: &mut wgpu::CommandEncoder,
		render_resource: &mut RenderResource,
		window: &mut GameWindow,
		destination_view: &wgpu::TextureView,
	) {
		let egui_start = Instant::now();
		window.platform.begin_frame();

		let input = egui::RawInput::default();
		let (_output, shapes) = window.platform.context().run(input, |egui_ctx| {
			egui::SidePanel::left("my_side_panel").show(egui_ctx, |ui| {
				ui.label(format!("submit time: {}ms", render_resource.submit_durations.latest().unwrap_or(Duration::ZERO).as_millis()));
				ui.label(format!("encode time: {}ms", render_resource.encode_durations.latest().unwrap_or(Duration::ZERO).as_millis()));

				// if ui.button("Clickme").clicked() {
				// 	panic!("Button click");
				// }
			});
		});

		//let (_output, shapes) = window.platform.end_frame(Some(&window.window));
		let paint_jobs = window.platform.context().tessellate(shapes);

		let frame_time = (Instant::now() - egui_start).as_secs_f64() as f32;
		window.previous_frame_time = Some(frame_time);

		let device = render_resource.render_instance.device.clone();
		let queue = render_resource.render_instance.queue.clone();

		// GPU upload
		let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
			physical_width: window.window.outer_size().width,
			physical_height: window.window.outer_size().height,
			scale_factor: window.window.scale_factor() as f32,
		};
		render_resource.egui_rpass.update_texture(
			&device, 
			&queue, 
			&window.platform.context().font_image(),
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

	/// Should do culling in the future
	fn get_render_data<'a>(
		models: &ReadStorage<'a, ModelComponent>,
		maps: &ReadStorage<'a, MapComponent>,
		transforms: &ReadStorage<'a, TransformComponent>,
		_camera: &CameraComponent,
		_camera_transform: &TransformComponent,
	) -> Vec<ModelInstance> {
		let mut render_data = Vec::new();
		// Models
		for (model_c, transform_c) in (models, transforms).join() {
			let instance = Instance::new()
				.with_position(transform_c.position);
			let model_instance = ModelInstance {
				material_idx: model_c.material_idx,
				mesh_idx: model_c.mesh_idx,
				instance,
			};
			render_data.push(model_instance);
		}
		// Map chunks
		for (map_c, transform_c) in (maps, transforms).join() {
			// Renders ALL meshed chunks
			for (cp, entry) in &map_c.chunk_models {
				match entry {
					ChunkModelEntry::Complete(mesh_mats) => {
						
						let position = transform_c.position + map_c.map.chunk_point(*cp);
						let instance = Instance::new().with_position(position);
						for (mesh_idx, material_idx) in mesh_mats.iter().cloned() {
							let model_instance = ModelInstance {
								material_idx,
								mesh_idx,
								instance,
							};
							render_data.push(model_instance);
						}
					},
					_ => {},
				}
			}
		}
		render_data
	}

	fn render_game<'a>(
		mut encoder: &mut wgpu::CommandEncoder,
		render_resource: &mut RenderResource,
		camera: &CameraComponent,
		camera_transform: &TransformComponent,
		width: u32,
		height: u32,
		render_data: Vec<ModelInstance>,
		destination_texture: &wgpu::Texture,
	) {
		render_resource.render_instance.set_data(render_data);

		let render_camera = crate::render::Camera {
			position: camera_transform.position,
			rotation: camera_transform.rotation,
			fovy: camera.fovy,
			znear: camera.znear,
			zfar: camera.zfar,
		};
		
		render_resource.render_instance.render(
			&mut encoder,
			destination_texture, 
			width, 
			height, 
			&render_camera, 
			Instant::now(),
		);
	}
}
impl<'a> System<'a> for RenderSystem {
	type SystemData = (
		WriteExpect<'a, RenderResource>,
		WriteExpect<'a, WindowResource>,
		ReadStorage<'a, ModelComponent>,
		ReadStorage<'a, MapComponent>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);

	fn run(
		&mut self, 
		(
			mut render_resource, 
			mut window_resource,
			models,
			maps,
			cameras,
			transforms,
		): Self::SystemData,
	) { 
		for (camera, camera_transform) in (&cameras, &transforms).join() {
			
			match camera.target {
				// Find destination
				RenderTarget::Window(id) => {
					// Don't render for windows that don't want to be rendered to
					// if !window_resource.window_redraw_queue.contains(&id) {
					// 	info!("render for window {id} is skipped (not queued for rendering)");
					// }
					// window_resource.window_redraw_queue.remove(&id);

					if id >= window_resource.windows.len() {
						error!("Tried to render to nonexistent window! idx: {}", id);
						return
					}
					info!("Rendering to window idx {}", id);

					// Get window data
					let mut window = window_resource.windows.get_mut(id).unwrap();
					window.surface.configure(&render_resource.render_instance.device, &window.surface_config);
					
					let frame = match window.surface.get_current_texture() {
						Ok(tex) => tex,
						Err(wgpu::SurfaceError::Outdated) => {
							// Apparently happens when minimized on Windows
							error!("Render to outdated texture for window {id}");
							panic!();
						},
						Err(e) => {
							panic!("{}", e);
						},
					};
					let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

					let mut encoder = render_resource.render_instance.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
						label: Some("Render Encoder"),
					});

					// If should render game
					if true {
						let encode_st = Instant::now();
						let width = window.surface_config.width;
						let height = window.surface_config.height;
						RenderSystem::render_game(
							&mut encoder,
							&mut render_resource,
							camera,
							camera_transform,
							width,
							height,
							RenderSystem::get_render_data(
								&models,
								&maps,
								&transforms,
								camera,
								camera_transform,
							),
							&frame.texture,
						);
						render_resource.encode_durations.record(Instant::now() - encode_st);
					}

					// If should render ui
					if true {
						RenderSystem::render_ui(
							&mut encoder,
							&mut render_resource,
							&mut window,
							&frame_view,
						);
					}
					
					let submit_st = Instant::now();
					render_resource.render_instance.queue.submit(std::iter::once(encoder.finish()));
					render_resource.submit_durations.record(Instant::now() - submit_st);

					frame.present();
				},
				RenderTarget::Texture(_) => {
					todo!();
				},
			}
		}
	}
}
