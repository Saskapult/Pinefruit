use glam::*;


pub trait Intersect<Other> {
	type IOutput;
	fn intersect(&self, other: &Other) -> Option<Self::IOutput>;
}



struct RayPointLight {
	pub position: Vec3,
	pub radius: f32,
	pub colour: [f32; 3],
}



pub struct Ray {
	pub origin: Vec3,
	pub direction: Vec3,
}
impl Ray {
	pub fn new(origin: Vec3, direction: Vec3) -> Self {
		Self { origin, direction: direction.normalize() }
	}
}



pub struct Sphere {
	pub position: Vec3,
	pub radius: f32,
}



#[derive(Debug, Clone)]
pub struct OBB {
	pub aabb: AABB,
	pub orientation: Quat,
}
impl OBB {
	pub fn corners(&self) -> [Vec3; 8] {
		let n = &self.aabb.min;
		let p = &self.aabb.max;
		let nnn = Vec3::new(n[0], n[1], n[2]);
		let nnp = Vec3::new(n[0], n[1], p[2]);
		let npn = Vec3::new(n[0], p[1], n[2]);
		let npp = Vec3::new(n[0], p[1], p[2]);
		let pnn = Vec3::new(p[0], n[1], n[2]);
		let pnp = Vec3::new(p[0], n[1], p[2]);
		let ppn = Vec3::new(p[0], p[1], n[2]);
		let ppp = Vec3::new(p[0], p[1], p[2]);
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
		origin: Vec3, 
		direction: Vec3, 
		position: Vec3, 
		t0: f32, 
		t1: f32, 
	) -> Option<(f32, f32)> {
		let poisiton_relative_to_ray = position - origin;

		let mut t_min = t0;
		let mut t_max = t1;

		for i in 0..3 {
			let axis = self.orientation * Vec3::new(
				if i==0 { 1.0 } else { 0.0 }, 
				if i==1 { 1.0 } else { 0.0 }, 
				if i==2 { 1.0 } else { 0.0 },
			);
			let e = axis.dot(poisiton_relative_to_ray);
			let f = direction.dot(axis);
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
	pub min: Vec3,
	pub max: Vec3,
}
impl AABB {
	pub fn new(
		aabb_min: Vec3,
		aabb_max: Vec3,
	) -> Self {
		Self {
			min: aabb_min, max: aabb_max,
		}
	}

	pub fn extent(&self) -> Vec3 {
		(self.max - self.min).abs() / 2.0
	}

	pub fn centre(&self) -> Vec3 {
		self.min + self.extent()
	}

	// Todo: handle div by zero
	// https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-box-intersection
	#[inline]
	pub fn ray_intersect(
		&self, 
		origin: Vec3,
		direction: Vec3,
		position: Vec3, 
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

	pub fn contains(&self, point: Vec3) -> bool {
		point.cmpge(self.min).all() && point.cmple(self.max).all()
	}

	pub fn mid_planes(&self) -> [Plane; 3] {
		let centre = self.centre();
		[
			Plane {
				normal: Vec3::Z,
				distance: centre[2],
			},
			Plane {
				normal: Vec3::Y,
				distance: centre[1],
			},
			Plane {
				normal: Vec3::X,
				distance: centre[0],
			},
		]
	}
}



#[derive(Debug, Clone)]
pub struct Plane {
	pub normal: Vec3,
	pub distance: f32,
}
impl Plane {
	// Restricted to along positive line direction
	pub fn ray_intersect(
		&self, 
		origin: Vec3,
		direction: Vec3,
		position: Vec3, 
		t0: f32, // Min distance
		t1: f32, // Max distance
	) -> Option<f32> {
		let d = self.normal.dot(direction);
		if d > f32::EPSILON {
			let g = position - origin;
			let t = g.dot(self.normal) / d;
			if t > t0 && t < t1 {
				return Some(t)
			}
		}
		None
	}
}



/// Generates ray directions for each pixel in a thingy
pub fn ray_spread(
	rotation: Quat,
	width: u32, 
	height: u32, 
	fovy: f32,
) -> Vec<Vec3> {
	let coords = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).collect::<Vec<_>>();

	let near = 1.0 / (fovy.to_radians() / 2.0).tan();
	// println!("near is {near}");
	let directions = coords.iter().map(|&(x, y)| {
		rotation * Vec3::new(
			(((x as f32 + 0.5) / width as f32) - 0.5) * 2.0,
			-(((y as f32 + 0.5) / height as f32) - 0.5) * 2.0,
			near,
		).normalize()
	}).collect::<Vec<_>>();

	directions
}
