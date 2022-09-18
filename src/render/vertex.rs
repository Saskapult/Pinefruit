use std::mem;
use nalgebra::*;
use serde::{Serialize, Deserialize};




#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexProperty {
	VertexPosition,
	VertexColour,
	VertexUV,
	VertexTexture,
	VertexSkin,
}
impl VertexProperty {
	pub fn attribute_segment(self) -> AttributeSegment {
		match self {
			Self::VertexPosition => VertexPosition::attributes(),
			Self::VertexColour => VertexColour::attributes(),
			Self::VertexUV => VertexUV::attributes(),
			Self::VertexTexture => VertexTexture::attributes(),
			Self::VertexSkin => VertexSkin::attributes(),
		}
	}
}
pub type VertexProperties = Vec<VertexProperty>;



#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceProperty {
	InstanceModelMatrix,
	InstanceColour,
	InstanceTexture,
}
impl InstanceProperty {
	pub fn attribute_segment(self) -> AttributeSegment {
		match self {
			Self::InstanceModelMatrix => InstanceModelMatrix::attributes(),
			Self::InstanceColour => InstanceColour::attributes(),
			Self::InstanceTexture => InstanceTexture::attributes(),
		}
	}
}
pub type InstanceProperties = Vec<InstanceProperty>;



/// Trait for "can be put in vertex buffer"
pub trait Vertexable {
	const ASEG: AttributeSegment;
	fn attributes() -> AttributeSegment;
}



/// Attributes generated at the current field offset
/// length (bytes), vertex format
pub type AttributeSegment = &'static [(usize, wgpu::VertexFormat)];



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexPosition {
	pub position: [f32; 3],
}
impl Vertexable for VertexPosition {
	const ASEG: AttributeSegment = &[(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}


#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexNormal {
	pub normal: [f32; 3],
}
impl Vertexable for VertexNormal {
	const ASEG: AttributeSegment = &[(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}



// I think that colour should be represented by a in [0,1]
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexColour {
	pub colour: [f32; 3],
}
impl Vertexable for VertexColour {
	const ASEG: AttributeSegment = &[(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexUV {
	pub uv: [f32; 2],
}
impl Vertexable for VertexUV {
	const ASEG: AttributeSegment = &[(mem::size_of::<[f32; 2]>(), wgpu::VertexFormat::Float32x2)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexTexture {
	pub id: u32,
}
impl Vertexable for VertexTexture {
	const ASEG: AttributeSegment = &[(mem::size_of::<u32>(), wgpu::VertexFormat::Uint32)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}



const MAX_BONE_INFLUENCE: usize = 4;
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexSkin {
	pub n: u32,
	pub bones: [u32; MAX_BONE_INFLUENCE],
	pub weights: [f32; MAX_BONE_INFLUENCE],
}
impl VertexSkin {
	const fn get_mbi_aseg() -> [(usize, wgpu::VertexFormat); MAX_BONE_INFLUENCE*2+1] {
		let mut res = [(mem::size_of::<u32>(), wgpu::VertexFormat::Uint32); MAX_BONE_INFLUENCE*2+1];

		let mut i = 1;
		while i <= MAX_BONE_INFLUENCE {
			res[i] = (mem::size_of::<u32>(), wgpu::VertexFormat::Uint32);
			i += 1;
		}
		while i <= MAX_BONE_INFLUENCE*2 {
			res[i] = (mem::size_of::<f32>(), wgpu::VertexFormat::Float32);
			i += 1;
		}

		res
	}
	const MBI_ASEG: [(usize, wgpu::VertexFormat); MAX_BONE_INFLUENCE*2+1] = Self::get_mbi_aseg();
}
impl Vertexable for VertexSkin {
	const ASEG: AttributeSegment = &Self::MBI_ASEG;
	fn attributes() -> AttributeSegment {
		Self::ASEG
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
		let model = Matrix4::new_translation(position) * rotation.to_homogeneous();
		Self {
			model: model.into(),
		}
	}
}
impl Vertexable for InstanceModelMatrix {
	const ASEG: AttributeSegment = &[
		(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
		(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
		(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
		(mem::size_of::<[f32; 4]>(), wgpu::VertexFormat::Float32x4),
	];
	fn attributes() -> AttributeSegment {
		Self::ASEG
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
	const ASEG: AttributeSegment = &[(mem::size_of::<[f32; 3]>(), wgpu::VertexFormat::Float32x3)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}



#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceTexture {
	pub id: u32,
}
impl InstanceTexture {
	pub fn new(id: u32) -> Self {
		Self {
			id,
		}
	}
}
impl Vertexable for InstanceTexture {
	const ASEG: AttributeSegment = &[(mem::size_of::<u32>(), wgpu::VertexFormat::Uint32)];
	fn attributes() -> AttributeSegment {
		Self::ASEG
	}
}
