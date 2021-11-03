use nalgebra::*;
use crate::render::camera::Camera;


#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    // Mat doesn't implement Zeroable so we need to convert to array literals
	pos: [f32; 4],
    vp: [[f32; 4]; 4],
	ip: [[f32; 4]; 4],
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
			pos: [0.0, 0.0, 0.0, 0.0],
            vp: Matrix4::identity().into(),
			ip: Matrix4::identity().into(),
        }
    }
    pub fn update(&mut self, camera: &Camera) {
        self.pos = camera.position.to_homogeneous().into();
        self.vp = camera.view_matrix().into();
    }
}


#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    pub _padding: u32,
    pub color: [f32; 3],
}

