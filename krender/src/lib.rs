//! Krender is here becuse my build times were awful.
//! It is extensible I hope.

#![feature(return_position_impl_trait_in_trait)]
#![feature(int_roundings)]
#![feature(let_chains)]

use std::{collections::{BTreeMap, HashMap}, hash::Hash};
use bindgroup::BindGroupManager;
use material::MaterialManager;
use mesh::MeshManager;
use prelude::BufferManager;
use rendercontext::RenderContextManager;
use shader::ShaderManager;
use slotmap::new_key_type;
use texture::TextureManager;
use vertex::InstanceDataProvider;
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

pub mod prelude {
	pub use crate::input::RenderInput;
	pub use tobj;
	pub use crate::texture::{Texture, TextureManager, TextureFormat};
	pub use crate::mesh::{Mesh, MeshManager};
	pub use crate::buffer::{BufferManager, Buffer};
	pub use crate::vertex::{InstanceDataProvider, InstanceComponentProvider, InstanceAttributeSource, FetchedInstanceAttributeSource};
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


pub trait EntityIdentifier: Copy + PartialEq + Eq + Hash {}
impl<T> EntityIdentifier for T where T: Copy + PartialEq + Eq + Hash {}


pub struct RenderResourcesRefMut<'a, T: EntityIdentifier> {
	pub device: &'a wgpu::Device,
	pub queue: &'a wgpu::Queue,
	pub shaders: &'a mut ShaderManager,
	pub materials: &'a mut MaterialManager,
	pub meshes: &'a mut MeshManager,
	pub textures: &'a mut TextureManager,
	pub buffers: &'a mut BufferManager,
	pub bind_groups: &'a mut BindGroupManager,
	pub contexts: &'a mut RenderContextManager<T>,
}


// Put in RenderResourcesRefMut
pub fn prepare_for_render<T: EntityIdentifier>(
	device: &wgpu::Device,
	queue: &wgpu::Queue,
	shaders: &mut ShaderManager,
	materials: &mut MaterialManager,
	meshes: &mut MeshManager,
	textures: &mut TextureManager,
	buffers: &mut BufferManager,
	bind_groups: &mut BindGroupManager,
	contexts: &RenderContextManager<T>,
) {
	warn!("Preparing for render");
	// Double space for stuff that can't be parallel

	info!("Register shaders");
	materials.read_unknown_shaders(shaders);


	info!("Load shaders");
	shaders.load_and_register(meshes, bind_groups);


	info!("Bind layouts");
	bind_groups.build_layouts(device);

	
	info!("Update materials");
	materials.read_unknown_resources(shaders, textures, buffers).unwrap();
	materials.update(shaders, textures, buffers, bind_groups, contexts);
	

	info!("Bind textures");
	textures.update_bindings(device, queue, bind_groups);

	info!("Bind buffers");
	buffers.update_bindings(device, bind_groups);
	buffers.do_queued_writes(queue);
	

	info!("Bind the bind groups");
	bind_groups.update_bindings(device, textures, buffers);

	info!("Build shader pipelines");
	shaders.build_pipelines(device, bind_groups);


	// info!("Mesh binding");
	// meshes.bind_unbound(device);
}
