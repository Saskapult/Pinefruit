use std::{time::Instant, sync::mpsc::{Receiver, SyncSender}};
use egui;
use specs::Entity;
use std::sync::mpsc::sync_channel;
use crate::{render::*, ecs::GPUResource};




// use egui::Widget;


#[derive(Debug)]
pub struct GameWidget {
	pub tracked_entity: Option<Entity>,
	rgba_texture: Option<BoundTexture>,
	srgba_texture: Option<BoundTexture>,
	display_texture: Option<egui::TextureId>,
	last_size: [f32; 2],
	conversion_sampler: Option<wgpu::Sampler>,
	fug_buffer: Option<wgpu::Buffer>,
}
impl GameWidget {
	pub fn new(
		tracked_entity: Option<Entity>,
	) -> Self {
		Self {
			tracked_entity,
			rgba_texture: None,
			srgba_texture: None,
			display_texture: None,
			last_size: [400.0; 2],
			conversion_sampler: None,
			fug_buffer: None,
		}
	}

	pub fn encode_render(
		&mut self,
		encoder: &mut wgpu::CommandEncoder,
		_world: &specs::World,
		gpu_resource: &mut GPUResource,
	) {
		// use specs::WorldExt;
		if let Some(_entity) = self.tracked_entity {
			self.update_size(&gpu_resource.device);
			let rgba = self.rgba_texture.as_ref().unwrap();
			let srgba = self.srgba_texture.as_ref().unwrap();
			
			let fugg_buffer = self.fug_buffer.get_or_insert_with(|| {
				use wgpu::util::DeviceExt;
				gpu_resource.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some("fugg Buffer"),
					contents: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
					usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
				})
			});
			let conversion_sampler = self.conversion_sampler.get_or_insert_with(|| {
				gpu_resource.device.create_sampler(&wgpu::SamplerDescriptor {
					label: Some("conversion sampler"),
					address_mode_u: wgpu::AddressMode::ClampToEdge,
					address_mode_v: wgpu::AddressMode::ClampToEdge,
					address_mode_w: wgpu::AddressMode::ClampToEdge,
					mag_filter: wgpu::FilterMode::Linear,
					min_filter: wgpu::FilterMode::Linear,
					mipmap_filter: wgpu::FilterMode::Nearest,
					..Default::default()
				})
			});


