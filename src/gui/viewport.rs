use std::time::{Duration, Instant};
use krender::{prelude::RenderInput, prepare_for_render, RenderContextKey};
use render::{ActiveContextResource, BufferResource, ContextResource, DeviceResource, MaterialResource, MeshResource, OutputResolutionComponent, QueueResource, RenderInputResource, TextureResource};
use slotmap::SecondaryMap;
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};
use ekstensions::prelude::*;
use crate::{client::GameInstance, GraphicsHandle};



#[derive(Debug)]
pub struct ViewportWidget {
	pub context: RenderContextKey,
	// Could hold Arc<Mutex<ViewportManager>> in order to drop automatically 
}
impl ViewportWidget {
	// Should this create the context? 
	// It will be writing to the context. 
	// Maybe viewports are a secondarymap of contexts!
	pub fn new(context: RenderContextKey, viewports: &mut ViewportManager) -> Self {
		assert!(!viewports.contexts.contains_key(context), "Tried to have two viewports for one context!");

		viewports.contexts.insert( context, ViewportEntry {
			context,
			display_texture: None,
			update_delay: None,
			last_update: None,
			profiler: GpuProfiler::new(GpuProfilerSettings::default()).unwrap(),
			last_profiler_frame: None,
			// update_times: RingDataHolder::new(30),
			last_size: [400.0, 300.0],
			aspect: None,
		});
		Self { context }
	}

	pub fn show(&mut self, ui: &mut egui::Ui, viewports: &mut ViewportManager) -> egui::Response {
		let viewport = viewports.contexts.get_mut(self.context).unwrap();

		let mut size = ui.available_size();
		if let Some(a) = viewport.aspect {
			size.y = size.x / a; 
		}

		// ViewportWidget must trigger a redraw if the size of its display area changed
		viewport.last_size = size.into();

		// Allocate space and show texture there
		let (rect, response) = ui.allocate_at_least(size, egui::Sense::click());
		if let Some(texture_id) = viewport.display_texture {
			let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
			ui.painter().image(texture_id, rect, uv, egui::Color32::WHITE);
		} else {
			ui.label("Display texture not initialized");
		}

		response
	}
}


/// ViewportWidgets show textures from these, and are responsible for changing the settings. 
/// Be careful to have only one ViewportWidget setitng the resolution of the texture! 
/// The window checks if any of these must be redrawn when it redraws. 
/// 
/// Stored in a Arc<Mutex<Vec<Self>>> in a window to allow for command buffer batching. 
struct ViewportEntry {
	context: RenderContextKey,
	// Becuase context textures are not bound until after a render, this can be none
	display_texture: Option<egui::TextureId>,

	update_delay: Option<Duration>,
	last_update: Option<Instant>,

	profiler: wgpu_profiler::GpuProfiler,
	last_profiler_frame: Option<Vec<GpuTimerQueryResult>>,
	// update_times: RingDataHolder<Duration>,

	last_size: [f32; 2],
	aspect: Option<f32>, 
}
impl ViewportEntry {
	fn should_update(&self) -> bool {
		self.update_delay.is_none() || self.last_update.is_none() || self.last_update.unwrap().elapsed() >= self.update_delay.unwrap()
	}

