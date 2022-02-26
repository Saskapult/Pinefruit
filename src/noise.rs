use noise::{NoiseFn, Perlin};
use splines::{Interpolation, Key, Spline};
use anyhow::*;




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
