
use nalgebra::*;
extern crate nalgebra_glm as glm;




#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
	1.0, 0.0, 0.0, 0.0,
	0.0, 1.0, 0.0, 0.0,
	0.0, 0.0, 0.5, 0.0,
	0.0, 0.0, 0.5, 1.0,
);



#[derive(Debug)]
pub struct Camera {
	pub position: Vector3<f32>,
	pub rotation: UnitQuaternion<f32>,
	pub fovy: f32,
	pub znear: f32,
	pub zfar: f32,
}
impl Camera {
	pub fn new<P: Into<Vector3<f32>>, R: Into<UnitQuaternion<f32>>,>(
		position: P,
		rotation: R,
		fovy: f32,
		znear: f32,
		zfar: f32,
	) -> Self {

		Self {
			position: position.into(),
			rotation: rotation.into(),
			fovy,
			znear,
			zfar,
		}
	}

	pub fn view_matrix(&self) -> Matrix4<f32> {
		// Todo: Use cool nalgebra stuff to bypass expensive inversion
		(Matrix4::new_translation(&self.position) * self.rotation.to_homogeneous()).try_inverse().expect("fugg")
	}

	pub fn projection_matrix(&self, width: f32, height: f32) -> Matrix4<f32> {
		let fovr = (self.fovy / 360.0) * 2.0 * std::f32::consts::PI;
		OPENGL_TO_WGPU_MATRIX * glm::perspective_lh(width / height, fovr, self.znear, self.zfar)
	}

	pub fn view_projection_matrix(&self, width: f32, height: f32) -> Matrix4<f32> {
		// We could optimize this using solver stuff
		self.projection_matrix(width, height) * self.view_matrix()
	}
}



#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
	// Mat doesn't implement Zeroable so we need to convert to array literals
	position: [f32; 4],
	view_projection: [[f32; 4]; 4],
}
impl CameraUniform {
	pub fn new() -> Self {
		Self {
			position: [0.0; 4],
			view_projection: Matrix4::identity().into(),
		}
	}
	pub fn update(&mut self, camera: &Camera, width: f32, height: f32,) {
		self.position = camera.position.to_homogeneous().into();
		self.view_projection = (camera.view_projection_matrix(width, height)).into();
	}
}
