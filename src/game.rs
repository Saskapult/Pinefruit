use crossbeam_channel::Receiver;
use glam::{Vec3, Mat4};
use krender::prelude::Mesh;
use krender::{RenderContextKey, prepare_for_render};
use wgpu_profiler::GpuProfiler;
use winit::event_loop::*;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::time::{Instant, Duration};
use std::sync::Arc;
use crate::ecs::*;
use crate::ecs::loading::{map_loading_system, ChunkLoadingResource};
use crate::ecs::model::{map_modelling_system, map_model_rendering_system, MapModelResource};
use crate::ecs::modification::{map_modification_system, map_placement_system};
use crate::ecs::octree::{gpu_chunk_loading_system, chunk_rays_system, BigBufferResource, GPUChunksResource, block_colours_system};
use crate::rendering_integration::WorldWrapper;
use crate::util::RingDataHolder;
use crate::voxel::load_all_blocks_in_file;
use crate::window::*;
use eks::prelude::*;
use krender::prelude::*;



#[derive(Debug, ResourceIdent)]
pub struct DeviceResource(Arc<wgpu::Device>);
impl Deref for DeviceResource {
	type Target = Arc<wgpu::Device>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}


#[derive(Debug, ResourceIdent)]
pub struct QueueResource(Arc<wgpu::Queue>);
impl Deref for QueueResource {
	type Target = Arc<wgpu::Queue>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}


#[derive(Debug, ResourceIdent, Default)]
pub struct MaterialResource { pub materials: MaterialManager }
impl Deref for MaterialResource {
	type Target = MaterialManager;
	fn deref(&self) -> &Self::Target {
		&self.materials
	}
}
impl DerefMut for MaterialResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.materials
	}
}


#[derive(Debug, ResourceIdent)]
pub struct BufferResource { pub buffers: BufferManager }
impl Deref for BufferResource {
	type Target = BufferManager;
	fn deref(&self) -> &Self::Target {
		&self.buffers
	}
}
impl DerefMut for BufferResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.buffers
	}
}


#[derive(Debug, ResourceIdent, Default)]
pub struct TextureResource { pub textures: TextureManager }
impl Deref for TextureResource {
	type Target = TextureManager;
	fn deref(&self) -> &Self::Target {
		&self.textures
	}
}
impl DerefMut for TextureResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.textures
	}
}


#[derive(Debug, ResourceIdent, Default)]
pub struct MeshResource { pub meshes: MeshManager }
impl Deref for MeshResource {
	type Target = MeshManager;
	fn deref(&self) -> &Self::Target {
		&self.meshes
	}
}
impl DerefMut for MeshResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.meshes
	}
}


#[derive(Debug, ResourceIdent, Default)]
pub struct ContextResource { pub contexts: RenderContextManager<Entity> }
impl Deref for ContextResource {
	type Target = RenderContextManager<Entity>;
	fn deref(&self) -> &Self::Target {
		&self.contexts
	}
}
impl DerefMut for ContextResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.contexts
	}
}


/// Sent to the window manager to track game status
pub enum GameStatus {
	Exit(i32),
	Continue(Instant), // Time of next tick 
	// Maybe include time to next tick
	// Or instant of next tick
	// Or tick deficit?
	// Argh
}


#[derive(Debug)]
pub struct TickData {
	pub start: Instant,
	pub end: Instant,
}
impl TickData {
	pub fn delta(&self) -> Duration {
		self.end.duration_since(self.start)
	}
}


pub struct Game {
	pub world: World,
	
	next_tick: u64,
	first_tick: Option<Instant>,
	tick_period: Duration,
	pub tick_times: RingDataHolder<TickData>,

	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,

	commands_receiver: Receiver<GameCommand>,
	commands_sender: EventLoopProxy<WindowCommand>,

	shaders: ShaderManager,
	bind_groups: BindGroupManager,

	pub render_rays: bool,
	pub render_polygons: bool,
}
impl Game {
	/// Creating the game should be fast because it is done on the main thread. 
	/// Intensive work should be saved for [Game::initialize]
	pub fn new(
		device: Arc<wgpu::Device>,
		queue: Arc<wgpu::Queue>,
		commands_receiver: Receiver<GameCommand>,
		commands_sender: EventLoopProxy<WindowCommand>,
	) -> Self {
		let shaders = ShaderManager::new();
		let bind_groups = BindGroupManager::new();

		let materials = MaterialManager::new();
		let textures = TextureManager::new();
		let buffers = BufferManager::new();
		let meshes = MeshManager::new();
		let contexts = RenderContextManager::new();

		let mut world = World::new();

		world.insert_resource(MaterialResource { materials });
		world.insert_resource(TextureResource { textures });
		world.insert_resource(BufferResource { buffers });
		world.insert_resource(MeshResource { meshes });
		world.insert_resource(ContextResource { contexts });
		world.insert_resource(DeviceResource(device.clone()));
		world.insert_resource(QueueResource(queue.clone()));

		Self {
			world,
			next_tick: 0,
			first_tick: None,
			tick_period: Duration::from_secs_f32(1.0 / 30.0),
			tick_times: RingDataHolder::new(30),
			device, queue, 
			commands_receiver, commands_sender, 
			shaders, bind_groups, 
			render_rays: false,
			render_polygons: true,
		}
	}

