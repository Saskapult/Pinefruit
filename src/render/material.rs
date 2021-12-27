
use std::{
	collections::HashMap,
	num::NonZeroU32,
	path::PathBuf,
	sync::Arc,
};
use crate::indexmap::*;
use crate::render::*;
use serde::{Serialize, Deserialize};




#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSpecification {
	pub name: String,
	pub shader: PathBuf,
	pub textures: HashMap<String, Vec<PathBuf>>,
	pub values: HashMap<String, Vec<f32>>,
}



pub fn load_materials_file(
	path: &PathBuf,
) -> Vec<MaterialSpecification> {
	let f = std::fs::File::open(path).expect("Failed to open file");
	let info: Vec<MaterialSpecification> = ron::de::from_reader(f).expect("Failed to read materials ron file");
	info
}



pub struct Material {
	pub name: String,
	pub shader_id: usize,
	pub bind_group_id: usize,
	pub source: PathBuf, // The file which this material was read from
}



pub struct MaterialManager {
	device: Arc<wgpu::Device>,
	queue: Arc<wgpu::Queue>,
	pub materials: SonOfIndexMap<String, Material>,
	pub bind_groups: SonOfIndexMap<String, wgpu::BindGroup>,
}
impl MaterialManager {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();
		let materials = SonOfIndexMap::new();
		let bind_groups = SonOfIndexMap::new();
		Self { 
			device, 
			queue, 
			materials,
			bind_groups, 
		}
	}
}











// The texture array bind group layout needs to know the length of the texture array
// The render pipeline needs to know the bind group layout
// Fortunately for us, wgpu lets us use bind groups with less than the count specified in the bind group layout entry
// Specifying a big length allows us to avoid reconstructing the pipeline if we add a new texture
// The upper limit depends on the host system and must be requested during device creation
// The default limit (works on all systems) is 16, which is bad
// max_sampled_textures_per_shader_stage is 1,048,576 on my system (gtx 1050m?), which is good
const ARRAY_MAX_TEXTURES: u32 = 1024;
pub struct TextureArrayManager {
	pub textures: IndexMap<Texture>,
	pub bind_group_layout: wgpu::BindGroupLayout,
	pub sampler: wgpu::Sampler,
}
impl TextureArrayManager {
	pub fn new(
		device: &wgpu::Device, 
		queue: &wgpu::Queue, 
	) -> Self {
		let mut textures = IndexMap::new();

		// Not specific to an instance, I just don't know where to put it
		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("texture array bind group layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: NonZeroU32::new(ARRAY_MAX_TEXTURES as u32),
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler {
						comparison: false,
						filtering: true,
					},
					count: None,
				},
			],
		});

		let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

		Self {
			textures,
			bind_group_layout,
			sampler,
		}
	}

	pub fn get_views(&self) -> Vec<&wgpu::TextureView> {
		let mut texture_views = Vec::new();
		for texture in &self.textures.data {
			texture_views.push(&texture.view);
		}
		texture_views
	}

	pub fn make_bg(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::BindGroup {
		let texture_views = self.get_views();
		
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureViewArray(texture_views.as_slice()),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
			layout: &self.bind_group_layout,
			label: Some("texture array bind group"),
		})
	}
}


pub fn make_tarray_bg(
	device: &wgpu::Device, 
	textures: Vec<&Texture>, 
	sampler: &wgpu::Sampler,
	bind_group_layout: &wgpu::BindGroupLayout
) -> wgpu::BindGroup {
	let mut texture_views = Vec::new();
	for texture in &textures {
		texture_views.push(&texture.view);
	}

	device.create_bind_group(&wgpu::BindGroupDescriptor {
		entries: &[
			wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::TextureViewArray(texture_views.as_slice()),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: wgpu::BindingResource::Sampler(sampler),
			},
		],
		layout: bind_group_layout,
		label: Some("texture array bind group"),
	})
}



#[cfg(test)]
mod tests {
	use super::*;

	fn create_example_material() -> MaterialSpecification {
		let mut textures = HashMap::new();
		let albedo = ["g.png", "f.png"].iter().map(|s| PathBuf::from(&s)).collect::<Vec<_>>();
		textures.insert("albedo".to_string(), albedo);
		MaterialSpecification {
			name: "example material".into(),
			shader: "exap_shader.ron".into(),
			values: HashMap::new(),
			textures,
		}
	}

	#[test]
	fn test_serialize() {
		let data = create_example_material();
		let pretty = ron::ser::PrettyConfig::new()
			.depth_limit(3)
			.separate_tuple_members(true)
			.enumerate_arrays(false);
		let s = ron::ser::to_string_pretty(&data, pretty).expect("Serialization failed");
		println!("{}", s);
		assert!(true);
	}
}







