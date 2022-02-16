use noise::{NoiseFn, Perlin};




pub fn octave_perlin_2d(
	perlin: &Perlin,
	input: [f64; 2],
	octaves: u32,
	persistence: f64,
) -> f64 {
	let mut amplitude = 1.0;
	let mut frequency = 1.0;
	let mut total = 0.0;
	let mut max_total = 0.0;
	for _ in 0..octaves {
		let mut adjusted_input = input.clone();
		adjusted_input.iter_mut().for_each(|v| *v *= frequency);

		total += perlin.get(input) * amplitude;
		max_total += amplitude;
		amplitude *= persistence;
		frequency *= 2.0;
	}
	total / max_total
}
