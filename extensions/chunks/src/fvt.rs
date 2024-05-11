use glam::{IVec3, Vec3};


#[derive(Debug, Clone, Copy)]
pub struct FVTIteratorItem {
	pub voxel: IVec3,
	pub t: f32,
	pub normal: IVec3,
}


/// An iterator for Fast Voxel Traversal
#[derive(Debug)]
pub struct FVTIterator {
	// origin: Vec3,
	// direction: Vec3,
	pub vx: i32,
	pub vy: i32,
	pub vz: i32,
	v_step_x: i32,
	v_step_y: i32,
	v_step_z: i32,
	t_delta_x: f32,
	t_delta_y: f32,
	t_delta_z: f32,
	t_max_x: f32,
	t_max_y: f32,
	t_max_z: f32,
	pub t: f32,
	t_max: f32,
	pub normal: IVec3,
}
impl FVTIterator {
	pub fn new(
		origin: Vec3,
		direction: Vec3,
		_t_min: f32, // Could do origin = origin + direction * t_min but that loses normal data
		t_max: f32,
		voxel_scale: f32,
	) -> Self {

		if t_max < 0.0 {
			panic!("No.")
		}

		// Origin cell
		let vx = (origin[0] / voxel_scale).floor() as i32;
		let vy = (origin[1] / voxel_scale).floor() as i32; 
		let vz = (origin[2] / voxel_scale).floor() as i32;

		let direction = direction.normalize();
		let dx = direction[0]; 
		let dy = direction[1]; 
		let dz = direction[2];

		let v_step_x = dx.signum() as i32;
		let v_step_y = dy.signum() as i32;
		let v_step_z = dz.signum() as i32;

		let t_delta_x = voxel_scale / dx.abs();
		let t_delta_y = voxel_scale / dy.abs();
		let t_delta_z = voxel_scale / dz.abs();


		let frac = |f: f32, dp: bool| {
			if dp {
				f - f.floor()
			} else {
				1.0 - f + f.floor()
			}
		};
		let t_max_x = t_delta_x * (1.0 - frac(origin[0] / voxel_scale, v_step_x >= 0));
		let t_max_y = t_delta_y * (1.0 - frac(origin[1] / voxel_scale, v_step_y >= 0));
		let t_max_z = t_delta_z * (1.0 - frac(origin[2] / voxel_scale, v_step_z >= 0));

		if t_delta_x == 0.0 && t_delta_y == 0.0 && t_delta_z == 0.0 {
			panic!("This train is going nowhere!")
		}
		if t_delta_x == f32::INFINITY && t_delta_y == f32::INFINITY && t_delta_z == f32::INFINITY {
			panic!("This train is also going nowhere!")
		}

		Self {
			// origin,
			// direction,
			vx, vy, vz,
			v_step_x, v_step_y, v_step_z,
			t_delta_x, t_delta_y, t_delta_z, 
			t_max_x, t_max_y, t_max_z, 
			t: 0.0,
			t_max,
			normal: IVec3::ZERO,
		}
	}
}
impl Iterator for FVTIterator {
	type Item = FVTIteratorItem;

	fn next(&mut self) -> Option<Self::Item> {

		if self.t_max_x < self.t_max_y {
			if self.t_max_x < self.t_max_z {
				self.normal = IVec3::new(-self.v_step_x, 0, 0);
				self.vx += self.v_step_x;
				self.t = self.t_max_x;
				self.t_max_x += self.t_delta_x;
				
			} else {
				self.normal = IVec3::new(0, 0, -self.v_step_z);
				self.vz += self.v_step_z;
				self.t = self.t_max_z;
				self.t_max_z += self.t_delta_z;
			}
		} else {
			if self.t_max_y < self.t_max_z {
				self.normal = IVec3::new(0, -self.v_step_y, 0);
				self.vy += self.v_step_y;
				self.t = self.t_max_y;
				self.t_max_y += self.t_delta_y;
			} else {
				self.normal = IVec3::new(0, 0, -self.v_step_z);
				self.vz += self.v_step_z;
				self.t = self.t_max_z;
				self.t_max_z += self.t_delta_z;
			}
		}

		if self.t <= self.t_max {
			Some(FVTIteratorItem {
				voxel: IVec3::new(self.vx, self.vy, self.vz),
				t: self.t,
				normal: self.normal,
			})
		} else {
			None
		}
	}
}
