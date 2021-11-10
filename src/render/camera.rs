
use winit::event::*;
use winit::dpi::PhysicalPosition;
use std::time::Duration;
use nalgebra::*;
use crate::render::*;
use wgpu::util::DeviceExt;


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
	aspect: f32,
    fovy: f32,
    near: f32,
    far: f32,
    pub camera_uniform: CameraUniform,
    pub camera_uniform_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,
}
impl Camera {
    pub fn new<P: Into<Vector3<f32>>, R: Into<UnitQuaternion<f32>>,>(
        position: P,
        rotation: R,
		aspect: f32,
		fovy: f32,
		near: f32,
		far: f32,
        device: &wgpu::Device,
    ) -> Self {
        let camera_uniform = CameraUniform::new();

        let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        //let camera_controller = CameraController::new(4.0, 0.4);

        Self {
            position: position.into(),
            rotation: rotation.into(),
			aspect,
			fovy,
			near,
			far,
            camera_uniform,
            camera_uniform_buffer,
            camera_bind_group_layout,
            camera_bind_group,
        }
    }

    pub fn view_matrix(&self) -> Matrix4<f32> {
        self.rotation.to_homogeneous() * Matrix4::new_translation(&self.position)
    }

	pub fn proj_matrix(&self) -> Matrix4<f32> {
        Matrix4::new_perspective(self.aspect, self.fovy, self.near, self.far)
    }

	pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
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
    pub fn update(&mut self, camera: &Camera) {
        self.position = camera.position.to_homogeneous().into();
        self.view_projection = (OPENGL_TO_WGPU_MATRIX * camera.proj_matrix() * camera.view_matrix()).into();
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
    move_sensitivity: f32,
    look_sensitivity: f32,
    mousedown: bool,
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
            move_sensitivity: sensitivity,
            look_sensitivity: sensitivity,
            mousedown: false,
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

    pub fn process_mouse_movement(&mut self, mouse_dx: f64, mouse_dy: f64) {
        if self.mousedown {
            self.rotate_horizontal = mouse_dx as f32;
            self.rotate_vertical = mouse_dy as f32;
        }
    }

    pub fn process_mouse_key(&mut self, mouse_dx: f64, mouse_dy: f64) {
        if self.mousedown {
            self.rotate_horizontal = mouse_dx as f32;
            self.rotate_vertical = mouse_dy as f32;
        }
    }

    pub fn process_mouse_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            // I'm assuming a line is about 100 pixels
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition {
                y: scroll,
                ..
            }) => *scroll as f32,
        };
    }

    pub fn update_movement(&mut self, dt: Duration) -> (Vector3<f32>, UnitQuaternion<f32>) {
        let dt = dt.as_secs_f32();

        // Move
        let dx = (self.amount_right - self.amount_left) * self.speed * dt;
        let dz = (self.amount_forward - self.amount_backward) * self.speed * dt;
        let dy = (self.amount_up - self.amount_down) * self.speed * dt;
        let displacement = Vector3::new(dx, dy, dz);

        // Rotate
        let drot = UnitQuaternion::from_euler_angles(0.0, -self.rotate_vertical * self.look_sensitivity * dt, self.rotate_horizontal * self.look_sensitivity * dt);

        // If process_mouse isn't called every frame, these values
        // will not get set to zero, and the camera will rotate
        // when moving in a non cardinal direction.
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        (displacement, drot)
    }
}





