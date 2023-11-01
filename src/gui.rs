use std::{time::{Instant, Duration}, sync::mpsc::{Receiver, SyncSender}};
use crossbeam_channel::Sender;
use egui::{self, plot::{Line, PlotPoints}, ComboBox};
use glam::Vec3;
use krender::{RenderContextKey, prelude::RenderContext};
use eks::prelude::*;
use wgpu_profiler::GpuTimerScopeResult;
use std::sync::mpsc::sync_channel;
use crate::{window::{WindowPropertiesAndSettings, GraphicsHandle}, game::{Game, ContextResource, TextureResource, OutputResolutionComponent}, input::{KeyDeduplicator, InputEvent}, ecs::{octree::{GPUChunkViewer, GPUChunkLoadingComponent}, loading::{ChunkLoadingComponent, ChunkLoadingResource}, model::MapMeshingComponent, modification::VoxelModifierComponent}, util::RingDataHolder};
use crate::ecs::*;




#[derive(Debug)]
pub struct GameWidget {
	context: Option<RenderContextKey>,
	
	display_texture: Option<egui::TextureId>,
	
	update_delay: Option<Duration>, // If none, then renders whenever the gui does
	last_update: Option<Instant>,
	pub update_times: RingDataHolder<Duration>,
	last_size: [f32; 2],

	deduplicator: KeyDeduplicator,
	game_input: Option<Sender<(InputEvent, Instant)>>,

	aspect: Option<f32>, // Aspect ratio for the widget (4.0 / 3.0, 16.0 / 9.0, and so on)
}
impl GameWidget {
	pub fn new() -> Self {
		Self {
			context: None,

			display_texture: None,
			last_size: [400.0; 2],

			update_delay: None, // Duration::from_secs_f32(1.0 / 30.0),
			last_update: None,
			update_times: RingDataHolder::new(30),

			deduplicator: KeyDeduplicator::new(),
			game_input: None,

			aspect: Some(4.0 / 3.0),
		}
	}

	pub fn input(&mut self, event: InputEvent, when: Instant) {
		trace!("Game widget input {event:?}");
		if let Some(sender) = self.game_input.as_ref() {
			// You should deduplicate this pls
			sender.send((event.into(), when)).unwrap();
		} else {
			warn!("Game input before sender init");
		}
	}

	pub fn release_keys(&mut self) {
		todo!("Use deduplicator to release all pressed keys")
	}

	pub fn context<'a>(&'a self, contexts: &'a ContextResource) -> Option<&'a RenderContext<Entity>> {
		self.context.and_then(move |key| contexts.contexts.get(key))
	}

	pub fn should_update(&self) -> bool {
		self.update_delay.is_none() || self.last_update.is_none() || self.last_update.unwrap().elapsed() >= self.update_delay.unwrap()
	}
	
