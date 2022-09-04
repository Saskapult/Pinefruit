use wgpu::util::DeviceExt;
use nalgebra::*;




#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
	1.0, 0.0, 0.0, 0.0,
	0.0, 1.0, 0.0, 0.0,
	0.0, 0.0, 0.5, 0.0,
	0.0, 0.0, 0.5, 1.0,
);



#[derive(Debug)]
pub struct RenderCamera {
	pub position: Vector3<f32>,
	pub rotation: UnitQuaternion<f32>,
	pub fovy: f32,
	pub near: f32,
	pub far: f32,
}
impl RenderCamera {
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
			near: znear,
			far: zfar,
		}
	}

	pub fn view_matrix(&self) -> Matrix4<f32> {
		Matrix4::new_translation(&self.position) * self.rotation.to_homogeneous()
	}

	pub fn projection_matrix(&self, width: f32, height: f32) -> Matrix4<f32> {
		let fovr = (self.fovy / 360.0) * 2.0 * std::f32::consts::PI;
		OPENGL_TO_WGPU_MATRIX * perspective_lh(width / height, fovr, self.near, self.far)
	}

	pub fn view_projection_matrix(&self, width: f32, height: f32) -> Matrix4<f32> {
		// Todo: Use cool nalgebra stuff to bypass expensive inversion
		self.projection_matrix(width, height) * self.view_matrix().try_inverse().unwrap()
	}

	// Gets the direction from the camera that the mouse does point
	// https://antongerdelan.net/opengl/raycasting.html
	pub fn mouse_ray(
		&self, 
		width: f32, 
		height: f32, 
		mouse_x: u32, 
		mouse_y: u32,
	) -> Vector3<f32> {
		// 1
		let x = (2.0 * mouse_x as f32) / (width as f32) - 1.0;
		let y = 1.0 - (2.0 * mouse_y as f32) / (height as f32);
		let z = 1.0;
		let nds = Vector3::new(x, y, z);
		// 2
		let clip = Vector4::new(nds.x, nds.y, nds.z, 1.0);
		// 3
		let ip = self.projection_matrix(width, height).try_inverse().unwrap();
		let mut eye = ip * clip;
		eye[3] = 0.0;
		// 4
		let iv = self.view_matrix().try_inverse().unwrap();
		let world = (iv * eye).xyz().normalize();

		world
	}
}



#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
	// Position of the camera
	position: [f32; 4],
	// Projection * inverse of camera transform 
	view_projection: [[f32; 4]; 4],
	// Projection stuff
	projection: [[f32; 4]; 4],
	inv_projection: [[f32; 4]; 4],
}
impl CameraUniform {
	pub fn new() -> Self {
		Self {
			position: [0.0; 4],
			view_projection: Matrix4::identity().into(),
			projection: Matrix4::identity().into(),
			inv_projection: Matrix4::identity().into(),
		}
	}

	pub fn new_from_camera(camera: &RenderCamera, width: f32, height: f32) -> Self {
		Self {
			position: camera.position.to_homogeneous().into(),
			view_projection: camera.view_projection_matrix(width, height).into(),
			projection: camera.projection_matrix(width, height).into(),
			inv_projection: camera.projection_matrix(width, height).try_inverse().unwrap().into(),
		}
	}

	pub fn update(&mut self, camera: &RenderCamera, width: f32, height: f32,) {
		self.position = camera.position.to_homogeneous().into();
		self.view_projection = camera.view_projection_matrix(width, height).into();
		self.projection = camera.projection_matrix(width, height).into();
		self.inv_projection = camera.projection_matrix(width, height).try_inverse().unwrap().into();
	}

	pub fn make_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera uniform Buffer"),
			contents: bytemuck::cast_slice(&[self.clone()]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		})
	}

	pub fn update_buffer(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer) {
		queue.write_buffer(
			buffer, 
			0, 
			bytemuck::cast_slice(&[self.clone()]),
		);
	}
}



fn perspective_lh(aspect: f32, fovy: f32, near: f32, far: f32) -> Matrix4<f32> {
	let mut mat = Matrix4::zeros();

    let tan_half_fovy = (fovy / 2.0).tan();

    mat[(0, 0)] = 1.0 / (aspect * tan_half_fovy);
    mat[(1, 1)] = 1.0 / tan_half_fovy;
    mat[(2, 2)] = (far + near) / (far - near);
    mat[(2, 3)] = -(2.0 * far * near) / (far - near);
    mat[(3, 2)] = 1.0;

	mat
}
