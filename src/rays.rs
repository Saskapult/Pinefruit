use nalgebra::*;


pub trait Intersect<Other> {
	type IOutput;
	fn intersect(&self, other: &Other) -> Option<Self::IOutput>;
}



struct RayPointLight {
	pub position: Vector3<f32>,
	pub radius: f32,
	pub colour: [f32; 3],
}



pub struct Ray {
	pub origin: Vector3<f32>,
	pub direction: Vector3<f32>,
}
impl Ray {
	pub fn new(origin: Vector3<f32>, mut direction: Vector3<f32>) -> Self {
		direction.normalize_mut();
		Self { origin, direction }
	}
}



pub struct Sphere {
	pub position: Vector3<f32>,
	pub radius: f32,
}



#[derive(Debug, Clone)]
pub struct OBB {
	pub aabb: AABB,
	pub orientation: UnitQuaternion<f32>,
}
impl OBB {
	pub fn corners(&self) -> [Vector3<f32>; 8] {
		let n = &self.aabb.min;
		let p = &self.aabb.max;
		let nnn = Vector3::new(n[0], n[1], n[2]);
		let nnp = Vector3::new(n[0], n[1], p[2]);
		let npn = Vector3::new(n[0], p[1], n[2]);
		let npp = Vector3::new(n[0], p[1], p[2]);
		let pnn = Vector3::new(p[0], n[1], n[2]);
		let pnp = Vector3::new(p[0], n[1], p[2]);
		let ppn = Vector3::new(p[0], p[1], n[2]);
		let ppp = Vector3::new(p[0], p[1], p[2]);
		[nnn, nnp, npn, npp, pnn, pnp, ppn, ppp]
	}

	pub fn bounding_aabb(&self) -> AABB {
		let c = self.corners().map(|c| self.orientation * c);
		let mut aabb_max = c[0];
		let mut aabb_min = c[0];
		for c in &c[1..] {
			for i in 0..3 {
				if c[i] > aabb_max[i] {
					aabb_max[i] = c[i];
				}
				if c[i] < aabb_min[i] {
					aabb_min[i] = c[i];
				}
			}
		}
		AABB::new(aabb_min, aabb_max)
	}

	// Untested
	pub fn ray_intersect(
		&self, 
		origin: Vector3<f32>, 
		direction: Vector3<f32>, 
		position: Vector3<f32>, 
		t0: f32, 
		t1: f32, 
	) -> Option<(f32, f32)> {
		let poisiton_relative_to_ray = position - origin;

		let mut t_min = t0;
		let mut t_max = t1;

		for i in 0..3 {
			let axis = self.orientation * Vector3::new(
				if i==0 { 1.0 } else { 0.0 }, 
				if i==1 { 1.0 } else { 0.0 }, 
				if i==2 { 1.0 } else { 0.0 },
			);
			let e = axis.dot(&poisiton_relative_to_ray);
			let f = direction.dot(&axis);
			if f.abs() > 0.00000001 {
				let (t1, t2) = {
					let t1 = (e + self.aabb.min[i]) / f;
					let t2 = (e + self.aabb.max[i]) / f;
					if t1 < t2 {
						(t1, t2)
					} else {
						(t2, t1)
					}
				};
	
				if t2 < t_max {
					t_max = t2;
				}
				if t1 > t_min {
					t_min = t1;
				}
	
				if t_max < t_min {
					return None;
				}
			} else {
				if -e + self.aabb.min[i] > 0.0 || -e + self.aabb.max[i] > 0.0 {
					return None;
				}
			}
		}

		Some((t_min, t_max))
	}
}



#[derive(Debug, Clone)]
pub struct AABB {
	pub min: Vector3<f32>,
	pub max: Vector3<f32>,
}
impl AABB {
	pub fn new(
		aabb_min: Vector3<f32>,
		aabb_max: Vector3<f32>,
	) -> Self {
		Self {
			min: aabb_min, max: aabb_max,
		}
	}

	pub fn extent(&self) -> Vector3<f32> {
		(self.max - self.min).abs() / 2.0
	}

