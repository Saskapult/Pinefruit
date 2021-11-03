
use winit::event::*;
use winit::dpi::PhysicalPosition;
use std::time::Duration;
use nalgebra::*;


#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);


// A camera in the world
#[derive(Debug)]
pub struct Camera {
    pub position: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
	aspect: f32,
    fovy: f32,
    near: f32,
    far: f32,
}
impl Camera {
    pub fn new<
        P: Into<Vector3<f32>>,
        R: Into<UnitQuaternion<f32>>
    >(
        position: P,
        rotation: R,
		aspect: f32,
		fovy: f32,
		near: f32,
		far: f32,
    ) -> Self {
        Self {
            position: position.into(),
            rotation: rotation.into(),
			aspect,
			fovy,
			near,
			far,
        }
    }

    pub fn cam_matrix(&self) -> Matrix4<f32> {
        self.rotation.to_homogeneous() * Matrix4::new_translation(&self.position)
    }

	pub fn proj_matrix(&self) -> Matrix4<f32> {
        Matrix4::new_perspective(self.aspect, self.fovy, self.near, self.far)
        //glm::perspective_lh(self.aspect, self.fovy, self.near, self.far)
    }

    pub fn view_matrix(&self) -> Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * self.proj_matrix() * self.cam_matrix()
    }

	pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }
}


// A coltroller for the camera
#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
}
impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool{
        let amount = if state == ElementState::Pressed { 1.0 } else { 0.0 };
        match key {
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.amount_forward = amount;
                true
            }
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.amount_backward = amount;
                true
            }
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.amount_left = amount;
                true
            }
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.amount_right = amount;
                true
            }
            VirtualKeyCode::Space => {
                self.amount_up = amount;
                true
            }
            VirtualKeyCode::LShift => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn process_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            // I'm assuming a line is about 100 pixels
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition {
                y: scroll,
                ..
            }) => *scroll as f32,
        };
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: Duration) {
        let dt = dt.as_secs_f32();

        // Move
        camera.position.x += (self.amount_right - self.amount_left) * self.speed * dt;
        camera.position.z += (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        // Rotate
        camera.rotation = camera.rotation * UnitQuaternion::from_euler_angles(0.0, -self.rotate_vertical * self.sensitivity * dt, self.rotate_horizontal * self.sensitivity * dt);

        println!("Pos: x={}, y={}, z={}", camera.position.x, camera.position.y, camera.position.z);
        //let (rx, ry, rz) = camera.rotation.euler_angles();
        //println!("Rot: x={}, y=%{}, z=%{}", rx, ry, rz);


        // If process_mouse isn't called every frame, these values
        // will not get set to zero, and the camera will rotate
        // when moving in a non cardinal direction.
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;
    }
}

