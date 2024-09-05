use std::path::Path;

use crate::client::GameInstance;


const CONSOLE_HISTORY_FILE: &str = ".console_history";


#[derive(Default)]
pub struct ConsoleWidget {
	// Everything that should appear in the console's log
	log: Vec<(String, egui::Color32)>,
	// Commands the user has entered 
	history: Vec<String>,
	input: String,
	// 0 => input, 1.. => history[i]
	input_history: usize,
}
impl ConsoleWidget {
	pub fn new() -> Self {
		let mut s = Self::default();
		if Path::new(CONSOLE_HISTORY_FILE).exists() {
			debug!("Loading console history file");
			let contents = std::fs::read_to_string(Path::new(CONSOLE_HISTORY_FILE)).unwrap();
			match ron::de::from_str(&contents) {
				Ok(v) => s.history = v,
				Err(e) => error!("Failed to load console history: {}", e),
			}
		}
		s
	}

	pub fn show(
		&mut self, 
		ui: &mut egui::Ui, 
		instance: &mut GameInstance,
	) {
		ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
		ui.style_mut().visuals.override_text_color = Some(egui::Color32::DEBUG_COLOR);

		egui::ScrollArea::vertical()
		.stick_to_bottom(true)
		.max_height(ui.available_height() - 3.0 * ui.text_style_height(&egui::TextStyle::Monospace))
		.max_width(f32::INFINITY)
		.auto_shrink([false, false])
		.show_rows(ui, ui.text_style_height(&egui::TextStyle::Monospace), self.log.len(), |ui, row_range| {
			for i in row_range {
				let (s, c) = &self.log[i];
				ui.label(egui::RichText::new(s).color(*c).monospace());
			}
		});

		if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
			self.input_history = self.history.len().min(self.input_history+1);
			if self.input_history > 0 {
				self.input = self.history[self.input_history-1].clone();
			}
		}
		if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
			self.input_history = self.input_history.saturating_sub(1);
			if self.input_history > 0 {
				self.input = self.history[self.input_history-1].clone();
			} else if self.input_history == 0 {
				self.input.clear();
			}
		}

		let r = egui::TextEdit::singleline(&mut self.input)
			.code_editor()
			.text_color(egui::Color32::DEBUG_COLOR)
			.desired_width(f32::INFINITY)
			.hint_text("...")
			.show(ui);
		if r.response.lost_focus() {
			self.commit(instance);
		}
	}

	fn commit(
		&mut self,
		instance: &mut GameInstance,
	) {
		self.history.push(self.input.clone());
		self.backup_history();

		let registry = &mut instance.extensions;
		let world = &mut instance.world;

		let parts = self.input.split(" ").collect::<Vec<_>>();
		match registry.command(world, parts.as_slice()) {
			Ok(v) => {
				self.log.push((self.input.clone(), egui::Color32::LIGHT_GREEN));
				self.log.push((v, egui::Color32::GREEN))
			},
			Err(e) => {
				self.log.push((self.input.clone(), egui::Color32::LIGHT_RED));
				self.log.push((format!("{}", e), egui::Color32::RED))
			},
		}
		self.input.clear();
	}

	fn backup_history(&mut self) {
		self.history.dedup();
		let contents = ron::ser::to_string_pretty(&self.history, ron::ser::PrettyConfig::default()).unwrap();
		std::fs::write(Path::new(CONSOLE_HISTORY_FILE), contents).unwrap();
	}
}