	/// Every setup thing that is computationally expensive should go here. 
	pub fn initialize(&mut self) {
		self.world.insert_resource(ControlMap::new());
		
		self.world.insert_resource(MapResource::new());

		self.world.insert_resource(MapModelResource::new(8));

		self.world.insert_resource({
			let blocks = BlockResource::default();
			let mut g = blocks.write();
			let mut materials = self.world.borrow::<ResMut<MaterialResource>>();
			load_all_blocks_in_file(&mut g, "resources/kblocks.ron", &mut materials).unwrap();
			drop(g);
			blocks
		});

		self.world.insert_resource(ChunkLoadingResource::new(42));

		{ // Octree thing
			let r = GPUChunksResource::default();
			self.world.insert_resource(r);
		}
		
		{ // Big buffer
			let mut buffers = self.world.borrow::<ResMut<BufferResource>>();
			let big_buffer = BigBufferResource::new(&mut buffers);
			drop(buffers);
			self.world.insert_resource(big_buffer);
		}

		let material = {
			let mut materials = self.world.borrow::<ResMut<MaterialResource>>();
			materials.read("resources/materials/grass.ron")
		};
		let mesh = {
			let mut meshes = self.world.borrow::<ResMut<MeshResource>>();
			meshes.read_or("resources/meshes/box.obj", || Mesh::read_obj("resources/meshes/box.obj"))
		};
		self.world.spawn()
			.with(TransformComponent::new()
				.with_position(Vec3::new(0.0, 0.0, 5.0)))
			.with(ModelComponent {
				material,
				mesh,
			})
			.with(ModelMatrixComponent::new())
			.finish();

	}
	
	// pub fn intended_tick(&self) -> u64 {
	// 	self.next_tick + self.last_tick
	// 		.and_then(|t| Some(t.elapsed().div_f32(self.tick_period.as_secs_f32()).as_secs_f32().floor() as u64))
	// 		.unwrap_or(0)
	// }

	// pub fn time_of_tick(&self, tick: u64) -> Instant {
	// 	let diff = self.next_tick as f64 - tick as f64;
	// 	self.last_tick
	// 		.and_then(|t| Some(t + self.tick_period.mul_f64(diff)))
	// 		.unwrap_or(Instant::now())
	// }

	#[profiling::function]
	pub fn tick(&mut self) -> GameStatus {
		while let Ok(command) = self.commands_receiver.try_recv() {
			info!("Game receives command {command:?}");
			match command {
				GameCommand::Shutdown => return GameStatus::Exit(0),
				_ => {},
			}
		}

		info!("Tick {}", self.next_tick);
		let tick_start = Instant::now();
		self.first_tick.get_or_insert(tick_start);
		
		self.world.run(raw_control_system);
		self.world.run(movement_system);
		self.world.run(map_placement_system);

		self.world.run(map_loading_system);
		self.world.run(map_modification_system);

		if self.render_rays {
			self.world.run(gpu_chunk_loading_system); // Could be moved to render, but that'd give frame out of date issues
		}
		if self.render_polygons {
			self.world.run(map_modelling_system);
		}

		self.world.run(model_matrix_system);

		let tick_end = Instant::now();
		info!("Ticked in {}ms", tick_end.duration_since(tick_start).as_millis());

		self.tick_times.insert(TickData { start: tick_start, end: tick_end });
		self.next_tick += 1;

		let next_tick_time = self.first_tick.unwrap() + self.tick_period.mul_f64(self.next_tick as f64);
		GameStatus::Continue(next_tick_time)
	}

	pub fn new_window(&mut self) {
		info!("Requesting new game window");
		self.commands_sender.send_event(WindowCommand::NewWindow).unwrap();
	}

