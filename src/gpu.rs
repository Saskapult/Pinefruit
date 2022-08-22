use std::sync::{Arc, RwLock};
use crate::render::*;
use crate::mesh::*;
use crate::texture::*;
use crate::material::*;


pub enum DeviceOptions {
	Maximum,
	Default,
	Custom((wgpu::Features, wgpu::Limits)),
}



pub fn acquire_device(
	adapter: &wgpu::Adapter,
	features_limits: DeviceOptions,
) -> Result<(Arc<wgpu::Device>, Arc<wgpu::Queue>), wgpu::RequestDeviceError> {
	let info = adapter.get_info();
	info!("Attaching to device {} ({:?})", info.name, info.backend);
	let (features, limits) = match features_limits {
		DeviceOptions::Custom(fl) => {
			info!("using custom features and limits");
			fl
		},
		DeviceOptions::Default => {
			info!("using wgpu default features and limits");
			(wgpu::Features::default(), wgpu::Limits::default())
		},
		DeviceOptions::Maximum => {
			warn!("using all supported features and limits");
			(adapter.features(), adapter.limits())
		}
	};
	info!("Features: {features:?}");
	info!("Limits: {limits:?}");
	let (device, queue) = pollster::block_on(adapter.request_device(
		&wgpu::DeviceDescriptor {
			features, limits,
			label: Some("kkraft device descriptor"),
		},
		None,
	))?;
	let device = Arc::new(device);
	let queue = Arc::new(queue);

	Ok((device, queue))
}



/// A thing that stores data and stuff on the GPU.
pub struct GpuData {
	pub textures: BoundTextureManager,
	pub shaders: ShaderManager,
	pub materials: BoundMaterialManager,
	pub meshes: BoundMeshManager,
}
impl GpuData {
	pub fn new(
		device: &Arc::<wgpu::Device>, 
		queue: &Arc::<wgpu::Queue>,
		textures_data_manager: &Arc<RwLock<TextureManager>>,
		meshes_data_manager: &Arc<RwLock<MeshManager>>,
		materials_data_manager: &Arc<RwLock<MaterialManager>>,
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();
		let textures = BoundTextureManager::new(&device, &queue, textures_data_manager);
		let materials = BoundMaterialManager::new(&device, &queue, materials_data_manager);
		let meshes = BoundMeshManager::new(&device, &queue, meshes_data_manager);
		let shaders = ShaderManager::new(&device, &queue);

		Self {
			textures,
			materials,
			meshes,
			shaders,
		}
	}
}
