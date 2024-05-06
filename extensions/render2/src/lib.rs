use ekstensions::prelude::*;
use krender::{prelude::*, MaterialKey, MeshKey};
use render::{MaterialResource, RenderInputResource};

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
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn dependencies() -> Vec<String> {
	vec![]
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn systems(loader: &mut ExtensionSystemsLoader) {
	loader.system("render", "model_render_system", model_render_system);
	loader.system("render", "skybox_render_system", skybox_render_system);
}


#[cfg_attr(not(feature = "no_export"), no_mangle)]
pub fn load(p: &mut ekstensions::ExtensionStorageLoader) {
	p.component::<ModelComponent>();
}

