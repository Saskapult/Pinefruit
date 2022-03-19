use noise::{NoiseFn, Perlin};
use splines::{Interpolation, Key, Spline};
use anyhow::*;
use rand::prelude::*;
use rand_xoshiro::Xoshiro256PlusPlus;


/// Various noisy things


pub fn octave_perlin_2d(
	perlin: &Perlin,
	input: [f64; 2],
	octaves: u32,
	persistence: f64,	// 0.5 is good
	lacunarity: f64,	// 2.0 is good
) -> f64 {
	let mut amplitude = 1.0;
	let mut frequency = 1.0;
	let mut total = 0.0;
	let mut max_total = 0.0;
	for _ in 0..octaves {
		let mut adjusted_input = input.clone();
		adjusted_input.iter_mut().for_each(|v| *v *= frequency);

		total += (perlin.get(adjusted_input) / 2.0 + 0.5) * amplitude;
		max_total += amplitude;
		amplitude *= persistence;
		frequency *= lacunarity;
	}
	total / max_total
}
pub fn octave_perlin_3d(
	perlin: &Perlin,
	input: [f64; 3],
	octaves: u32,
	persistence: f64,	// 0.5 is good
	lacunarity: f64,	// 2.0 is good
) -> f64 {
	let mut amplitude = 1.0;
	let mut frequency = 1.0;
	let mut total = 0.0;
	let mut max_total = 0.0;
	for _ in 0..octaves {
		let mut adjusted_input = input.clone();
		adjusted_input.iter_mut().for_each(|v| *v *= frequency);

		total += (perlin.get(adjusted_input) / 2.0 + 0.5) * amplitude;
		max_total += amplitude;
		amplitude *= persistence;
		frequency *= lacunarity;
	}
	total / max_total
}



fn squashfactor_fn(
	val: f64, 
	pt: f64,
	centre: f64, 
	squash_function: impl Fn(f64) -> f64, // Needs to handle negative numbers too
) -> f64 {
	let distance = centre - pt;
	let output = val * squash_function(distance);
	output
}



fn squashup_linear(
	pt: f64,
	centre: f64, 
	max_distance: f64,
) -> f64 {
	let distance = centre - pt;
	let p = distance / max_distance;
	if p > 1.0 {
		p
	} else if p < 0.0 {
		p
	} else {
		pt * p
	}
}



fn squashfactor_spline(
	pt: f64,
	centre: f64,
	spline: Spline<f64, f64>,
) -> Result<f64> {
	let distance = centre - pt;
	Ok(spline.clamped_sample(distance).unwrap())
}



fn linear_spline() -> Spline<f64, f64> {
	let st = Key::new(0.0, 0.0, Interpolation::Linear);
	let en = Key::new(1.0, 1.0, Interpolation::Linear);
	let spline = Spline::from_vec(vec![st, en]);
	spline
}



const BLUE_FREQUENCY: f64 = 50.0;
pub fn blue_noise_picker_2d(
	perlin: &Perlin, 
	pos: [i32; 2],
	scale: [f64; 2],
	r: u32,
) -> bool {
	let r = r as i32;
	let here = perlin.get([
		pos[0] as f64 / scale[0] * BLUE_FREQUENCY + 0.5, 
		pos[1] as f64 / scale[1] * BLUE_FREQUENCY + 0.5,
	]);
	for x in (pos[0] - r)..(pos[0] + r) {
		for y in (pos[1] - r)..(pos[1] + r) {
			let sample = perlin.get([
				x as f64 / scale[0] * BLUE_FREQUENCY + 0.5, 
				y as f64 / scale[1] * BLUE_FREQUENCY + 0.5,
			]);
			if sample > here {
				return false
			}
		}
	}
	true
}



// seed should be based on some variable position, maybe world seed plus chunk sum(x,y,z)
pub fn xoshiro_2d(seed: u64, width: u32, height: u32) -> Vec<f64> {
	let mut x = Xoshiro256PlusPlus::seed_from_u64(seed);
	(0..(width*height)).map(|_| x.gen::<f64>()).collect::<Vec<_>>()
}



pub fn xoshiro_blue_2d(
	seed: u64,
	width: u32,
	height: u32,
	r: u32,
) -> Vec<bool> {
	let adjusted_width = width + r * 2;
	let adjusted_height = height + r * 2;

	// Generate values
	let data = xoshiro_2d(seed, adjusted_width, adjusted_height);
		// .chunks_exact(adjusted_width as usize)
		// .map(|row| Vec::from(row))
		// .collect::<Vec<_>>();

	let tester = |x: u32, y: u32| {
		let here = data[((x*adjusted_width) + y) as usize];
		for sx in (x-r)..=(x+r) {
			for sy in (y-r)..=(y+r) {
				let sample = data[((sx*adjusted_width) + sy) as usize];
				if sample > here {
					return false
				}
			}
		}
		true
	};

	let st_row = r;
	let en_row = height + r;
	let st_col = r;
	let en_col = width + r;

	// Test stuff
	let mut bmap = Vec::new();
	for y in st_row..en_row {
		for x in st_col..en_col {
			bmap.push(tester(x, y))
		}
	}

	bmap
}



#[inline(always)]
pub fn xoshiro_hash_rng_3d(base_seed: u64, position: [i32; 3]) -> f64 {
	let a = Xoshiro256PlusPlus::seed_from_u64(base_seed + position[0] as u64).gen::<u64>();
	let b = Xoshiro256PlusPlus::seed_from_u64(a + position[1] as u64).gen::<u64>();
	Xoshiro256PlusPlus::seed_from_u64(b + position[2] as u64).gen::<f64>()
}
#[inline(always)]
pub fn xoshiro_hash_rng_2d(base_seed: u64, position: [i32; 2]) -> f64 {
	let a = Xoshiro256PlusPlus::seed_from_u64(base_seed + position[0] as u64).gen::<u64>();
	Xoshiro256PlusPlus::seed_from_u64(a + position[1] as u64).gen::<f64>()
}



#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Instant;
	use rayon::prelude::*;

	// This test shows that parallel stuff is good stuff
	#[test]
	fn testyy() {
		const WIDTH: u32 = 3000;
		const HEIGHT: u32 = 3000;

		println!("Noise start");
		let st = Instant::now();
		let output = (0..WIDTH*HEIGHT).into_par_iter().map(|v| {
			let x = v % WIDTH;
			let y = v / HEIGHT;
			xoshiro_hash_rng_2d(0, [x as i32, y as i32])
		}).collect::<Vec<_>>();
		let en = Instant::now();
		println!("Noise done in {:?}", (en-st));

		let img = image::DynamicImage::ImageRgb8(
			image::ImageBuffer::from_vec(WIDTH, HEIGHT, output.iter().flat_map(|&f| {
				[(f * u8::MAX as f64) as u8; 3]
			}).collect::<Vec<_>>()).unwrap()
		);

		crate::util::show_image(img).unwrap();

		assert_eq!(2 + 2, 4);
	}

    #[test]
    fn chunk_seed_test() {

		const WIDTH: u32 = 30;
		const HEIGHT: u32 = 30;
		const R: u32 = 1;
		
		let output = xoshiro_blue_2d(0, WIDTH, HEIGHT, R);

		let img = image::DynamicImage::ImageRgb8(
			image::ImageBuffer::from_vec(WIDTH, HEIGHT, output.iter().flat_map(|&b| {
				if b {
					[u8::MAX; 3]
				} else {
					[u8::MIN; 3]
				}
			}).collect::<Vec<_>>()).unwrap()
		);

		crate::util::show_image(img).unwrap();

        assert_eq!(2 + 2, 4);
    }
}
