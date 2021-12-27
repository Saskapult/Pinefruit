use nalgebra::*;
use wgpu;

// An instance of an object
#[derive(Debug, Copy, Clone)]
pub struct Instance {
	pub position: Vector3<f32>,
	pub rotation: UnitQuaternion<f32>,
}
impl Instance {
	pub fn new() -> Self {
		let position = Vector3::new(0.0, 0.0, 0.0);
		let rotation = UnitQuaternion::identity();

		Self {
			position,
			rotation,
		}
	}
	pub fn to_raw(&self) -> InstanceRaw {
		let model = self.rotation.to_homogeneous() * Matrix4::new_translation(&self.position);
		InstanceRaw {
			model: model.into(),
		}
	}
}


// Instance data to pass to shaders
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
	model: [[f32; 4]; 4],	// Model matrix
}
impl InstanceRaw {
	pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
		use std::mem;
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
			// change to next instance only when processing a new instance
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &[
				wgpu::VertexAttribute {
					offset: 0,
					shader_location: 4,		// Vertex data uses previous slots
					format: wgpu::VertexFormat::Float32x4,
				},
				wgpu::VertexAttribute {
					offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
					shader_location: 5,
					format: wgpu::VertexFormat::Float32x4,
				},
				wgpu::VertexAttribute {
					offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
					shader_location: 6,
					format: wgpu::VertexFormat::Float32x4,
				},
				wgpu::VertexAttribute {
					offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
					shader_location: 7,
					format: wgpu::VertexFormat::Float32x4,
				},
			],
		}
	}
}