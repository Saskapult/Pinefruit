pub mod controls;
pub mod player;
pub mod render;
pub mod time;
pub mod transform;

use controls::{local_control_system, ControlComponent, ControlMap, LocalInputComponent};
use eeks::prelude::*;
use player::{player_spawn, player_spawn_components, player_spawned, PlayerSpawnResource};
use render::{context_albedo_system, context_camera_system, model_render_system, output_texture_system, skybox_render_system, spawn_test_model, ssao_system, AlbedoOutputComponent, CameraComponent, ModelComponent, OutputResolutionComponent, RenderTargetSizeComponent, SSAOComponent};
use time::{time_buffer_system, time_update_system, TimeResource};
use transform::{movement_system, MovementComponent, TransformComponent};

#[macro_use]
extern crate log;


#[info]
pub fn dependencies() -> Vec<String> {
	vec![]
}


#[systems]
pub fn systems(loader: &mut ExtensionSystemsLoader) {	
	loader.system("client_tick", "local_control_system", local_control_system);

	loader.system("client_tick", "player_spawn", player_spawn);
	loader.system("client_tick", "player_spawned", player_spawned)
		.run_after("player_spawn");
	// Smooshed system is run between the other two
	loader.system("client_tick", "player_spawn_components", player_spawn_components)
		.run_after("player_spawn")
		.run_before("player_spawned");

	loader.system("render", "context_albedo_system", context_albedo_system);
	loader.system("render", "context_camera_system", context_camera_system)
		.run_after("output_texture");
	loader.system("render", "ssao_system", ssao_system)
		.run_after("output_texture");
	loader.system("render", "output_texture", output_texture_system);
	loader.system("render", "model_render_system", model_render_system);
	loader.system("render", "skybox_render_system", skybox_render_system);
	loader.system("client_init", "spawn_test_model", spawn_test_model);

	loader.system("client_tick", "time_buffer_system", time_buffer_system)
		.run_after("time_update_system");
	loader.system("client_tick", "time_update_system", time_update_system);

	loader.system("client_tick", "movement_system", movement_system)
		.run_after("local_control_system");
}


#[load]
pub fn load(storages: &mut ExtensionStorageLoader) {
	storages.component::<ControlComponent>();
	storages.component::<LocalInputComponent>();
	storages.resource::<ControlMap>(ControlMap::new());

	storages.resource(PlayerSpawnResource::default());

	storages.component::<CameraComponent>();
	storages.component::<RenderTargetSizeComponent>();
	storages.component::<OutputResolutionComponent>();
	storages.component::<SSAOComponent>();
	storages.component::<AlbedoOutputComponent>();
	// Resources are inserted and managed by the main program, as it is the one to acquire the graphics handle 
	storages.component::<ModelComponent>();

	storages.resource(TimeResource::new());

	storages.component::<TransformComponent>();
	storages.component::<MovementComponent>();
}