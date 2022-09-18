use wgpu::util::DeviceExt;
use nalgebra::*;




#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
	1.0, 0.0, 0.0, 0.0,
	0.0, 1.0, 0.0, 0.0,
	0.0, 0.0, 0.5, 0.0,
	0.0, 0.0, 0.5, 1.0,
);



// width / height
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
		near: f32,
		far: f32,
	) -> Self {

		Self {
			position: position.into(),
			rotation: rotation.into(),
			fovy,
			near,
			far,
		}
	}

	fn view_matrix(&self) -> Matrix4<f32> {
		Matrix4::new_translation(&self.position) * self.rotation.to_homogeneous()
	}
	fn inverse_view_matrix(&self) -> Matrix4<f32> {
		Matrix4::new_translation(&-self.position) * self.rotation.inverse().to_homogeneous()
	}

	pub fn projection_matrix(&self, aspect: f32) -> Matrix4<f32> {
		let fovr = self.fovy.to_radians();
		OPENGL_TO_WGPU_MATRIX * perspective_lh(aspect, fovr, self.near, self.far)
	}

	fn view_projection_matrix(&self, aspect: f32) -> Matrix4<f32> {
		// Todo: Use cool nalgebra stuff to bypass expensive inversion
		self.projection_matrix(aspect) * self.inverse_view_matrix()
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
		let ip = self.projection_matrix(width / height).try_inverse().unwrap();
		let mut eye = ip * clip;
		eye[3] = 0.0;
		// 4
		let iv = self.view_matrix().try_inverse().unwrap();
		let world = (iv * eye).xyz().normalize();

		world
	}

	fn uniform_data(&self, aspect: f32) -> RenderCameraUniform {
		RenderCameraUniform {
			position: self.position.to_homogeneous().into(),
			rotation: self.rotation.to_homogeneous().into(),
			near: self.near,
			far: self.far,
			fovy: self.fovy,
			pad: 0.0,
			view: self.view_matrix().into(),
			inverse_view: self.inverse_view_matrix().into(),
			projection: self.projection_matrix(aspect).into(),
			inverse_projection: self.projection_matrix(aspect).try_inverse().unwrap().into(),
			view_projection: self.view_projection_matrix(aspect).into(),
		}
	}

	pub fn make_buffer(&self, device: &wgpu::Device, aspect: f32) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera uniform Buffer"),
			contents: bytemuck::bytes_of(&self.uniform_data(aspect)),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		})
	}

	pub fn update_buffer(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer, aspect: f32) {
		queue.write_buffer(
			buffer, 
			0, 
			bytemuck::bytes_of(&self.uniform_data(aspect)),
		);
	}
}



#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RenderCameraUniform {
	position: [f32; 4],
	rotation: [[f32; 4]; 4],
	near: f32,
	far: f32,
	fovy: f32,
	pad: f32,
	
	view: [[f32; 4]; 4],
	inverse_view: [[f32; 4]; 4],

	projection: [[f32; 4]; 4],
	inverse_projection: [[f32; 4]; 4],

	view_projection: [[f32; 4]; 4],
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
