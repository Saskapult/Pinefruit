use serde::{Serialize, Deserialize};

use crate::EntityIdentifier;


/// Attributes needed for an instance for a shader
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstanceAttribute {
	pub name: String,
	pub source: InstanceAttributeSource,
	pub fields: Vec<VertexFormatWrapper>,
	pub default: Option<Vec<u8>>,
}
impl InstanceAttribute {
	pub fn size(&self) -> u64 {
		self.fields.iter().fold(0, |acc, vfw| acc + vfw.convert().size())
	}
}

/// Attributes needed from a mesh for a shader
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VertexAttribute {
	pub name: String,
	pub source: String, // A field within the mesh data map
	pub fields: Vec<VertexFormatWrapper>,
	pub default: Option<Vec<u8>>,
}
impl VertexAttribute {
	pub fn size(&self) -> u64 {
		self.fields.iter().fold(0, |acc, vfw| acc + vfw.convert().size())
	}
}

/// Where to get instance attribute data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InstanceAttributeSource {
	Component(String),
	Resource(String),
}

/// Am more advanced form of [InstanceAttributeSource], ready to be used.
pub enum FetchedInstanceAttributeSource<'a, E: EntityIdentifier> {
	Component(Box<dyn InstanceComponentProvider<'a, E> + 'a>), // Holds a reference which lives for that long
	Resource(&'a [u8]),
}

/// Something that can return something which can return instance data.
/// Like an ECS world.
pub trait InstanceDataProvider<'a, E: EntityIdentifier> {
	fn get_storage(&self, component_id: impl AsRef<str>) -> Option<impl InstanceComponentProvider<'a, E>>;
	fn get_resource(&self, resource_id: impl Into<String>) -> Option<&'a [u8]>;
	fn fetch_source(&self, attribute: &InstanceAttributeSource) -> Option<FetchedInstanceAttributeSource<'a, E>>;
}

/// Something which can return instance data
/// Like a sparse set
pub trait InstanceComponentProvider<'a, E> {
	fn get_component(&self, entity_id: E) -> Option<&'a [u8]>; // Lifetime of reference, not self! 
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexFormatWrapper {
	Uint8x2,
	Uint8x4,
	Sint8x2,
	Sint8x4,
	Unorm8x2,
	Unorm8x4,
	Snorm8x2,
	Snorm8x4,
	Uint16x2,
	Uint16x4,
	Sint16x2,
	Sint16x4,
	Unorm16x2,
	Unorm16x4,
	Snorm16x2,
	Snorm16x4,
	Float16x2,
	Float16x4,
	Float32,
	Float32x2,
	Float32x3,
	Float32x4,
	Uint32,
	Uint32x2,
	Uint32x3,
	Uint32x4,
	Sint32,
	Sint32x2,
	Sint32x3,
	Sint32x4,
	Float64,
	Float64x2,
	Float64x3,
	Float64x4,
}
impl VertexFormatWrapper {
	pub fn convert(self) -> wgpu::VertexFormat {
		self.into()
	}
}
impl Into<wgpu::VertexFormat> for VertexFormatWrapper {
	fn into(self) -> wgpu::VertexFormat {
		match self {
			Self::Uint8x2 => wgpu::VertexFormat::Uint8x2,
			Self::Uint8x4 => wgpu::VertexFormat::Uint8x4,
			Self::Sint8x2 => wgpu::VertexFormat::Sint8x2,
			Self::Sint8x4 => wgpu::VertexFormat::Sint8x4,
			Self::Unorm8x2 => wgpu::VertexFormat::Unorm8x2,
			Self::Unorm8x4 => wgpu::VertexFormat::Unorm8x4,
			Self::Snorm8x2 => wgpu::VertexFormat::Snorm8x2,
			Self::Snorm8x4 => wgpu::VertexFormat::Snorm8x4,
			Self::Uint16x2 => wgpu::VertexFormat::Uint16x2,
			Self::Uint16x4 => wgpu::VertexFormat::Uint16x4,
			Self::Sint16x2 => wgpu::VertexFormat::Sint16x2,
			Self::Sint16x4 => wgpu::VertexFormat::Sint16x4,
			Self::Unorm16x2 => wgpu::VertexFormat::Unorm16x2,
			Self::Unorm16x4 => wgpu::VertexFormat::Unorm16x4,
			Self::Snorm16x2 => wgpu::VertexFormat::Snorm16x2,
			Self::Snorm16x4 => wgpu::VertexFormat::Snorm16x4,
			Self::Float16x2 => wgpu::VertexFormat::Float16x2,
			Self::Float16x4 => wgpu::VertexFormat::Float16x4,
			Self::Float32 => wgpu::VertexFormat::Float32,
			Self::Float32x2 => wgpu::VertexFormat::Float32x2,
			Self::Float32x3 => wgpu::VertexFormat::Float32x3,
			Self::Float32x4 => wgpu::VertexFormat::Float32x4,
			Self::Uint32 => wgpu::VertexFormat::Uint32,
			Self::Uint32x2 => wgpu::VertexFormat::Uint32x2,
			Self::Uint32x3 => wgpu::VertexFormat::Uint32x3,
			Self::Uint32x4 => wgpu::VertexFormat::Uint32x4,
			Self::Sint32 => wgpu::VertexFormat::Sint32,
			Self::Sint32x2 => wgpu::VertexFormat::Sint32x2,
			Self::Sint32x3 => wgpu::VertexFormat::Sint32x3,
			Self::Sint32x4 => wgpu::VertexFormat::Sint32x4,
			Self::Float64 => wgpu::VertexFormat::Float64,
			Self::Float64x2 => wgpu::VertexFormat::Float64x2,
			Self::Float64x3 => wgpu::VertexFormat::Float64x3,
			Self::Float64x4 => wgpu::VertexFormat::Float64x4,
		}
	}
}