	// In the final thing we'd run a bunch of scripts which do stuff
	// In this version we will just insert a result texture
	#[profiling::function]
	pub fn render(
		&mut self, 
		context: RenderContextKey, 
		profiler: &mut GpuProfiler,
	) -> wgpu::CommandBuffer {
		let render_st = Instant::now();
		
		// Render resource systems
		{
			let mut contexts = self.world.borrow::<ResMut<ContextResource>>();
			let context_mut = contexts.contexts.render_contexts.get_mut(context).unwrap();
			self.world.run_with_data((context_mut,), output_texture_system);

			let context_mut = contexts.contexts.render_contexts.get_mut(context).unwrap();
			self.world.run_with_data((context_mut,), context_albedo_system);

			let context_mut = contexts.contexts.render_contexts.get_mut(context).unwrap();
			self.world.run_with_data((context_mut,), context_camera_system);

			self.world.run(block_colours_system);
		}

		// Retain this?
		// In a resource?
		// In the context? That does seem best
		let mut input = RenderInput::new();
		// Collect stuff

		let d = {
			let textures = self.world.borrow::<Res<TextureResource>>();
			textures.key_by_name(&"depth".to_string()).unwrap()
		};
		input.clear_depth("models", d);
		
		{ // Render skybox
			let mut materials = self.world.borrow::<ResMut<MaterialResource>>();
			let skybox_mtl = materials.read("resources/materials/skybox.ron");
			input.insert_item("skybox", skybox_mtl, None, Entity::default());
		}

		input.add_dependency("models", "skybox");
		self.world.run_with_data((&mut input,), model_render_system);
		
		// Render chunk meshes
		if self.render_polygons {
			self.world.run_with_data((&mut input,), map_model_rendering_system);
		}

		// Render chunks with rays
		if self.render_rays {
			input.add_dependency("voxels", "skybox");
			input.add_dependency("models", "voxels");
		
			let mut contexts = self.world.borrow::<ResMut<ContextResource>>();
			let context_mut = contexts.contexts.render_contexts.get_mut(context).unwrap();
			self.world.run_with_data((context_mut, &mut input), chunk_rays_system);
		}

		input.add_dependency("ssao generate", "models");
		input.add_dependency("ssao apply", "ssao generate");
		{
			let mut contexts = self.world.borrow::<ResMut<ContextResource>>();
			let context_mut = contexts.contexts.render_contexts.get_mut(context).unwrap();
			self.world.run_with_data((context_mut, &mut input), ssao_system);
		}
		
		let mut materials = self.world.borrow::<ResMut<MaterialResource>>();
		let mut meshes = self.world.borrow::<ResMut<MeshResource>>();
		let mut textures = self.world.borrow::<ResMut<TextureResource>>();
		let mut buffers = self.world.borrow::<ResMut<BufferResource>>();
		let mut contexts = self.world.borrow::<ResMut<ContextResource>>();
		prepare_for_render(
			&self.device, 
			&self.queue, 
			&mut self.shaders, 
			&mut materials.materials, 
			&mut meshes.meshes, 
			&mut textures.textures, 
			&mut buffers.buffers, 
			&mut self.bind_groups, 
			&mut contexts.contexts,
		);

		let storage_provider = WorldWrapper { world: &self.world, };
		let bundle = input.bundle(&self.device, &mut meshes.meshes, &materials.materials, &self.shaders, context, &storage_provider);

		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: None,
		});

		bundle.execute(&self.shaders, &self.bind_groups, &meshes.meshes, &textures.textures, &mut encoder, &self.device, profiler);

		let buf = encoder.finish();

		let render_dur = render_st.elapsed();
		info!("Encoded render in {:.1}ms", render_dur.as_secs_f32() * 1000.0);

		buf
	}
}


#[derive(Debug, ComponentIdent)]
pub struct OutputResolutionComponent {
	pub width: u32,
	pub height: u32,
}


pub fn output_texture_system(
	(context,): (&mut RenderContext<Entity>,), 
	mut textures: ResMut<TextureResource>,
	output_resolutions: Comp<OutputResolutionComponent>,
) {
	if let Some(entity) = context.entity {
		if let Some(resolution) = output_resolutions.get(entity) {
			if let Some(key) = context.texture("output_texture") {
				// If resolution matches then terminate
				let t = textures.get_mut(key).unwrap();
				if resolution.width == t.size.width && resolution.height == t.size.height {
					return
				}

				info!("Rebuild output texure to size {}x{}", resolution.width, resolution.height);
				t.set_size(resolution.width, resolution.height, 1);

				// This is bad
				let k = textures.key_by_name(&"depth".to_string()).unwrap();
				let d = textures.get_mut(k).unwrap();
				d.set_size(resolution.width, resolution.height, 1);
			} else {
				let t = Texture::new(
					"output_texture", 
					wgpu::TextureFormat::Rgba8UnormSrgb.into(), 
					resolution.width, resolution.height, 
					1, false, false, 
				).with_usages(wgpu::TextureUsages::TEXTURE_BINDING);
				let key = textures.insert(t);
				context.insert_texture("output_texture", key);

				let d = textures.insert(Texture::new(
					"depth", 
					wgpu::TextureFormat::Depth32Float.into(), 
					resolution.width, resolution.height, 
					1, false, false, 
				).with_usages(wgpu::TextureUsages::RENDER_ATTACHMENT));
				context.insert_texture("depth", d);
			}
		}
	}
}


fn model_render_system(
	(input,): (&mut RenderInput<Entity>,), 
	models: Comp<ModelComponent>,
) {
	for (entity, (model,)) in (&models,).iter().with_entities() {
		input.insert_item("models", model.material, Some(model.mesh), entity);
	}
}


// Used to put transform into shader
#[repr(C)]
#[derive(Debug, ComponentIdent)]
pub struct ModelMatrixComponent {
	pub model_matrix: Mat4,
}
impl ModelMatrixComponent {
	pub fn new() -> Self {
		Self {
			model_matrix: Mat4::IDENTITY,
		}
	}
}


fn model_matrix_system(
	transforms: Comp<TransformComponent>,
	mut model_matrices: CompMut<ModelMatrixComponent>,
) {
	for (transform, model_matrix) in (&transforms, &mut model_matrices).iter() {
		model_matrix.model_matrix = Mat4::from_rotation_translation(transform.rotation, transform.translation);
	}
}
