use ekstensions::prelude::*;
use glam::Vec3;
use krender::{prelude::*, MaterialKey, MeshKey};
use render::{MaterialResource, MeshResource, RenderInputResource};
use transform::TransformComponent;

#[macro_use]
extern crate log;



#[derive(Component, Debug)]
pub struct ModelComponent {
	pub material: MaterialKey,
	pub mesh: MeshKey,
}


fn skybox_render_system(
	mut materials: ResMut<MaterialResource>,
	mut input: ResMut<RenderInputResource>,
) {
	input.stage("skybox")
		.clear_depth(RRID::context("depth"))
		.clear_colour(RRID::context("albedo"));

	let skybox_mtl = materials.read("resources/materials/skybox.ron");
	input.stage("skybox")
		.target(AbstractRenderTarget::new()
			.with_colour(RRID::context("albedo"), None)
			.with_depth(RRID::context("depth")))
		.push((skybox_mtl, None, Entity::default()));	

	input.add_dependency("models", "skybox");
}


fn model_render_system(
	// context: Res<ActiveContextResource>,
	// mut contexts: ResMut<ContextResource>, 
	models: Comp<ModelComponent>,
	mut input: ResMut<RenderInputResource>,
) {
	// let context = contexts.get_mut(context.key).unwrap();

	let items = input.stage("models")
		.target(AbstractRenderTarget::new()
			.with_colour(RRID::context("albedo"), None)
			.with_depth(RRID::context("depth")));
	for (entity, (model,)) in (&models,).iter().with_entities() {
		items.push((model.material, Some(model.mesh), entity));
	}

	if models.len() != 0 {
		input.add_dependency("ssao generate", "models");
	}
}


fn spawn_test_model(
	mut entities: EntitiesMut,
	mut models: CompMut<ModelComponent>,
	mut meshes: ResMut<MeshResource>,
	mut materials: ResMut<MaterialResource>,
	mut transforms: CompMut<TransformComponent>,
) {
	let material = materials.read("resources/materials/grass.ron");
	let mesh = meshes.read_or("resources/meshes/box.obj", || Mesh::read_obj("resources/meshes/box.obj"));

	for p in [
		Vec3::new(0.0, 0.0, 0.0),
		Vec3::new(0.0, 0.0, 1.0),
		Vec3::new(0.0, 1.0, 0.0),
		Vec3::new(1.0, 0.0, 0.0),
		Vec3::new(0.0, -10.0, 0.0),
	] {
		let entity = entities.spawn();
		models.insert(entity, ModelComponent { material, mesh, });
		transforms.insert(entity, TransformComponent::new().with_position(p));
	}
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn dependencies() -> Vec<String> {
	env_logger::init();
	vec![]
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("render", "model_render_system", model_render_system);
	loader.system("render", "skybox_render_system", skybox_render_system);

	loader.system("client_init", "spawn_test_model", spawn_test_model);
}


#[cfg_attr(feature = "extension", no_mangle)]
pub fn load(p: &mut ekstensions::ExtensionStorageLoader) {
	p.component::<ModelComponent>();
}
