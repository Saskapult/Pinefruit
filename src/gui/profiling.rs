
// Should be a config option? That way we can remove puffin_http dependency
const PUFFIN_SERVER: bool = true;


// This could be bitflags, but that would allow for server with other flags and that would be bad
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, variantly::Variantly)]
pub enum ProfilingMode {
	#[default]
	Disabled,
	Server,
	Client,
}


pub struct ProfilingWidget {
	pub profiling_mode: ProfilingMode,
	pub show_profiling: bool,

	puffin_server: Option<puffin_http::Server>,

	display_bug_workaround_done: bool,
}
impl ProfilingWidget {
	pub fn new() -> Self {
		Self {
			profiling_mode: ProfilingMode::Client,
			show_profiling: false,
			puffin_server: None,
			display_bug_workaround_done: false,
		}
	}

	/// puffin_egui and puffin_http will not display scopes that were in frames finished before puffin_egui and puffin_http were started. 
	/// This was a horrible thing to debug. 
	/// We can get around this by starting both things before the first frame is finished, which is what this function does. 
	pub fn display_bug_workaround(&mut self, ctx: &egui::Context) {
		if !self.display_bug_workaround_done {
			if PUFFIN_SERVER {
				let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
				debug!("Start puffin server on {}", server_addr);
				self.puffin_server = Some(puffin_http::Server::new(&server_addr).unwrap());
			}

			let temp = self.show_profiling;
			self.show_profiling = true;
			self.show_profiler(ctx);
			self.show_profiling = temp;

			self.display_bug_workaround_done = true;
		}
	}

	pub fn show_options(&mut self, ui: &mut egui::Ui) {
		ui.horizontal(|ui| {
			ui.toggle_value(&mut self.show_profiling, "Profiling");
			ui.menu_button(format!("{:?}", self.profiling_mode), |ui| {
				let clicked = [
					ProfilingMode::Disabled,
					ProfilingMode::Server,
					ProfilingMode::Client,
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
				// profiling::puffin::set_scopes_on(true);
				puffin_egui::profiler_ui(ui);
				// profiling::puffin::set_scopes_on(false);
			});
		}
	}
}
