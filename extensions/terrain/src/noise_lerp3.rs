use glam::{IVec3, UVec3, Vec3};
use crate::generator::RawFbmSettings;
use crate::generator::ConfigureRawFbm;


fn xyz_major_iterator(size: UVec3) -> impl Iterator<Item = UVec3> {
	let [x, y, z] = size.to_array();
	(0..z).flat_map(move |z| {
		(0..y).flat_map(move |y| {
			(0..x).map(move |x| {
				UVec3::new(x, y, z)
			})
		})
	})
} 


#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
	a * (1.0 - t) + b * t
}


#[inline]
fn lerp3(
	xt: f32, yt: f32, zt: f32, 
	q000: f32, q001: f32, q010: f32, q011: f32, q100: f32, q101: f32, q110: f32, q111: f32, 
) -> f32 {
	// let xt = (x2 - x) / (x2 - x1);
	let x00 = lerp(q000, q100, xt);
	let x10 = lerp(q010, q110, xt);
	let x01 = lerp(q001, q101, xt);
	let x11 = lerp(q011, q111, xt);

	let r0 = lerp(x00, x01, zt);
	let r1 = lerp(x10, x11, zt);
	
	lerp(r0, r1, yt)
}


#[inline]
fn xyz_major_index(x: u32, y: u32, z: u32, scale: UVec3) -> u32 {
	z * scale.x * scale.y + y * scale.x + x
}


/// Lerp coefficients are precomuted and stored on the stack. 
/// This constant dictates the maximum scale. 
/// 
/// TODO: const generics
const MAX_SCALE: usize = 8;


/// Generates FBM noise and scales it by a factor. 
/// Profiling has revealed that this is faster than generating more data! 
/// Returns a flattened xyz-major 3d vector of values in (approximately) [0, 1]. 
// TODO: 
//  clamp samiling positions to scaled grid
//  adjust settings frequency to generate the same shapes at different resolutions
pub fn fbm_scaled_linear(
	mut settings: RawFbmSettings, 
	pos: IVec3,
	extent: UVec3, 
	chonkularity: UVec3,
) -> Vec<f32> {
	assert!(chonkularity.to_array().into_iter().all(|v| v <= MAX_SCALE as u32), "Max chonkularity exceeded!");
	assert!((extent % chonkularity).to_array().into_iter().all(|v| v == 0), "Extent must be divisible by chonkularity!");

	// Adjust frequency to account for chonkulation
	// TODO: Update simdnoise to allow for per-axis frequency scaling
	settings.freq *= chonkularity.x as f32;

	let inner_size = extent / chonkularity;

	let adjusted_pos = pos / chonkularity.as_ivec3();
	let [xf, yf, zf] = (adjusted_pos.as_vec3() + Vec3::splat(0.5)).to_array();
	// Samples are extended by one 
	let samples_size = inner_size + UVec3::ONE;
	let [width, height, depth] = samples_size.to_array();
	let mut samples = simdnoise::NoiseBuilder::fbm_3d_offset(
		xf, width as usize,
		yf, height as usize,
		zf, depth as usize,
	).apply_raw_settings(settings).generate().0;
	{ // Normalize [0, 1]
		let scale = settings.compute_scale();
		samples.iter_mut().for_each(|v| *v = (*v * scale + 1.0) / 2.0);
	}

	let mut interpolated = vec![0.0; extent.element_product() as usize];

	// We only operate on fixed values of t and each involves a floating-point
	// division, so I tried precomputing them! 
	// Profiling has revealed that this was a good idea 
	let mut precomputed_t = [[0.0; MAX_SCALE]; 3];
	for (i, sz) in chonkularity.to_array().into_iter().enumerate() {
		for j in 0..sz {
			precomputed_t[i][j as usize] = j as f32 / sz as f32;
		}
	}

	for (i, pos) in xyz_major_iterator(extent).enumerate() {
		// Option to skip at base values
		// Profiling has shown this to be detrimental! (weird!)
		if false {
			if (pos % inner_size).element_sum() == 0 {
				continue 
			}
		}

		// Base sample position
		let [x, y, z] = (pos / chonkularity).to_array();
		let q000 = samples[xyz_major_index(x, y, z, samples_size) as usize];
		let q001 = samples[xyz_major_index(x, y, z+1, samples_size) as usize];
		let q010 = samples[xyz_major_index(x, y+1, z, samples_size) as usize];
		let q011 = samples[xyz_major_index(x, y+1, z+1, samples_size) as usize];
		let q100 = samples[xyz_major_index(x+1, y, z, samples_size) as usize];
		let q101 = samples[xyz_major_index(x+1, y, z+1, samples_size) as usize];
		let q110 = samples[xyz_major_index(x+1, y+1, z, samples_size) as usize];
		let q111 = samples[xyz_major_index(x+1, y+1, z+1, samples_size) as usize];

		// Option to fetch rather than compute 
		// Please do profiling 
		// Profiling has shown this to be beneficial! 
		let [xt, yt, zt] = if false {
			[
				pos.x as f32 / x as f32,
				pos.y as f32 / y as f32,
				pos.z as f32 / z as f32,
			]
		} else {
			// Distance to the base sample cell 
			let d = pos - (pos / chonkularity) * chonkularity;
			[
				precomputed_t[0][d.x as usize],
				precomputed_t[1][d.y as usize],
				precomputed_t[2][d.z as usize],
			]
		};

		let v = lerp3(xt, yt, zt, q000, q001, q010, q011, q100, q101, q110, q111);
		interpolated[i] = v;
	}

	interpolated
}


