
struct RenderResource {
	renderer: Renderer,
}
impl RenderResource {
	pub fn new(instance: &wgpu::Instance, adapter: &wgpu::Adapter) -> Self {
		let renderer = pollster::block_on(Renderer::new(adapter));

		Self {
			renderer,
		}		
	}
}






// Processes new events
struct WindowEventSystem;
impl<'a> System<'a> for WindowEventSystem {
    type SystemData = (
		WriteExpect<'a, WindowResource>,
		WriteExpect<'a, InputResource>,
		ReadExpect<'a, SimulationResource>
	);

    fn run(&mut self, (mut window_resource, mut input_resource, simulation_resource): Self::SystemData) {
		// Pressed keys for this timestep
		let mut keymap = HashMap::new();
		// Duration of this timestep
		
	fn process_queue(&mut self) {
		// resource.process
	}
}

