use crate::render::*;
use std::sync::{Arc, RwLock};
use crate::mesh::*;
use crate::texture::*;
use crate::material::*;




/// All that sweet GPU stuff, could maybe have a better name (GPUResources?)
#[derive(Debug)]
pub struct RenderResources {
	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,

	pub textures: BoundTextureManager,
	pub shaders: ShaderManager,
	pub materials: BoundMaterialManager,
	pub meshes: BoundMeshManager,
	pub models: ModelManager,
}
impl RenderResources {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		textures_data_manager: &Arc<RwLock<TextureManager>>,
		meshes_data_manager: &Arc<RwLock<MeshManager>>,
		materials_data_manager: &Arc<RwLock<MaterialManager>>,
	) -> Self {
		Self {
			device: device.clone(),
			queue: queue.clone(),
			textures: BoundTextureManager::new(device, queue, textures_data_manager), 
			materials: BoundMaterialManager::new(device, queue, materials_data_manager), 
			meshes: BoundMeshManager::new(device, queue, meshes_data_manager), 
			shaders: ShaderManager::new(device, queue), 
			models: ModelManager::new(), 
		}
	}
}