	#[profiling::function]
	pub fn update(
		&mut self,
		graphics: &mut GraphicsHandle,
		game: &mut Game,
	) -> wgpu::CommandBuffer {
		if let Some(t) = self.last_update {
			self.update_times.insert(t.elapsed());
		}
		self.last_update = Some(Instant::now());

		// Create entity
		let context_key = *self.context.get_or_insert_with(|| {
			let (raw_input, game_input) = RawInputComponent::new();
			self.game_input = Some(game_input);

			let mut control_map = game.world.borrow::<ResMut<ControlMap>>();
			let movement = MovementComponent::new(&mut control_map);
			let modifier_comp = VoxelModifierComponent::new(&mut control_map);
			drop(control_map);

			let entity_id = game.world.spawn()
				.with(TransformComponent::new().with_position(Vec3::new(0.0, 0.0, 0.0)))
				.with(raw_input)
				.with(ControlComponent::new())
				.with(CameraComponent::new())
				.with(movement)
				.with(ChunkLoadingComponent::new(8))
				.with(GPUChunkLoadingComponent::new(4, 2))
				.with(GPUChunkViewer::new(3))
				.with(MapMeshingComponent::new(7, 2))
				.with(modifier_comp)
				.with(SSAOComponent::default())
				.finish();

			let mut contexts = game.world.borrow::<ResMut<ContextResource>>();
			
			let context = RenderContext::new("default context")
				.with_entity(entity_id);
			contexts.contexts.insert(context)
		});

		// Update size of display texture
		let entity = {
			let mut contexts = game.world.borrow::<ResMut<ContextResource>>();
			let context = contexts.contexts.get_mut(context_key).unwrap();
			context.entity.unwrap()
		};
		let width = self.last_size[0].round() as u32;
		let height = self.last_size[1].round() as u32;
		game.world.add_component(entity, OutputResolutionComponent {
			width, height, 
		});
		
		let command_buffer = game.render(context_key, &mut graphics.profiler);

		// Update (or create) egui texture handle	
		// Assumes that the texture is wgpu::TextureFormat::Rgba8UnormSrgb and wgpu::TextureUsages::TEXTURE_BINDING	(egui compatible)
		if let Some(context_key) = self.context {
			let contexts = game.world.borrow::<Res<ContextResource>>();
			let context = contexts.contexts.get(context_key).unwrap();

			if let Some(key) = context.texture("output_texture") {
				let textures = game.world.borrow::<Res<TextureResource>>();
				let texture = textures.textures.get(key).unwrap();
	
				if let Some(id) = self.display_texture {
					// Only do this if texture rebound
					// I don't know how to do that through
					// Maybe a texture can store a callback?
					// Or binding can have a generation? 
					// I will put this off for now
					// Hopefully it will not haunt me

					// Fuck it, do it every frame
					graphics.egui_renderer.update_egui_texture_from_wgpu_texture(
						&graphics.device, 
						&texture.binding().unwrap().view, 
						wgpu::FilterMode::Linear, 
						id,
					);
				} else {
					info!("Register game widget display");
					let id = graphics.egui_renderer.register_native_texture(
						&graphics.device, 
						&texture.binding().unwrap().view,
						wgpu::FilterMode::Linear,
					);
					self.display_texture = Some(id);
				}
			}
		}

		command_buffer
	}

	pub fn display(
		&mut self, 
		ui: &mut egui::Ui, 
		window_settings: &mut WindowPropertiesAndSettings,
	) {
		ui.vertical_centered_justified(|ui| {
			if self.context.is_none() {
				ui.label("Tracked entity not set!");
				ui.centered_and_justified(|ui| ui.spinner());
			} else if self.display_texture.is_none() {
				ui.label("Display texture not created!");
				ui.centered_and_justified(|ui| ui.spinner());
			} else {
				let mut size = ui.available_size();
				if let Some(a) = self.aspect {
					size.y = size.x / a; 
				}
				self.last_size = size.into();

				let display_texture = self.display_texture.unwrap();

				let game = ui.image(display_texture, size);		
				let interactions = game.interact(egui::Sense::click());
				if interactions.clicked() {
					info!("Capture cursor");
					window_settings.set_cursor_grab(true);
				};
				if interactions.secondary_clicked() {
					info!("Release cursor");
					window_settings.set_cursor_grab(false);
				}
			}
		});
	}
}


pub struct MapLoadingWidget;
impl MapLoadingWidget {
	pub fn display(
		ui: &mut egui::Ui,
		loading: &ChunkLoadingResource,
	) {
		ui.collapsing("Chunk Loading", |ui| {
			ui.label(format!("{} / {} jobs", loading.cur_generation_jobs, loading.max_generation_jobs));
			let av = loading.generation_durations.iter()
				.copied()
				.reduce(|a, v| a + v)
				.and_then(|d| Some(d.as_secs_f32() / loading.generation_durations.len() as f32))
				.unwrap_or(0.0);
			ui.label(format!("Average: {}ms", av * 1000.0));

			for (p, st) in loading.vec_generation_jobs.iter() {
				ui.label(format!("{:.2}ms - {p}", st.elapsed().as_secs_f32() * 1000.0));
			}
			
		});
	}
}


#[derive(Debug)]
pub struct RenderProfilingWidget {
	trace_path: String,
	errs: Option<String>,
}
impl RenderProfilingWidget {
	pub fn new() -> Self {
		Self { 
			trace_path: "/tmp/trace.json".to_string(), 
			errs: None,
		}
	}