			let comp_shader = gpu_resource.data.shaders.index(0);
			let cp_bind_group = gpu_resource.device.create_bind_group(&wgpu::BindGroupDescriptor {
				label: Some("compute bind group"),
				layout: gpu_resource.data.shaders.bind_group_layout_index(comp_shader.bind_groups[&0].layout_idx),
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&rgba.view),
					},
				],
			});
			{
				let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: Some("compute pass"),
				});
				
				match &comp_shader.pipeline {
					crate::render::ShaderPipeline::Compute(pipeline) => cp.set_pipeline(pipeline),
					_ => panic!("Weird shader things"),
				}

				cp.set_bind_group(0, &cp_bind_group, &[]);
				
				cp.dispatch_workgroups(rgba.size.width / 16 + 1, rgba.size.height / 16 + 1, 1);
			}


			let blit_shader = gpu_resource.data.shaders.index(1);
			let bp_bind_group = gpu_resource.device.create_bind_group(&wgpu::BindGroupDescriptor {
				label: Some("blit bind group"),
				layout: gpu_resource.data.shaders.bind_group_layout_index(blit_shader.bind_groups[&0].layout_idx),
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&rgba.view),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(conversion_sampler),
					},
				],
			});
			{
				let mut bp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None,
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &srgba.view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Load,
							store: true,
						},
					})],
					depth_stencil_attachment: None,
				});

				match &blit_shader.pipeline {
					crate::render::ShaderPipeline::Polygon{pipeline, ..} => bp.set_pipeline(pipeline),
					_ => panic!("Weird shader things"),
				}

				bp.set_vertex_buffer(0, fugg_buffer.slice(..));
				bp.set_vertex_buffer(1, fugg_buffer.slice(..));

				bp.set_bind_group(0, &bp_bind_group, &[]);

				bp.draw(0..3, 0..1);
			}


			// let ccs = world.read_component::<CameraComponent>();
			// let camera = ccs.get(entity)
			// 	.expect("Render point has no camera!");
			// let tcs = world.read_component::<TransformComponent>();
			// let camera_transform = tcs.get(entity)
			// 	.expect("Render camera has no transform!");

			// gpu_resource.set_data(camera.render_data.clone());
			// let render_camera = crate::render::Camera {
			// 	position: camera_transform.position,
			// 	rotation: camera_transform.rotation,
			// 	fovy: camera.fovy,
			// 	znear: camera.znear,
			// 	zfar: camera.zfar,
			// };
			// gpu_resource.encode_render(
			// 	&mut encoder,
			// 	&self.game_thing.get_source(&gpu_resource.device).texture, 
			// 	self.surface_config.width, 
			// 	self.surface_config.height, 
			// 	&render_camera, 
			// 	Instant::now(),
			// );

			// let cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
			// 	label: Some("CP"),
			// });

			// let bt = crate::render::BoundTexture::from_image(
			// 	&gpu_resource.device, 
			// 	&gpu_resource.queue, 
			// 	img, 
			// 	"Testyy", 
			// 	true,
			// );

			self.display_texture = Some(gpu_resource.egui_rpass.egui_texture_from_wgpu_texture(
				&gpu_resource.device,
				&srgba.view,
				wgpu::FilterMode::Nearest, 
			));
		}
	}

	/// To be used if one wished to display an image independently of other systems.
	pub fn set_source(&mut self, source: BoundTexture) {
		self.srgba_texture = Some(source);
	}

	/// Adjusts the texture size to the widget size
	pub fn update_size(&mut self, device: &wgpu::Device) {
		fn update_size_internal(device: &wgpu::Device, texture: &mut Option<BoundTexture>, intended_size: [u32; 2], format: TextureFormat) -> bool {
			if let Some(texture) = texture.as_ref() {
				let srgba_size = [texture.size.width, texture.size.height];
				if srgba_size == intended_size {
					return false;
				}
			}
			texture.insert(BoundTexture::new(
				device, 
				format,
				intended_size[0], 
				intended_size[1], 
				"GameWidgetSource",
			));
			true
		}

		let intended_size = self.last_size.map(|f| f.round() as u32);

		update_size_internal(device, &mut self.rgba_texture, intended_size, TextureFormat::Rgba8Unorm);
		update_size_internal(device, &mut self.srgba_texture, intended_size, TextureFormat::Rgba8UnormSrgb);
	}
	

	pub fn get_source<'a>(&'a mut self, device: &wgpu::Device) -> &'a BoundTexture {
		let intended_size = self.last_size.map(|f| f.round() as u32);
		if self.srgba_texture.is_some() {
			let source = self.srgba_texture.as_ref().unwrap();
			let source_size = [source.size.width, source.size.height];
			if intended_size == source_size {
				return self.srgba_texture.as_ref().unwrap()
			}
			info!("Resizing GameWidget source texture ({source_size:?} -> {intended_size:?})")
		} 
		self.srgba_texture = Some(BoundTexture::new(
			device, 
			TextureFormat::Rgba8UnormSrgb,
			intended_size[0], 
			intended_size[1], 
			"GameWidgetSource",
		));
		self.srgba_texture.as_ref().unwrap()
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

	pub fn display(&mut self, ui: &mut egui::Ui) {

		if self.tracked_entity.is_none() {
			ui.label("Tracked entity not set!");
		} else if self.rgba_texture.is_none() {
			ui.label("RGBA texture not created!");
		} else if self.srgba_texture.is_none() {
			ui.label("SRGBA texture not created!");
		}
		
		if let Some(tid) = self.display_texture {
			self.last_size = ui.available_size().into();
			ui.image(tid, ui.available_size());
		}
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

	pub fn add_message(&mut self, message: String, remove_after: Instant) {
		self.messages.push((message, remove_after));
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