#[cfg(test)]
pub mod tests {
	use super::*;
	use glam::UVec3;
	use test::Bencher;

	/// Tests that my magic scaling number is still working 
	#[test]
	fn test_noise_normalization() {
		let settings = RawFbmSettings {
			seed: 0,
			freq: 1.0,
			lacunarity: 1.0,
			gain: 2.5,
			octaves: 6,
		};

		let extent = 256;
		let noise = fbm_scaled_linear(settings, IVec3::ZERO, UVec3::splat(extent), UVec3::ONE);
		
		assert!(noise.iter().copied().all(|v| v <= 1.0));
		assert!(noise.iter().copied().all(|v| v >= 0.0));
	}

	fn get_bench_settings() -> RawFbmSettings {
		RawFbmSettings {
			seed: 42,
			freq: 1.0 / 50.0,
			lacunarity: 2.0,
			gain: 0.5,
			octaves: 3,
		}
	}

	fn bench_fbm_lerp(settings: RawFbmSettings, extent: UVec3, scale: UVec3, b: &mut Bencher) {
		b.iter(|| {
			let world_pos = rand::random::<IVec3>();
			fbm_scaled_linear(settings, world_pos, extent, scale)
		});
	}

	#[bench]
	fn bench_fbm_16_lerp_8(b: &mut Bencher) {
		bench_fbm_lerp(get_bench_settings(), UVec3::splat(16), UVec3::splat(2), b);
	}

	#[bench]
	fn bench_fbm_32_lerp_8(b: &mut Bencher) {
		bench_fbm_lerp(get_bench_settings(), UVec3::splat(32), UVec3::splat(4), b);
	}

	#[bench]
	fn bench_fbm_32_lerp_16(b: &mut Bencher) {
		bench_fbm_lerp(get_bench_settings(), UVec3::splat(32), UVec3::splat(2), b);
	}

	fn bench_fbm_base(settings: RawFbmSettings, extent: UVec3, b: &mut Bencher) {
		b.iter(|| {
			let pos = rand::random::<IVec3>();
			let [xf, yf, zf] = (pos.as_vec3() + Vec3::splat(0.5)).to_array();
			let [width, height, depth] = extent.to_array();
			let mut samples = simdnoise::NoiseBuilder::fbm_3d_offset(
				xf, width as usize + 1,
				yf, height as usize + 1,
				zf, depth as usize + 1,
			).apply_raw_settings(settings).generate().0;
			{
				let scale = settings.compute_scale();
				samples.iter_mut().for_each(|v| *v *= scale);
			}
			samples
		});
	}

	#[bench]
	fn bench_fbm_16_base(b: &mut Bencher) {
		bench_fbm_base(get_bench_settings(), UVec3::splat(16), b);
	}	

	#[bench]
	fn bench_fbm_32_base(b: &mut Bencher) {
		bench_fbm_base(get_bench_settings(), UVec3::splat(32), b);
	}
}
