use std::{time::Instant, sync::mpsc::{Receiver, SyncSender}};
use egui;
use specs::Entity;
use std::sync::mpsc::sync_channel;
use crate::render::*;




// use egui::Widget;



pub struct GameWidget {
	pub tracked_entity: Option<Entity>,
	source_texture: Option<BoundTexture>,
	display_texture: Option<egui::TextureId>,
	size: [f32; 2],
}
impl GameWidget {
	pub fn new(tracked_entity: Option<Entity>) -> Self {
		Self {
			tracked_entity,
			source_texture: None,
			display_texture: None,
			size: [400.0; 2]
		}
	}

	/// To be used if one wished to display an image independently of other systems.
	pub fn set_source(&mut self, source: BoundTexture) {
		self.source_texture = Some(source);
	}

	pub fn get_source<'a>(&'a mut self, device: &wgpu::Device) -> &'a BoundTexture {
		let intended_size = self.size.map(|f| f.round() as u32);
		if self.source_texture.is_some() {
			let source = self.source_texture.as_ref().unwrap();
			let source_size = [source.size.width, source.size.height];
			if intended_size == source_size {
				return self.source_texture.as_ref().unwrap()
			}
		} 
		self.source_texture = Some(BoundTexture::new(
			device, 
			TextureFormat::Rgba8UnormSrgb,
			intended_size[0], 
			intended_size[1], 
			"GameWidgetSource",
		));
		self.source_texture.as_ref().unwrap()
	}

	pub fn update_display(
		&mut self,
		rpass: &mut egui_wgpu_backend::RenderPass, 
		device: &wgpu::Device,
	) {
		self.display_texture = self.source_texture.as_ref().and_then(|st| {
			Some(rpass.egui_texture_from_wgpu_texture(
				device,
				&st.view,
				wgpu::FilterMode::Nearest, 
			))
		});
	}

	pub fn display(&mut self, ui: &mut egui::Ui) {

		if self.source_texture.is_none() {
			ui.label("Source texture not created!");
		}

		if self.tracked_entity.is_none() {
			ui.label("Tracked entity not set!");
		}
		
		if let Some(tid) = self.display_texture {
			let [width, height] = self.size;
			ui.image(tid, egui::vec2(width, height));
		}

		
	}
}




pub struct PopupWidget {
	popups: Vec<(String, Instant)>,
	receiver: Receiver<(String, Instant)>,
	sender: SyncSender<(String, Instant)>,
}
impl PopupWidget {
	pub fn new() -> Self {

		let (sender, receiver) = sync_channel(100);

		Self {
			popups: Vec::new(),
			receiver,
			sender,
		}
	}

	pub fn new_sender(&self) -> SyncSender<(String, Instant)> {
		self.sender.clone()
	}

	pub fn display(&mut self, ui: &mut egui::Ui) {
		// Get new popups
		self.popups.extend(self.receiver.try_iter());

		// Remove expired popups
		let now = Instant::now();
		self.popups.drain_filter(|(_, t)| *t < now);

		// List popups
		ui.scope(|ui| {
			ui.visuals_mut().override_text_color = Some(egui::Color32::RED);
			ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
  			ui.style_mut().wrap = Some(false);
			
			self.popups.iter().for_each(|(message, _)| {
				ui.label(message);
			});
		});
		
	}
}