	pub fn update<'a>(
		&'a mut self, 
		graphics: &mut GraphicsHandle,
		instance: &mut GameInstance, // Replace with (render)world? 
	) -> (wgpu::CommandBuffer, &'a mut wgpu_profiler::GpuProfiler) {
		profiling::scope!("Viewport Update");

		// Record update
		if let Some(t) = self.last_update {
			// self.update_times.insert(t.elapsed());
		}
		self.last_update = Some(Instant::now());

		// Update size of display texture
		let entity: Entity = {
			let mut contexts = instance.world.query::<ResMut<ContextResource>>();
			let context = contexts.get_mut(self.context).unwrap();
			context.entity.unwrap()
		};
		let width = self.last_size[0].round() as u32;
		let height = self.last_size[1].round() as u32;
		instance.world.add_component(entity, OutputResolutionComponent {
			width, height, 
		});

		// Render game
		instance.world.insert_resource(ActiveContextResource { key: self.context });
		instance.world.insert_resource(RenderInputResource(RenderInput::new()));

		instance.extensions.run(&mut instance.world, "render").unwrap();

		let input = instance.world.remove_resource_typed::<RenderInputResource>().unwrap().0;
		
		let b = {
			let device = instance.world.query::<Res<DeviceResource>>();
			let queue = instance.world.query::<Res<QueueResource>>();
			let mut materials = instance.world.query::<ResMut<MaterialResource>>();
			let mut meshes = instance.world.query::<ResMut<MeshResource>>();
			let mut textures = instance.world.query::<ResMut<TextureResource>>();
			let mut buffers = instance.world.query::<ResMut<BufferResource>>();
			let mut contexts = instance.world.query::<ResMut<ContextResource>>();

			prepare_for_render(
				&device, 
				&queue, 
				&mut instance.shaders, 
				&mut materials, 
				&mut meshes, 
				&mut textures, 
				&mut buffers, 
				&mut instance.bind_groups, 
				&mut contexts,
			);
			
			let context = contexts.get(self.context).unwrap();
			let bundle = {
				profiling::scope!("Render Bundle");
				input.bundle(
					&device, 
					&textures, 
					&mut meshes, 
					&materials, 
					&instance.shaders, 
					&instance.world, 
					&context, 
					true,
				)
			};

			meshes.bind_unbound(&device);
			
			let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: None,
			});

			buffers.do_writes(&queue, &mut encoder);
			textures.do_writes(&queue, &mut encoder);

			{
				profiling::scope!("Render Execute");
				bundle.execute(
					&device, 
					&mut instance.shaders, 
					&mut instance.bind_groups, 
					&meshes, 
					&textures, 
					&mut encoder, 
					&mut self.profiler,
				);
			}

			self.profiler.resolve_queries(&mut encoder);

			encoder.finish()
		};

		// (Re)Register texture
		let contexts = instance.world.query::<Res<ContextResource>>();
		let context = contexts.get(self.context).unwrap();
		let texture_key = context.texture("output_texture").unwrap();
		let textures = instance.world.query::<Res<TextureResource>>();
		let texture = textures.get(texture_key).unwrap();

		if let Some(id) = self.display_texture {
			graphics.egui_renderer.update_egui_texture_from_wgpu_texture(
				&graphics.device, 
				texture.view().unwrap(), 
				wgpu::FilterMode::Linear, 
				id,
			);
		} else {
			let id = graphics.egui_renderer.register_native_texture(
				&graphics.device, 
				texture.view().unwrap(),
				wgpu::FilterMode::Linear,
			);
			self.display_texture = Some(id);
		}

		// `end_frame` must be called only after work has been submitted
		(b, &mut self.profiler)
	}
}


#[derive(Default)]
pub struct ViewportManager {
	contexts: SecondaryMap<RenderContextKey, ViewportEntry>,
}
impl ViewportManager {
	// Output option dt? 
	pub fn is_tick_needed(
		&mut self,
	) -> bool {
		self.contexts.iter().any(|(_, v)| v.should_update())
	}

	/// Output command buffers for each viewport to be redrawn. 
	pub fn update_viewports(
		&mut self, 
		graphics: &mut GraphicsHandle, 
		instance: &mut GameInstance,
	) -> Vec<(wgpu::CommandBuffer, &mut wgpu_profiler::GpuProfiler)> {
		self.contexts.iter_mut()
			.filter(|(_, v)| v.should_update())
			.map(|(_, v)| v.update(graphics, instance))
			.collect()
	}

	/// Shows profiling data for each viewport. 
	pub fn show_viewport_profiling(
		&mut self, 
		ui: &mut egui::Ui,
		graphics: &GraphicsHandle, 
	) {
		for (i, (_, v)) in self.contexts.iter_mut().enumerate() {
			ui.collapsing(format!("Viewport {}", i), |ui| {
				// Check for new profiler data
				if let Some(new_profiler_frame) = v.profiler.process_finished_frame(graphics.queue.get_timestamp_period()) {
					v.last_profiler_frame = Some(new_profiler_frame);
				}

				// Display last profiler data
				if let Some(frame) = v.last_profiler_frame.as_ref() {
					let s = frame.iter()
						.map(|r| r.time.end - r.time.start)
						.sum::<f64>();

					ui.label(format!("{:.4}ms ({:.1}HZ)", s * 1000.0, 1.0 / s));
					show_frame_rec(ui, frame);
				} else {
					ui.label("No frame data (yet)");
				}
			});
		}
	}
}


fn show_frame_rec(ui: &mut egui::Ui, frame: &Vec<GpuTimerQueryResult>) {
	for r in frame {
		ui.collapsing(&r.label, |ui| {
			ui.label(format!("{:.4}ms", (r.time.end - r.time.start) * 1000.0));
			show_frame_rec(ui, &r.nested_queries);
		});
	}
}