	pub fn centre(&self) -> Vector3<f32> {
		self.min + self.extent()
	}

	// Todo: handle div by zero
	// https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-box-intersection
	#[inline]
	pub fn ray_intersect(
		&self, 
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		position: Vector3<f32>, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<(f32, f32)> {
		let v_max = self.max + position;
		let v_min = self.min + position;

		let (mut t_min, mut t_max) = {
			let t_min = (v_min[0] - origin[0]) / direction[0];
			let t_max = (v_max[0] - origin[0]) / direction[0];

			if t_min < t_max {
				(t_min, t_max)
			} else {
				(t_max, t_min)
			}
		};

		let (ty_min, ty_max) = {
			let ty_min = (v_min[1] - origin[1]) / direction[1];
			let ty_max = (v_max[1] - origin[1]) / direction[1];

			if ty_min < ty_max {
				(ty_min, ty_max)
			} else {
				(ty_max, ty_min)
			}
		};

		if t_min > ty_max || ty_min > t_max {
			return None
		}

		if ty_min > t_min {
			t_min = ty_min;
		}
		if ty_max < t_max {
			t_max = ty_max;
		}

		let (tz_min, tz_max) = {
			let tz_min = (v_min[2] - origin[2]) / direction[2];
			let tz_max = (v_max[2] - origin[2]) / direction[2];

			if tz_min < tz_max {
				(tz_min, tz_max)
			} else {
				(tz_max, tz_min)
			}
		};

		if t_min > tz_max || tz_min > t_max {
			return None
		}

		if tz_min > t_min {
			t_min = tz_min;
		}
		if tz_max < t_max {
			t_max = tz_max;
		}
		
		if (t_min < t1) && (t_max > t0) {
			Some((t_min, t_max))
		} else {
			None
		}
	}

	pub fn contains(&self, point: Vector3<f32>) -> bool {
		point >= self.min && point <= self.max

		// point[0] >= self.p0[0] &&
		// point[1] >= self.p0[1] &&
		// point[2] >= self.p0[2] &&
		// point[0] <= self.p1[0] &&
		// point[1] <= self.p1[1] &&
		// point[2] <= self.p1[2]
	}

	pub fn mid_planes(&self) -> [Plane; 3] {
		let centre = self.centre();
		[
			Plane {
				normal: *Vector3::z_axis(),
				distance: centre[2],
			},
			Plane {
				normal: *Vector3::y_axis(),
				distance: centre[1],
			},
			Plane {
				normal: *Vector3::x_axis(),
				distance: centre[0],
			},
		]
	}
}



#[derive(Debug, Clone)]
pub struct Plane {
	pub normal: Vector3<f32>,
	pub distance: f32,
}
impl Plane {
	// Restricted to along positive line direction
	pub fn ray_intersect(
		&self, 
		origin: Vector3<f32>,
		direction: Vector3<f32>,
		position: Vector3<f32>, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<f32> {
		let d = self.normal.dot(&direction);
		if d > f32::EPSILON {
			let g = position - origin;
			let t = g.dot(&self.normal) / d;
			if t > t0 && t < t1 {
				return Some(t)
			}
		}
		None
	}
}



/// Generates ray directions for each pixel in a thingy
pub fn ray_spread(
	rotation: UnitQuaternion<f32>,
	width: u32, 
	height: u32, 
	fovy: f32,
) -> Vec<Vector3<f32>> {
	let coords = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).collect::<Vec<_>>();

	let near = 1.0 / (fovy.to_radians() / 2.0).tan();
	// println!("near is {near}");
	let directions = coords.iter().map(|&(x, y)| {
		rotation * Vector3::new(
			(((x as f32 + 0.5) / width as f32) - 0.5) * 2.0,
			-(((y as f32 + 0.5) / height as f32) - 0.5) * 2.0,
			near,
		).normalize()
	}).collect::<Vec<_>>();

