use std::{time::Duration, path::Path, sync::{Mutex, Arc}};
use image::DynamicImage;
use anyhow::*;
use std::process::Command;
use splines::*;




/// Holds durations, can find average and median
#[derive(Debug)]
pub struct DurationHolder {
	num_to_hold: usize,
	durations: Vec<Duration>,
	durations_index: usize,
}
impl DurationHolder {
	pub fn new(num_to_hold: usize) -> Self {
		Self {
			num_to_hold,
			durations: Vec::with_capacity(num_to_hold),
			durations_index: num_to_hold-1,
		}
	}

	pub fn reset(&mut self) {
		self.durations = Vec::with_capacity(self.num_to_hold);
		self.durations_index = 0;
	}

	pub fn resize(&mut self, new_size: usize) {
		if new_size - self.num_to_hold > 0 {
			self.durations.reserve(new_size - self.num_to_hold);
		}
		if self.durations_index >= new_size {
			self.durations_index = 0;
		}
		self.num_to_hold = new_size;
	}

	pub fn record(&mut self, duration: Duration) {
		self.durations_index = (self.durations_index + 1) % self.num_to_hold;
		if self.durations_index < self.durations.len() {
			self.durations[self.durations_index] = duration;
		} else {
			self.durations.push(duration);
		}
	}

	pub fn latest(&self) -> Option<Duration> {
		if self.durations.len() == 0 {
			None
		} else {
			Some(self.durations[self.durations_index])
		}
	}

	pub fn average(&self) -> Option<Duration> {
		if self.durations.len() == 0 {
			None
		} else {
			Some(self.durations.iter().sum::<Duration>() / (self.durations.len() as u32))
		}
	}

	pub fn median(&self) -> Option<Duration> {
		if self.durations.len() == 0 {
			None
		} else {
			let mut sorted_durations = self.durations.clone();
			sorted_durations.sort_unstable();

			if sorted_durations.len() % 2 == 0 {
				Some((sorted_durations[sorted_durations.len()/2] + sorted_durations[sorted_durations.len()/2+1]) / 2)
			} else {
				Some(sorted_durations[sorted_durations.len()/2])
			}
		}
		
	}
}


// pub trait RapierConvertable<T> {
// 	fn to(input: T) -> T;
// 	fn from(input: T) -> T;
// }

// impl<V> RapierConvertable<V> for [V; 3] {
// 	fn to(mut input: [V; 3]) -> [V; 3] {
// 		input
// 	}
// 	fn from(mut input: [V; 3]) -> [V; 3] {
// 		input
// 	}
// }

// Rapier uses a y-up coordinate system
/// Switches y for z
pub fn k_tofrom_rapier(mut input: nalgebra::Vector3<f32>) -> nalgebra::Vector3<f32> {
	let y = input[1];
	input[1] = input[2];
	input[2] = y;
	input
}



/// Shows an image by saving it to tmp and opening it with gwenview
// Todo: Make an iterator of prorams to try?
const IMAGE_PATH: &str = "/tmp/kkraftimagetoshow.png";
const IMAGE_VIEWER: &str = "gwenview";
pub fn show_image(image: DynamicImage) -> Result<()> {
	image.save(IMAGE_PATH)?;

	Command::new(IMAGE_VIEWER)
		.arg(IMAGE_PATH)
		.output()?;
	
	Ok(())
}



/// Saves a spline to a ron file
pub fn save_spline(
	spline: &Spline<f64, f64>, 
	path: impl AsRef<Path>,
) -> Result<()> {
	let path = path.as_ref();

	let f = std::fs::File::create(&path)
		.with_context(|| format!("Failed to write file path '{:?}'", &path))?;
	ron::ser::to_writer(f, spline)
		.with_context(|| format!("Failed to write spline ron file '{:?}'", &path))?;
	
	Ok(())
}



/// Loads a spline from a ron file
pub fn load_spline(path: impl AsRef<Path>) -> Result<Spline<f64, f64>> {
	let path = path.as_ref();
	let canonical_path = path.canonicalize()
		.with_context(|| format!("Failed to canonicalize path '{:?}'", &path))?;
	
	let f = std::fs::File::open(&path)
		.with_context(|| format!("Failed to read from file path '{:?}'", &canonical_path))?;
	let spline: Spline<f64, f64> = ron::de::from_reader(f)
		.with_context(|| format!("Failed to parse spline ron file '{:?}'", &canonical_path))?;
	
	Ok(spline)
}


