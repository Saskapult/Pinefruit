use serde::{Serialize, Deserialize};
use nalgebra::*;
// use crate::render::*;




#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum LightType {
	PointLight,
	ConeLight,
	OrthoLight,
}



pub struct PointLight {
	pub colour: [f32; 3],
	pub position: Vector3<f32>,
	pub radius: f32,
}



pub struct ConeLight {
	pub colour: [f32; 3],
	pub position: Vector3<f32>,
	pub rotation: UnitQuaternion<f32>,
	pub radius: f32,
	pub angle: f32,
}



pub struct OrthoLight {
	pub colour: [f32; 3],
	pub rotation: UnitQuaternion<f32>,
}

