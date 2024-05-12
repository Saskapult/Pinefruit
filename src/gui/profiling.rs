// This could be bitflags, but that would allow for server with other flags and that would be bad
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, variantly::Variantly)]
pub enum ProfilingMode {
	#[default]
	Disabled,
	Server,
	Client,
	Render,
	ClientRender,
}
impl ProfilingMode {
	pub fn enable_server(&self) -> bool {
		self.is_server()
	}
	
	pub fn enable_client(&self) -> bool {
		self.is_client() || self.is_client_render()
	}

	pub fn enable_render(&self) -> bool {
		self.is_render() || self.is_client_render()
	}

	pub fn frame_post_client(&self) -> bool {
		self.is_client()
	}

	pub fn frame_post_render(&self) -> bool {
		self.is_render() || self.is_client_render()
	}

	pub fn frame_post_server(&self) -> bool {
		self.is_server()
	}
}


pub struct ProfilingWidget {
	pub profiling_mode: ProfilingMode,
	pub show_profiling: bool,
	// puffin_thread: Option<>
}
impl ProfilingWidget {
	pub fn new() -> Self {
		Self {
			profiling_mode: ProfilingMode::Client,
			show_profiling: false,
		}
	}

	pub fn show_options(&mut self, ui: &mut egui::Ui) {
		ui.horizontal(|ui| {
			ui.toggle_value(&mut self.show_profiling, "Profiling");
			ui.menu_button(format!("{:?}", self.profiling_mode), |ui| {
				let clicked = [
					ProfilingMode::Disabled,
					ProfilingMode::Server,
					ProfilingMode::Render,
					ProfilingMode::Client,
					ProfilingMode::ClientRender,
				].into_iter().map(|mode| {
					ui.selectable_value(&mut self.profiling_mode, mode, format!("{:?}", mode)).clicked()
				}).any(|b| b);
				if clicked {
					ui.close_menu();
				}
			});
		});
	}

	pub fn show_profiler(&mut self, ctx: &egui::Context) {
		if self.show_profiling {
			egui::Window::new(format!("Profiling: {:?}", self.profiling_mode))
			.id(egui::Id::new("Profiling"))
			.open(&mut self.show_profiling)
			.show(ctx, |ui| {
				profiling::puffin::set_scopes_on(true);
				puffin_egui::profiler_ui(ui);
				profiling::puffin::set_scopes_on(false);
			});
		}
	}
}
