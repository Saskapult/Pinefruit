pub mod profiling;
pub mod viewport;

use std::time::Instant;
use controls::{ControlComponent, InputEvent, LocalInputComponent};
use crossbeam_channel::Sender;
use eeks::prelude::*;
use player::PlayerSpawnResource;
use render::{CameraComponent, ContextResource, SSAOComponent};
use crate::window::WindowPropertiesAndSettings;
use self::viewport::{ViewportManager, ViewportWidget};



#[derive(Debug)]
pub struct GameWidget {
	viewport: ViewportWidget,

	client_input: Sender<(InputEvent, Instant)>,
}
impl GameWidget {
	pub fn new(
		world: &mut World, 
		viewports: &mut ViewportManager,
		entity: Entity,
	) -> Self {
		// Insert input component
		// Other storages, like ControlMap and MovementComponent, are assumed to be present 
		// Also ControlComponent but I don't remember what that is
		let (input_component, client_input) = LocalInputComponent::new();
		{
			if world.query::<CompMut<LocalInputComponent>>().insert(entity, input_component).is_some() {
				warn!("Overwrote a LocalInputComponent for entity {:?}", entity);
			}
		}

		// Create render context
		// Insert required components for that
		// Not sure what should be here and what should be in the spawn function
		{ // Camera
			world.query::<CompMut<CameraComponent>>()
				.insert(entity, CameraComponent::new());
		}
		{ // SSAO
			world.query::<CompMut<SSAOComponent>>()
				.insert(entity, SSAOComponent::default());
		}
		{ // Control
			world.query::<CompMut<ControlComponent>>()
				.insert(entity, ControlComponent::new());
		}
		{ // Add to player spawn queue
			world.query::<ResMut<PlayerSpawnResource>>().entities.insert(entity);
		}
		let context = {
			let mut contexts = world.query::<ResMut<ContextResource>>();
			
			let (key, context) = contexts.new_context("default context");
			context.entity = Some(entity);

			key
		};

		let viewport = ViewportWidget::new(context, viewports);

		Self {
			viewport,
			client_input,
		}
	}

	pub fn input(&mut self, event: impl Into<InputEvent>, when: Instant) {
		// Send to client
		self.client_input.send((event.into(), when)).unwrap();
	}

	pub fn release_keys(&mut self) {
		todo!("Use deduplicator to release all pressed keys")
	}

	pub fn show(
		&mut self, 
		ui: &mut egui::Ui, 
		window_settings: &mut WindowPropertiesAndSettings,
		viewports: &mut ViewportManager,
	) {
		let response = self.viewport.show(ui, viewports);
		if response.clicked() {
			info!("Capture cursor");
			window_settings.set_cursor_grab(true);
		};
		if response.secondary_clicked() {
			info!("Release cursor");
			window_settings.set_cursor_grab(false);
		}
	}
}


pub fn show_workgroup_info(ui: &mut egui::Ui, registry: &ExtensionRegistry) {
	let wg_info = registry.workload_info();

	for (name, systems, order) in wg_info {
		ui.collapsing(name, |ui| {
			ui.horizontal(|ui| {
				for (i, stage) in order.iter().enumerate() {
					ui.vertical(|ui| {
						ui.heading(format!("Stage {}", i));
						
						for item in stage {
							let (name, _) = systems[*item];
							ui.label(format!("{}", name));
						}
					});
				}
			});
		});
	}
	// Get run order
	// Get edges between nodes
}


// pub struct MapLoadingWidget;
// impl MapLoadingWidget {
// 	pub fn display(
// 		ui: &mut egui::Ui,
// 		loading: &TerrainLoadingResource,
// 	) {
// 		ui.collapsing("Chunk Loading", |ui| {
// 			ui.label(format!("{} / {} jobs", loading.cur_generation_jobs, loading.max_generation_jobs));
// 			let av = loading.generation_durations.iter()
// 				.copied()
// 				.reduce(|a, v| a + v)
// 				.and_then(|d| Some(d.as_secs_f32() / loading.generation_durations.len() as f32))
// 				.unwrap_or(0.0);
// 			ui.label(format!("Average: {}ms", av * 1000.0));

// 			for (p, st) in loading.vec_generation_jobs.iter() {
// 				ui.label(format!("{:.2}ms - {p}", st.elapsed().as_secs_f32() * 1000.0));
// 			}
			
// 		});
// 	}
// }


// #[derive(Debug)]
// pub struct RenderProfilingWidget {
// 	trace_path: String,
// 	errs: Option<String>,
// }
// impl RenderProfilingWidget {
// 	pub fn new() -> Self {
// 		Self { 
// 			trace_path: "/tmp/trace.json".to_string(), 
// 			errs: None,
// 		}
// 	}