	directions
}



pub fn trace_octree(
	origin: Vector3<f32>,
	directions: &Vec<Vector3<f32>>,
	albedo: &mut Vec<[f32; 4]>,
	depth: &mut Vec<f32>,
	volume: &crate::octree::Octree<usize>,
	volume_position: Vector3<f32>,
	volume_palette: &Vec<[f32; 4]>,
	distance: f32,
) {
	let new_data = directions.iter().enumerate()
		.filter_map(|(i, &direction)| {
			if let Some((st, en)) = volume.aa_intersect(origin, direction, volume_position, 0.0, distance) {
				// Filter nothingburgers
				if depth[i] < st {
					None
				} else {
					Some((i, direction, (st, en)))
				}
			} else {
				None
			}
		})
		.map(|(i, direction, (st, en))| {
			let hit_pos = origin + direction * (st + 0.05);
			let rel_hit_pos = hit_pos - volume_position;

			let max1 = en - st;
			let max2 = distance - st;

			let mut iiter = AWIter::new(
				rel_hit_pos, 
				direction, 
				0.0, 
				f32::min(max1, max2),
				1.0,
			);

			// Mark initial miss as red
			if !volume.in_bounds([iiter.vx, iiter.vy, iiter.vz]) {
				return (i, [1.0, 0.0, 0.0, 0.0], st+iiter.t)
			}
			loop {
				if let Some(&g) = volume.get([iiter.vx, iiter.vy, iiter.vz]) {
					return (i, volume_palette[g], st+iiter.t)
					// return (i, [0.0, 0.0, 1.0, 0.0], st+iiter.t)
				}

				// Mark out of cast length as green
				if !iiter.next().is_some() {
					return (i, [0.0, 1.0, 0.0, 0.0], st+iiter.t)
				}

				// Mark out of bounds as white
				if !volume.in_bounds([iiter.vx, iiter.vy, iiter.vz]) {
					return (i, [1.0, 1.0, 1.0, 0.0], st+iiter.t)
				}
			}
		}).collect::<Vec<_>>();
	
	for (i, new_albedo, new_depth) in new_data {
		if depth[i] > new_depth {
			depth[i] = new_depth;
			albedo[i] = new_albedo;
		}
	}

}



/// An iterator for Fast Voxel Traversal
#[derive(Debug)]
pub struct AWIter {
	origin: Vector3<f32>,
	direction: Vector3<f32>,
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
	pub normal: Vector3<f32>,
}
impl AWIter {
	pub fn new(
		origin: Vector3<f32>,
		direction: Vector3<f32>,
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
			origin,
			direction,
			vx, vy, vz,
			v_step_x, v_step_y, v_step_z,
			t_delta_x, t_delta_y, t_delta_z, 
			t_max_x, t_max_y, t_max_z, 
			t: 0.0,
			t_max,
			normal: Vector3::zeros(),
		}
	}
}
impl Iterator for AWIter {
	type Item = ();

	fn next(&mut self) -> Option<Self::Item> {

		if self.t_max_x < self.t_max_y {
			if self.t_max_x < self.t_max_z {
				self.normal = Vector3::new(-self.v_step_x as f32, 0.0, 0.0);
				self.vx += self.v_step_x;
				self.t = self.t_max_x;
				self.t_max_x += self.t_delta_x;
				
			} else {
				self.normal = Vector3::new(0.0, 0.0, -self.v_step_z as f32);
				self.vz += self.v_step_z;
				self.t = self.t_max_z;
				self.t_max_z += self.t_delta_z;
			}
		} else {
			if self.t_max_y < self.t_max_z {
				self.normal = Vector3::new(0.0, -self.v_step_y as f32, 0.0);
				self.vy += self.v_step_y;
				self.t = self.t_max_y;
				self.t_max_y += self.t_delta_y;
			} else {
				self.normal = Vector3::new(0.0, 0.0, -self.v_step_z as f32);
				self.vz += self.v_step_z;
				self.t = self.t_max_z;
				self.t_max_z += self.t_delta_z;
			}
		}

		if self.t <= self.t_max {
			Some(())
		} else {
			None
		}
	}
}
