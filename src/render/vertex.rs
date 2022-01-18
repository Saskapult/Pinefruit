use std::mem;
use nalgebra::*;


/*
Vertex data is divided into various categories (positional, texture, instance) in order to maximize flexibility
A shader can specify the format of its vertex data
*/



// Trait for "can be put in vertex buffer"
pub trait Vertexable {
	// const for emum value entry??
	fn attributes() -> AttributeSegment;
}



// Attributes generated at the current field offset
// length (bytes), vertex format
pub type AttributeSegment = Vec<(usize, wgpu::VertexFormat)>;



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexPosition {
	pub position: [f32; 3],
}
impl Vertexable for VertexPosition {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3),
		]
	}
}


#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexNormal {
	pub normal: [f32; 3],
}
impl Vertexable for VertexNormal {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3),
		]
	}
}



// I think that colour should be represented by a in [0,1]
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexColour {
	pub colour: [f32; 3],
}
impl Vertexable for VertexColour {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3),
		]
	}
}



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexUV {
	pub uv: [f32; 2],
}
impl Vertexable for VertexUV {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<[f32; 2]>(), wgpu::VertexFormat::Float32x2),
		]
	}
}



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexTextureID {
	pub id: u32,
}
impl Vertexable for VertexTextureID {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<u32>(), wgpu::VertexFormat::Uint32),
		]
	}
}



#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceModelMatrix {
	model: [[f32; 4]; 4],
}
impl InstanceModelMatrix {
	// Todo: add scale and all that
	pub fn from_pr(position: &Vector3<f32>, rotation: &UnitQuaternion<f32>) -> Self {
		let model = rotation.to_homogeneous() * Matrix4::new_translation(position);
		Self {
			model: model.into(),
		}
	}
}
impl Vertexable for InstanceModelMatrix {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
			(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
			(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
			(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
		]
	}
}



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceColour {
	pub colour: [f32; 3],
}
impl InstanceColour {
	pub fn new(r: f32, g: f32, b: f32) -> Self {
		Self {
			colour: [r, g, b],
		}
	}
}
impl Vertexable for InstanceColour {
	fn attributes() -> AttributeSegment {
		vec![
			(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3),
		]
	}
}