pub fn show_spline(s: Spline<f64, f64>, height: u32, width: Option<u32>) -> Result<()> {
	let x_min = s.keys().iter().map(|k| k.t).reduce(|accum, v| accum.min(v)).unwrap();
	let x_max = s.keys().iter().map(|k| k.t).reduce(|accum, v| accum.max(v)).unwrap();
	let y_min = s.keys().iter().map(|k| k.value).reduce(|accum, v| accum.min(v)).unwrap();
	let y_max = s.keys().iter().map(|k| k.value).reduce(|accum, v| accum.max(v)).unwrap();

	println!("x: {x_min} -> {x_max}");
	println!("y: {y_min} -> {y_max}");

	let x_range = x_max - x_min;
	let y_range = y_max - y_min;
	
	let width = match width {
		Some(w) => w,
		None => {
			let aspect = x_range / y_range;
			let w = (aspect * height as f64) as u32;
			println!("derived width: {w}");
			w
		}
	};

	let output = (0..width).map(|x| {
		// let x = (v % width) as i32;
		// let y = height as i32 - (v / width) as i32;
		let xp = x as f64 / width as f64;
		let yp = 1.0 - (s.sample(x_min + xp * x_range).unwrap() - y_min) / y_range;
		// y_min + yp * y_range
		yp
	}).collect::<Vec<_>>();

	// println!("{output:?}");

	let mut imb = image::ImageBuffer::new(width, height);
	
	// output.iter()
	// 	.map(|&f| (f * (height-1) as f64).floor() as u32)
	// 	.enumerate()
	// 	.for_each(|(px, py)| {
	// 		imb[(px as u32, py)] = [u8::MAX; 3].into();
	// 	});
	
	// https://www.javatpoint.com/computer-graphics-bresenhams-line-algorithm
	let mut bresenham = |x1: i32, y1: i32, x2: i32, y2: i32| {
		// 4
		let dx = x2 - x1;
		let dy = y2 - y1;
		let i1 = 2 * dy;
		let i2 = 2 * (dy - dx);
		let mut d = i1 - dx;

		// 5
		let (mut x, mut y, x_end) = 
		if dx < 0 {
			(x2, y2, x1)
		} else if dx > 0 {
			(x1, y1, x2)
		} else {
			panic!()
		};

		// 6
		imb[(x as u32, y as u32)] = [u8::MAX; 3].into();

		// 7
		while x < x_end {

			// 8
			if d < 0 {
				d += i1;
			} else {
				d += i2;
				y += 1;
			}

			// 9
			x += 1;

			// 10
			imb[(x as u32, y as u32)] = [u8::MAX; 3].into();

			// 11
		}
	};
	let pxy = output.iter()
		.map(|&f| (f * (height-1) as f64).floor() as u32)
		.enumerate()
		// .step_by(4)
		.map(|(x, y)| (x as i32, y as i32))
		.collect::<Vec<_>>();
	pxy.iter().zip(pxy[1..].iter()).for_each(|(&(x1, y1), &(x2, y2))| {
		bresenham(x1, y1, x2, y2)
	});
	// bresenham(5, 5, 20, 60);

	let img = image::DynamicImage::ImageRgb8(imb);
	
	show_image(img)?;
	Ok(())
}



/// Pollable. Threadish. Checker. Thing.
#[derive(Debug, Clone)]
pub struct PTCT<T: std::fmt::Debug> {
	result: Arc<Mutex<Option<T>>>
}
impl<T: std::fmt::Debug> PTCT<T> {
	pub fn new() -> Self {
		Self { result: Arc::new(Mutex::new(None)) }
	}

	pub fn pollmebb(&mut self) -> Option<T> {
		let mut res = self.result.lock().unwrap();
		if res.is_some() {
			Some(res.take().unwrap())
		} else {
			None
		}
	}

	pub fn insert(&mut self, thing: T) {
		let mut res = self.result.lock().unwrap();
		*res = Some(thing);
	}
}



#[cfg(test)]
mod tests {
	use super::*;

	fn make_test_spline() -> Spline<f64, f64> {
		let k1 = Key::new(0.0, 0.0, Interpolation::Linear);
		let k2 = Key::new(1.0, 1.0, Interpolation::Linear);
		let spline = Spline::from_vec(vec![k1, k2]);
		spline
	}

	#[test]
	fn test_show_spline() {
		let s = Spline::from_vec(vec![
			Key::new(0.0, -20.0, Interpolation::Cosine),
			Key::new(0.1, 0.5, Interpolation::default()),
			Key::new(1.0, 20.0, Interpolation::Cosine),
		]);

		show_spline(s, 1024, Some(1024)).unwrap();

		assert!(true);
	}

	#[test]
	fn huh() {
		let v = vec![
			1,2,3,4,
		];

		v.iter().zip(v[1..].iter()).for_each(|v| println!("{v:?}"));

		assert!(true);
	}

    #[test]
    fn test_spline_serde() {
		let spline1 = make_test_spline();

		let spline_path = "/tmp/splinetime.ron";
		save_spline(&spline1, spline_path).unwrap();
		let spline2 = load_spline(spline_path).unwrap();

        assert_eq!(spline1.keys(), spline2.keys());
    }
}