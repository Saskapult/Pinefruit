//! Krender is here becuse my build times were awful.
//! It is extensible I hope.

use std::{collections::{BTreeMap, HashMap}, hash::Hash};
use bindgroup::BindGroupManager;
use material::MaterialManager;
use mesh::MeshManager;
use prelude::BufferManager;
use rendercontext::RenderContextManager;
use shader::ShaderManager;
use slotmap::new_key_type;
use texture::TextureManager;
use wgpu::util::DeviceExt;

use crate::vertex::FetchedInstanceAttributeSource;

mod shader;
mod mesh;
mod buffer;
mod texture;
mod bindgroup;
pub mod vertex;
mod material;
mod rendertarget;
mod input;
mod rendercontext;
mod util;
pub mod allocator;
mod bundle;
pub mod input_v2;

pub mod prelude {
	pub use crate::input::RenderInput;
	pub use crate::input_v2::RenderInput2;
	pub use tobj;
	pub use crate::texture::{Texture, TextureManager, TextureFormat};
	pub use crate::mesh::{Mesh, MeshManager};
	pub use crate::buffer::{BufferManager, Buffer};
	pub use crate::vertex::{InstanceAttributeSource, FetchedInstanceAttributeSource};
	pub use crate::shader::{ShaderManager, ShaderEntry};
	pub use crate::bindgroup::BindGroupManager;
	pub use crate::material::{MaterialManager, MaterialSpecification};
	pub use crate::rendertarget::*;
	pub use crate::rendercontext::*;
}

#[macro_use]
extern crate log;


new_key_type! {
	pub struct ShaderKey;
	pub struct MeshFormatKey;
	pub struct MeshKey;
	pub struct MaterialKey;
	pub struct TextureKey;
	pub struct BufferKey;
	pub struct BindGroupKey;
	pub struct BindGroupLayoutKey;
	pub struct RenderItemKey;
	pub struct RenderBatchKey;
	pub struct RenderComputeKey;
	pub struct RenderContextKey;
	pub struct SamplerKey;
}


pub struct RenderResourcesRefMut<'a> {
	pub device: &'a wgpu::Device,
	pub queue: &'a wgpu::Queue,
	pub shaders: &'a mut ShaderManager,
	pub materials: &'a mut MaterialManager,
	pub meshes: &'a mut MeshManager,
	pub textures: &'a mut TextureManager,
	pub buffers: &'a mut BufferManager,
	pub bind_groups: &'a mut BindGroupManager,
	pub contexts: &'a mut RenderContextManager,
}


// Put in RenderResourcesRefMut
#[profiling::function]
pub fn prepare_for_render(
	device: &wgpu::Device,
	queue: &wgpu::Queue,
	shaders: &mut ShaderManager,
	materials: &mut MaterialManager,
	meshes: &mut MeshManager,
	textures: &mut TextureManager,
	buffers: &mut BufferManager,
	bind_groups: &mut BindGroupManager,
	contexts: &mut RenderContextManager,
) {
	info!("Preparing for render");

	
	info!("Register shaders");
	// Materials load their shaders (if DNE) and fetch their keys
	materials.read_shaders_and_fetch_keys(shaders);


	info!("Load shaders");
	// Shaders are loaded
	// Register bind group layout with BGM
	// Register mesh format with MM
	shaders.load_and_register(meshes, bind_groups);

	
	info!("Bind layouts");
	// Layouts are built and bound
	bind_groups.build_layouts(device);

	
	info!("Update materials");
	// Materials load the resources in their specifications
	materials.read_specified_resources(shaders, textures, buffers).unwrap();
	

	info!("Context materials");
	// Contexts (create and) fetch bind group keys
	// Apply usages to resources
	contexts.bind_materials(
		materials, 
		shaders, 
		textures, 
		buffers, 
		bind_groups, 
	).unwrap();


	info!("Bind textures");
	// Textures are bound
	textures.update_bindings(device, queue, bind_groups);

	info!("Bind buffers");
	// Buffers are bound
	buffers.update_bindings(device, bind_groups);
	

	info!("Bind the bind groups");
	// Bind groups are created
	bind_groups.update_bindings(device, textures, buffers);

	info!("Build shader pipelines");
	// Pipelines are built
	shaders.build_pipelines(device, bind_groups);


	// Be sure to bind the meshes!
}