// use std::path::PathBuf;
// use image::DynamicImage;
// use std::sync::Arc;
// pub struct RenderResourceManager {
// 	device: Arc<wgpu::Device>,
// 	queue: Arc<wgpu::Queue>,
// 	pipelines: IndexMap<wgpu::RenderPipeline>,
// 	pipeline_layouts: IndexMap<wgpu::PipelineLayout>,
// 	material_layouts: IndexMap<wgpu::BindGroupLayout>,
// 	materials: IndexMap<wgpu::BindGroup>,
// 	textures: IndexMap<Texture>,
// }
// impl RenderResourceManager {
// 	pub fn new(
// 		device: Arc<wgpu::Device>,
// 		queue: Arc<wgpu::Queue>,
// 	) -> Self {
// 		let bmat_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
// 			label: Some("material bgl"),
// 			entries: &[
// 				wgpu::BindGroupLayoutEntry {
// 					binding: 0,
// 					visibility: wgpu::ShaderStages::FRAGMENT,
// 					ty: wgpu::BindingType::Texture {
// 						sample_type: wgpu::TextureSampleType::Float { filterable: true },
// 						view_dimension: wgpu::TextureViewDimension::D2,
// 						multisampled: false,
// 					},
// 					count: None,
// 				},
// 				wgpu::BindGroupLayoutEntry {
// 					binding: 1,
// 					visibility: wgpu::ShaderStages::FRAGMENT,
// 					ty: wgpu::BindingType::Sampler {
// 						comparison: false,
// 						filtering: true,
// 					},
// 					count: None,
// 				},
// 			],
// 		});
// 		let mata_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
// 			label: Some(&format!("array material bgl (size {})", max_len)),
// 			entries: &[
// 				wgpu::BindGroupLayoutEntry {
// 					binding: 0,
// 					visibility: wgpu::ShaderStages::FRAGMENT,
// 					ty: wgpu::BindingType::Texture {
// 						sample_type: wgpu::TextureSampleType::Float { filterable: true },
// 						view_dimension: wgpu::TextureViewDimension::D2,
// 						multisampled: false,
// 					},
// 					count: NonZeroU32::new(max_len as u32),
// 				},
// 				wgpu::BindGroupLayoutEntry {
// 					binding: 1,
// 					visibility: wgpu::ShaderStages::FRAGMENT,
// 					ty: wgpu::BindingType::Sampler {
// 						comparison: false,
// 						filtering: true,
// 					},
// 					count: None,
// 				},
// 			],
// 		})
// 	}
	

// 	pub fn new_pipeline(
// 		&mut self,
// 		vshader: &wgpu::ShaderModule,
// 		fshader: &wgpu::ShaderModule,
// 	) {

// 	}


// 	pub fn new_material_array(
// 		&mut self,
// 		name: &String,
// 		bgli: usize,
// 		textures: Vec<usize>,
// 	) -> wgpu::BindGroup {
// 		// Collect texture views
// 		let mut texture_views = Vec::new();
// 		for ti in textures {
// 			let texture_view = self.textures.get_index(ti).view;
// 			texture_views.push(&texture_view);
// 		}

// 		let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor::default());
// 		let layout = self.material_layouts.get_index(bgli);
// 		let material = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
// 			entries: &[
// 				wgpu::BindGroupEntry {
// 					binding: 0,
// 					resource: wgpu::BindingResource::TextureViewArray(texture_views.as_slice()),
// 				},
// 				wgpu::BindGroupEntry {
// 					binding: 1,
// 					resource: wgpu::BindingResource::Sampler(&sampler),
// 				},
// 			],
// 			layout,
// 			label: Some(name),
// 		});

// 		material
// 	}


// 	pub fn load_texture_disk(
// 		&mut self,
// 		name: &String,
// 		path: &PathBuf
// 	) -> usize {
// 		let image = image::open(path)
// 			.expect("Failed to open file");
// 		self.load_texture(name, image)
// 	}


// 	pub fn load_texture(
// 		&mut self,
// 		name: &String,
// 		data: DynamicImage,
// 	) -> usize {
// 		let texture = Texture::from_image(&self.device, &self.queue, &data, Some(name))
// 			.expect("Failed to create texture");
// 		self.textures.insert(name, texture)
// 	}


// 	// Creates a new material layout
// 	pub fn bmat_bgl(
// 		&self,
// 	) -> wgpu::BindGroupLayout {
		
// 	}


// 	// Creates a new material array layout
// 	pub fn mata_bgl(
// 		&self, 
// 		max_len: usize
// 	) -> wgpu::BindGroupLayout {
		
// 	}
// }