// 	fn recursive_thing(ui: &mut egui::Ui, sr: &GpuTimerQueryResult) {
// 		ui.collapsing(&sr.label, |ui| {
// 			let ft = sr.time.end - sr.time.start;
// 			ui.label(format!("{:.10}s", ft));
// 			ui.label(format!("~{:.2}Hz", 1.0 / ft));
// 			for ns in sr.nested_queries.iter() {
// 				Self::recursive_thing(ui, ns);
// 			}
// 		});		
// 	}

// 	pub fn display(&mut self, ui: &mut egui::Ui, profile_data: &Vec<GpuTimerQueryResult>) {
// 		let tft = profile_data.iter().fold(0.0, |a, p| a + (p.time.end - p.time.start));

// 		ui.label(format!("Frame: {:>4.1}ms, {}Hz", tft * 1000.0, (1.0 / tft).round()));
// 		ui.collapsing("Frame Details", |ui| {
// 			ui.label(format!("{tft:.10}s"));
// 			for sr in profile_data {
// 				Self::recursive_thing(ui, sr);
// 			}

// 			ui.text_edit_singleline(&mut self.trace_path);

// 			let mut text = egui::RichText::new("Output Trace File");
// 			if self.errs.is_some() {
// 				text = text.color(egui::Color32::RED);
// 			}
// 			let mut button = ui.button(text);
// 			if let Some(es) = self.errs.as_ref() {
// 				button = button.on_hover_text(es);
// 			}
// 			if button.clicked() {
// 				self.errs = wgpu_profiler::chrometrace::write_chrometrace(std::path::Path::new(&*self.trace_path), profile_data).err().and_then(|e| Some(e.to_string()));
// 			}
// 		});
// 	}
// }


// #[derive(Debug)]
// pub struct MessageWidget {
// 	messages: Vec<(String, Instant)>,
// 	receiver: Receiver<(String, Instant)>,
// 	sender: SyncSender<(String, Instant)>,
// }
// impl MessageWidget {
// 	pub fn new() -> Self {

// 		let (sender, receiver) = sync_channel(100);

// 		Self {
// 			messages: Vec::new(),
// 			receiver,
// 			sender,
// 		}
// 	}

// 	pub fn new_sender(&self) -> SyncSender<(String, Instant)> {
// 		self.sender.clone()
// 	}

// 	pub fn add_message(&mut self, message: impl Into<String>, remove_after: Instant) {
// 		self.messages.push((message.into(), remove_after));
// 	}

// 	pub fn display(&mut self, ui: &mut egui::Ui) {
// 		// Get new popups
// 		self.messages.extend(self.receiver.try_iter());

// 		// Remove expired popups
// 		let now = Instant::now();
// 		self.messages.retain(|(_, t)| *t > now);

// 		// List popups
// 		ui.scope(|ui| {
// 			ui.visuals_mut().override_text_color = Some(egui::Color32::RED);
// 			ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
//   			ui.style_mut().wrap = Some(false);
// 			ui.style_mut().text_styles.iter_mut().for_each(|(_, font_id)| font_id.size = 8.0);
// 			egui::ScrollArea::vertical()
// 				.auto_shrink([false, false])
// 				.max_height(128.0)
// 				.show(ui, |ui| {
// 					self.messages.iter().for_each(|(message, _)| {
// 						ui.label(message);
// 					});
// 				});
// 		});
		
// 	}
// }


// pub struct SSAOWidget;
// impl SSAOWidget {
// 	pub fn display(ui: &mut egui::Ui, ssao: &mut SSAOComponent) {
// 		ui.horizontal(|ui| {
// 			ui.vertical(|ui| {
// 				ui.style_mut().text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = 15.15;
// 				ui.label("output scale:");
// 				ui.label("noise tiling:");
// 				ui.label("contrast:");
// 				ui.label("bias:");
// 				ui.label("radius:");
// 			});
			
// 			ui.vertical(|ui| {
// 				ui.add(egui::Slider::new(&mut ssao.output_settings.scale, 0.0..=1.0));
// 				ui.add(egui::Slider::new(&mut ssao.render_settings.tile_scale, 0.0001..=32.0).step_by(1.0));
// 				ui.add(egui::Slider::new(&mut ssao.render_settings.contrast, 0.1..=2.0));
// 				ui.add(egui::Slider::new(&mut ssao.render_settings.bias, 0.0..=0.1));
// 				ui.add(egui::Slider::new(&mut ssao.render_settings.radius, 0.0..=5.0));
// 			});
// 		});
// 	}
// }