	fn recursive_thing(ui: &mut egui::Ui, sr: &GpuTimerScopeResult) {
		ui.collapsing(&sr.label, |ui| {
			let ft = sr.time.end - sr.time.start;
			ui.label(format!("{:.10}s", ft));
			ui.label(format!("~{:.2}Hz", 1.0 / ft));
			for ns in sr.nested_scopes.iter() {
				Self::recursive_thing(ui, ns);
			}
		});		
	}

	pub fn display(&mut self, ui: &mut egui::Ui, profile_data: &Vec<GpuTimerScopeResult>) {
		let tft = profile_data.iter().fold(0.0, |a, p| a + (p.time.end - p.time.start));

		ui.label(format!("Frame: {:>4.1}ms, {}Hz", tft * 1000.0, (1.0 / tft).round()));
		ui.collapsing("Frame Details", |ui| {
			ui.label(format!("{tft:.10}s"));
			for sr in profile_data {
				Self::recursive_thing(ui, sr);
			}

			ui.text_edit_singleline(&mut self.trace_path);

			let mut text = egui::RichText::new("Output Trace File");
			if self.errs.is_some() {
				text = text.color(egui::Color32::RED);
			}
			let mut button = ui.button(text);
			if let Some(es) = self.errs.as_ref() {
				button = button.on_hover_text(es);
			}
			if button.clicked() {
				self.errs = wgpu_profiler::chrometrace::write_chrometrace(std::path::Path::new(&*self.trace_path), profile_data).err().and_then(|e| Some(e.to_string()));
			}
		});
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
		self.messages.retain(|(_, t)| *t > now);

		// List popups
		ui.scope(|ui| {
			ui.visuals_mut().override_text_color = Some(egui::Color32::RED);
			ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
  			ui.style_mut().wrap = Some(false);
			ui.style_mut().text_styles.iter_mut().for_each(|(_, font_id)| font_id.size = 8.0);
			egui::ScrollArea::vertical()
				.auto_shrink([false, false])
				.max_height(128.0)
				.show(ui, |ui| {
					self.messages.iter().for_each(|(message, _)| {
						ui.label(message);
					});
				});
		});
		
	}
}


enum SplineInterpolation {
	Cosine,
	Bezier([f64; 2]),
}


use splines::{Spline, Key};
pub struct SplineWidget {
	keys: Vec<Key<f64, f64>>,
	resolution: usize,
}
impl SplineWidget {
	pub fn new(resolution: usize) -> Self {
		Self { 
			keys: vec![
				Key::new(0.1, 10.0, splines::Interpolation::Cosine),
				Key::new(0.5, 50.0, splines::Interpolation::Cosine),
				Key::new(0.7, 15.0, splines::Interpolation::Cosine),
			], 
			resolution,
		}
	}

	fn spline(&self) -> Option<Spline<f64, f64>> {
		if self.keys.is_empty() {
			None
		} else {
			Some(Spline::from_vec(self.keys.clone()))
		}
	}

	fn points(&self) -> egui::plot::Points {
		let points = self.keys.iter()
			.map(|k| [k.t, k.value])
			.collect::<Vec<_>>();
		egui::plot::Points::new(points)
			.name("Spline Points")
			.shape(egui::plot::MarkerShape::Circle)
			.filled(true)
			.radius(5.0)
	}
	
