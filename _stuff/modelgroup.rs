use crate::model::{
	model::Model,

};
use crate::entity::instance::Instance;
use wgpu::util::DeviceExt;

pub struct ModelGroup {
	pub name: String,
	pub model: Model,				// A reference to the loaded model
	pub instances: Vec<Instance>,
	pub instance_buffer: wgpu::Buffer,
}
impl ModelGroup {
	pub fn new(device: &wgpu::Device, name: String, model: Model) -> Self {
		let model = model;
		let instances = Vec::new();
		let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
		let instance_buffer = device.create_buffer_init(
			&wgpu::util::BufferInitDescriptor {
				label: Some("Instance Buffer"),
				contents: bytemuck::cast_slice(&instance_data),
				usage: wgpu::BufferUsages::VERTEX,
			}
		);

		Self {
			name,
			model,
			instances,
			instance_buffer
		}
	}

	pub fn add_instance(&mut self, device: &wgpu::Device, instance: Instance) {
		self.instances.push(instance);
		self.update_buffer(device);
	}

	pub fn update_buffer(&mut self, device: &wgpu::Device) {
		let instance_data = self.instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
		self.instance_buffer = device.create_buffer_init(
			&wgpu::util::BufferInitDescriptor {
				label: Some("Instance Buffer"),
				contents: bytemuck::cast_slice(&instance_data),
				usage: wgpu::BufferUsages::VERTEX,
			}
		);
	}
}

