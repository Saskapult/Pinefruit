use egui;
use specs::Entity;

use crate::render::BoundTexture;


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
			size: [100.0; 2]
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
		self.display_texture = Some(rpass.egui_texture_from_wgpu_texture(
			device,
			&self.source_texture.as_ref().unwrap().view,
			wgpu::FilterMode::Nearest, 
		));
	}

	pub fn thing(&mut self, ui: &mut egui::Ui) {
		ui.label("GAME GOES HERE PLEASE");

		if self.source_texture.is_none() {
			ui.label("Source texture not created!");
		}

		if self.tracked_entity.is_none() {
			ui.label("Tracked entity not set!");
		}

		// let texture = &*self.world_view.get_or_insert_with(|| {
		// 	// Load the texture only once.
		// 	ui.ctx().load_texture("my-image", egui::ColorImage::example())
		// });
		
		if let Some(tid) = self.display_texture {
			let [width, height] = self.size;
			ui.image(tid, egui::vec2(width, height));
		}

		
	}
}

