use std::{time::Instant, sync::mpsc::{Receiver, SyncSender}};
use bytemuck::{Pod, Zeroable};
use egui;
use specs::Entity;
use std::sync::mpsc::sync_channel;
use crate::{render::*, ecs::GPUResource, world::{TracingChunkManager, BlockManager, load_blocks_file_messy, Chunk}, octree::chunk_to_octree};
use crate::window::WindowSettings;




// use egui::Widget;


#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy)]
struct Camera {
	position: [f32; 4],
	rotation: [[f32; 4]; 4],
	near: f32,
}



#[derive(Debug)]
pub struct GameWidget {
	pub tracked_entity: Option<Entity>,
	rgba_texture: Option<BoundTexture>,
	srgba_texture: Option<BoundTexture>,
	display_texture: Option<egui::TextureId>,
	last_size: [f32; 2],
	conversion_sampler: Option<wgpu::Sampler>,
	fugg_buffer: Option<wgpu::Buffer>,
	
	bm: BlockManager,
	tcm: Option<TracingChunkManager>,

	aspect: Option<f32>, // Aspect ratio for the widget (4.0 / 3.0, 16.0 / 9.0, and so on)

	camera_buffer: Option<wgpu::Buffer>,
}
impl GameWidget {
	pub fn new(
		tracked_entity: Option<Entity>,
	) -> Self {

		let mut bm = BlockManager::new();
		load_blocks_file_messy(
			"./resources/kblocks.ron",
			&mut bm,
		);

		Self {
			tracked_entity,
			rgba_texture: None,
			srgba_texture: None,
			display_texture: None,
			last_size: [400.0; 2],
			conversion_sampler: None,
			fugg_buffer: None,

			aspect: None,

			bm,
			tcm: None,

			camera_buffer: None,
		}
	}

	pub fn encode_render(
		&mut self,
		encoder: &mut wgpu::CommandEncoder,
		world: &specs::World,
		gpu_resource: &mut GPUResource,
	) {
		use specs::WorldExt;
		use crate::ecs::*;
		use wgpu::util::DeviceExt;

		if let Some(entity) = self.tracked_entity {

			// Camera
			let ccs = world.read_component::<CameraComponent>();
			let _camera = ccs.get(entity)
				.expect("Render point has no camera!");
			let tcs = world.read_component::<TransformComponent>();
			let camera_transform = tcs.get(entity)
				.expect("Render camera has no transform!");
			let camera_data = Camera {
				position: camera_transform.position.to_homogeneous().into(),
				rotation: camera_transform.rotation.to_homogeneous().into(),
				near: 1.0 / (90.0_f32.to_radians() / 2.0).tan(),
			};


			// Textures and stuff
			self.update_size(&gpu_resource.device);
			let rgba = self.rgba_texture.as_ref().unwrap();
			let srgba = self.srgba_texture.as_ref().unwrap();
			let fugg_buffer = self.fugg_buffer.get_or_insert_with(|| {
				info!("Making fugg buffer");
				gpu_resource.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some("fugg buffer"),
					contents: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
					usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
				})
			});
			let conversion_sampler = self.conversion_sampler.get_or_insert_with(|| {
				info!("Making conversion sampler");
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


			// Camera
			let camera_buffer = self.camera_buffer.get_or_insert_with(|| {
				info!("Making camera buffer");
				println!("{:#?}", camera_data.rotation);
				gpu_resource.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some("camera buffer"),
					contents: bytemuck::bytes_of(&camera_data),
					usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
				})
			});
			gpu_resource.queue.write_buffer(&*camera_buffer, 0, bytemuck::bytes_of(&camera_data));


			let comp_shader = gpu_resource.data.shaders.index(0);
			let cp_bind_group = gpu_resource.device.create_bind_group(&wgpu::BindGroupDescriptor {
				label: Some("compute bind group"),
				layout: gpu_resource.data.shaders.bind_group_layout_index(comp_shader.bind_groups[&0].layout_idx),
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&rgba.view),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Buffer(camera_buffer.as_entire_buffer_binding()),
					},
				],
			});
			let tcm = self.tcm.get_or_insert_with(|| {
				let mut tcm = TracingChunkManager::new(&gpu_resource.device);
				
				let chunk = Chunk::from_compressed_mapped_rle(
					"./map_saved/0/-3.-2.1.cmrle", 
					[16; 3], &mut self.bm
				).unwrap();
				let octree = chunk_to_octree(&chunk).unwrap();
				
				warn!("Chunk uses  {} bytes", chunk.size());
				warn!("Octree uses {} bytes", octree.get_size());

				tcm.insert_octree(&gpu_resource.queue, [0,0,2], &octree);
				warn!("Buffer is at {:.2}% capacity", tcm.storage.capacity_frac() * 100.0);
				tcm.insert_octree(&gpu_resource.queue, [1,0,2], &octree);
				warn!("Buffer is at {:.2}% capacity", tcm.storage.capacity_frac() * 100.0);
				
				
				tcm
			});
			// tcm.chunks.insert([0,1,1], (0, 0));
			// tcm.chunks.insert([0,0,1], (0, 0));
			tcm.rebuild(&gpu_resource.queue, [0,0,0]);
			let tcmbg = tcm.make_bg(&gpu_resource.device, gpu_resource.data.shaders.bind_group_layout_index(comp_shader.bind_groups[&1].layout_idx));
			{
				let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: Some("compute pass"),
				});
				
				cp.set_pipeline(comp_shader.pipeline.unwrap_compute());

				cp.set_bind_group(0, &cp_bind_group, &[]);
				cp.set_bind_group(1, &tcmbg, &[]);
				
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

			match self.display_texture {
				Some(id) => {
					gpu_resource.egui_rpass.update_egui_texture_from_wgpu_texture(
						&gpu_resource.device, 
						&srgba.view, 
						wgpu::FilterMode::Nearest, 
						id,
					).unwrap();
				},
				None => {
					self.display_texture = Some(gpu_resource.egui_rpass.egui_texture_from_wgpu_texture(
						&gpu_resource.device,
						&srgba.view,
						wgpu::FilterMode::Nearest, 
					));
				},
			};

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
			texture.replace(BoundTexture::new(
				device, 
				format,
				intended_size[0], 
				intended_size[1], 
				"GameWidgetSource",
			));
			true
		}

		// let indended_width = self.last_size[0].round() as u32;
		// let height = (self.last_size[0] * 9.0 / 16.0).round() as u32;
		let intended_size = self.last_size.map(|f| f.round() as u32);
		// let intended_size = [indended_width, height];

		update_size_internal(device, &mut self.rgba_texture, intended_size, TextureFormat::Rgba8Unorm);
		update_size_internal(device, &mut self.srgba_texture, intended_size, TextureFormat::Rgba8UnormSrgb);
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