	pub fn display(&mut self, ui: &mut egui::Ui) {
		ui.horizontal_top(|ui| {
			// Needs to be before plot or else the thing grows infinitely
			ui.vertical(|ui| {
				if ui.button("Add Point").clicked() {
					let k = if let Some(k) = self.keys.last() {
						Key::new(k.t + 1.0, k.value, splines::Interpolation::Cosine)
					} else {
						Key::new(0.0, 0.0, splines::Interpolation::Cosine)
					};
					debug!("Poosh");
					self.keys.push(k);
					self.keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
				}
				// ui.collapsing("Points:", |ui| {
					let mut changed = false;
					let mut to_remove = Vec::new(); // Could retain for less allocation
					for (i, k)in self.keys.iter_mut().enumerate() {
						ui.horizontal(|ui| {
							if ui.add(egui::DragValue::new(&mut k.t).prefix("x: ").speed(0.1)).changed() {
								changed = true;
							}
							ui.add(egui::DragValue::new(&mut k.value).prefix("y: ").speed(0.1));
							ComboBox::from_id_source(i)
								.selected_text(format!("{:?}", k.interpolation))
								.show_ui(ui, |ui| {
									ui.selectable_value(&mut k.interpolation, splines::Interpolation::Cosine, "Cosine");
									ui.selectable_value(&mut k.interpolation, splines::Interpolation::Bezier(0.0), "Bezier");
								});
							if let splines::Interpolation::Bezier(v) = &mut k.interpolation {
								ui.add(egui::DragValue::new(v).prefix("y': ").speed(0.1));
							}
							
							if ui.button("X").clicked() {
								to_remove.push(i);
							}
						});
					}
					for i in to_remove {
						debug!("Remove index {i}");
						self.keys.remove(i);
					}
					if changed {
						self.keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
					}
				// });
			});
			
			let plot = egui::plot::Plot::new("Spline Editor Plot")
				.legend(egui::plot::Legend::default());
			plot.show(ui, |ui| {
				if let Some(spline) = self.spline() {
					ui.line(Line::new(PlotPoints::from_explicit_callback(
						move |t| spline.sample(t).unwrap_or(0.0), 
						.., 
						256,
					)).name("Spline Line"));
				}

				ui.points(self.points());

				// let size = ui.transform().frame();

				// for k in self.keys.iter_mut() {
				// 	let p = PlotPoint::new(k.t, k.value);
				// 	let point_in_screen = ui.screen_from_plot(p);
				// 	// If in rect?
				// 	let point_rect = Rect::from_center_size(point_in_screen, Vec2::new(8.0, 8.0));
					
				// 	// let point_response = ui.interact(point_rect, ui.id(), Sense::drag());

				// 	// k.t += point_response.drag_delta().x as f64;
				// 	// k.value += point_response.drag_delta().y as f64;

				// }

				// Derive plot bounds
				// ui.plot_bounds()
				
				
				// ui.screen_from_plot(position)
				// pui.plot_bounds().
				
				

				// let point_rect = Rect::from_center_size(point_in_screen, size);

				// let point_response = ui.interact(point_rect, point_id, Sense::drag());
				// let stroke = ui.style().interact(&point_response).fg_stroke;
				// ui.painter().circle(Pos2::new(8.0, 8.0), 5.0, Color32::RED, stroke)

					
			});
			
			

			// let desired_size = ui.available_size_before_wrap();
			// let (rect, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
			// painter.
			

			// if ui.is_rect_visible(rect) {
			// 	ui.painter().circle(center, 5.0, Color32::RED, stroke)
			// }


			// // Can we paint some interactable circles overtop of the plot?
			// let (response, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 300.0), Sense::hover());
			// let circle = Shape::circle_filled(center, radius, fill_color);
			
			// let resp = ui.interact(circle.visual_bounding_rect(), id, Sense::drag());
			// v += resp.drag_delta();


			// painter.add(circle)
		});
		

	}
}


pub struct SSAOWidget;
impl SSAOWidget {
	pub fn display(ui: &mut egui::Ui, ssao: &mut SSAOComponent) {
		ui.horizontal(|ui| {
			ui.vertical(|ui| {
				ui.style_mut().text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = 15.15;
				ui.label("output scale:");
				ui.label("noise tiling:");
				ui.label("contrast:");
				ui.label("bias:");
				ui.label("radius:");
			});
			
			ui.vertical(|ui| {
				ui.add(egui::Slider::new(&mut ssao.output_settings.scale, 0.0..=1.0));
				ui.add(egui::Slider::new(&mut ssao.render_settings.tile_scale, 0.0001..=32.0).step_by(1.0));
				ui.add(egui::Slider::new(&mut ssao.render_settings.contrast, 0.1..=2.0));
				ui.add(egui::Slider::new(&mut ssao.render_settings.bias, 0.0..=0.1));
				ui.add(egui::Slider::new(&mut ssao.render_settings.radius, 0.0..=5.0));
			});
		});
	}
}
