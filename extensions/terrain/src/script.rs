use std::{collections::HashMap, path::Path};
use splines::Spline;



#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NoiseSpecificationFBM {
	frequency: f32,
	lacunarity: f32, 
	gain: f32,
	octaves: u8, 
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum GenOp<VAR, NOISE, SPLINE> {
	Noise3D(VAR, NOISE),
	Noise2D(VAR, NOISE),
	Spline(VAR, VAR, SPLINE),
	Set(VAR, f32),
	Add(VAR, VAR, VAR),
	Sub(VAR, VAR, VAR),
	Mul(VAR, VAR, VAR),
	Div(VAR, VAR, VAR),
}
type RawOp = GenOp<String, String, String>;
type MappedOp = GenOp<usize, usize, usize>;


// Base 
// Ops 
// Memory 


pub struct ScriptMap {
	variables_map: HashMap<String, usize>,
	fbm: Vec<NoiseSpecificationFBM>,
	fbm_map: HashMap<String, usize>,
	splines: Vec<Spline<f32, f32>>,
	splines_map: HashMap<String, usize>,
}

pub struct Script(Vec<MappedOp>);

pub struct ScriptMemory(Vec<f32>);


pub struct CompiledScript {
	variables_map: HashMap<String, usize>,

	fbm: Vec<NoiseSpecificationFBM>,
	fbm_map: HashMap<String, usize>,

	splines: Vec<Spline<f32, f32>>,
	splines_map: HashMap<String, usize>,
	
	// Split maps/values and operations? Allows for shared memory
	// Do it later idc
	operations: Vec<MappedOp>,
}
impl CompiledScript {
	pub fn try_from_ops(path: impl AsRef<Path>) -> anyhow::Result<Self> {
		let s = std::fs::read_to_string(path.as_ref())?;
		let ops: Vec<RawOp> = ron::de::from_str(&s)?;
		
		let mut variables_map = HashMap::new();
		let mut var_or_insert = |id| {
			if let Some(&i) = variables_map.get(&id) {
				i
			} else {
				let i = variables_map.len();
				variables_map.insert(id, i);
				i
			}
		};

		let mut fbm = Vec::new();
		let mut fbm_map = HashMap::new();
		let mut fbm_or_insert = |id: String| {
			if let Some(&i) = fbm_map.get(&id) {
				anyhow::Ok(i)
			} else {
				let s = std::fs::read_to_string(&id)?;
				let g = ron::de::from_str(&s)?;
				let i = fbm.len();
				fbm.push(g);
				fbm_map.insert(id, i);
				Ok(i)
			}
		};

		let mut splines = Vec::new();
		let mut splines_map = HashMap::new();
		let mut spline_or_insert = |id: String| {
			if let Some(&i) = splines_map.get(&id) {
				anyhow::Ok(i)
			} else {
				let s = std::fs::read_to_string(&id)?;
				let g = ron::de::from_str(&s)?;
				let i = splines.len();
				splines.push(g);
				splines_map.insert(id, i);
				Ok(i)
			}
		};

		let operations = ops.into_iter().map(|o| match o {
			GenOp::Noise3D(var, noise) => {
				let var_i = var_or_insert(var);
				let noise_i = fbm_or_insert(noise)?;
				Ok(GenOp::Noise3D(var_i, noise_i))
			},
			GenOp::Noise2D(var, noise) => {
				let var_i = var_or_insert(var);
				let noise_i = fbm_or_insert(noise)?;
				Ok(GenOp::Noise2D(var_i, noise_i))
			},
			GenOp::Spline(i, j, spline) => {
				let spline_i = spline_or_insert(spline)?;
				Ok(GenOp::Spline(var_or_insert(i), var_or_insert(j), spline_i))
			},
			GenOp::Set(var, val) => {
				let var_i = var_or_insert(var);
				Ok(GenOp::Set(var_i, val))
			},
			GenOp::Add(a, b, c) => {
				Ok(GenOp::Div(var_or_insert(a), var_or_insert(b), var_or_insert(c)))
			},
			GenOp::Sub(a, b, c) => {
				Ok(GenOp::Sub(var_or_insert(a), var_or_insert(b), var_or_insert(c)))
			},
			GenOp::Mul(a, b, c) => {
				Ok(GenOp::Mul(var_or_insert(a), var_or_insert(b), var_or_insert(c)))
			},
			GenOp::Div(a, b, c) => {
				Ok(GenOp::Div(var_or_insert(a), var_or_insert(b), var_or_insert(c)))
			},
		}).collect::<anyhow::Result<Vec<_>>>()?;
		
		Ok(Self {
			variables_map, fbm, fbm_map, splines, splines_map, operations, 
		})
	}

	pub fn exec(&mut self, memory: &mut [f32]) {
		for op in self.operations.iter() {
			match op {
				&GenOp::Noise3D(i, n) => todo!("Find density and pos indices"),
				&GenOp::Noise2D(i, n) => todo!("Find density and pos indices"),
				&GenOp::Spline(i, j, n) => memory[i] = self.splines[i].clamped_sample(memory[j]).unwrap(),
				&GenOp::Set(a, b) => memory[a] = b,
				&GenOp::Add(a, b, c) => memory[a] = memory[b] + memory[c],
				&GenOp::Sub(a, b, c) => memory[a] = memory[b] - memory[c],
				&GenOp::Mul(a, b, c) => memory[a] = memory[b] * memory[c],
				&GenOp::Div(a, b, c) => memory[a] = memory[b] / memory[c],
			}
		}
	}
}
