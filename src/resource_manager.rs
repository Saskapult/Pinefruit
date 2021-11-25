
use std::collections::HashMap;
use std::num::NonZeroU32;
use crate::texture::Texture;



pub struct IndexMap<T> {
	pub data: Vec<T>,
	pub data_index: HashMap<String, usize>,
}
impl<T> IndexMap<T> {
	pub fn new() -> Self {
		let data = Vec::new();
		let data_index = HashMap::new();
		Self {
			data,
			data_index,
		}
	}

	pub fn insert(&mut self, name: &String, data: T) -> usize {
		// If name exists, update
		if self.data_index.contains_key(name) {
			let idx = self.data_index[name];
			self.data[idx] = data;
			return idx
		}
		// Else load
		let idx = self.data.len();
		self.data_index.insert(name.clone(), idx);
		self.data.push(data);
		idx
	}

	// Get resource by name
	pub fn get_name(&self, name: &String) -> &T {
		let idx = self.data_index[name];
		self.get_index(idx)
	}

	// Get resource by index
	pub fn get_index(&self, idx: usize) -> &T {
		&self.data[idx]
	}
}



// The texture array bind group layout needs to know the length of the texture array
// The render pipeline needs to know the bind group layout
// Fortunately for us, wgpu lets us use bind groups with less than the count specified in the bind group layout entry
// Specifying a big length allows us to avoid reconstructing the pipeline if we add a new texture
// The upper limit depends on the host system and must be requested during device creation
// The default limit (works on all systems) is 16, which is bad
// max_sampled_textures_per_shader_stage is 1,048,576 on my system (gtx 1050m?), which is good
const MAX_TEXTURES: u32 = 1024;
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
					count: NonZeroU32::new(MAX_TEXTURES as u32),
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





pub trait ResourceManager<R> {
	fn get(&self, id: usize) -> &R;
	fn get_byname(&self, name: &String) -> &R;
	fn get_id_byname(&self, name: &String) -> usize;
	fn register(&mut self, name: &String, data: R) -> usize;
}


// In the future it may be optimal to let this manager move items between GPU, RAM, and disk
// For now this implementation is fine
pub struct TextureManager {
	pub textures: IndexMap<Texture>,
}
impl TextureManager {
	pub fn new() -> Self {
		let textures = IndexMap::new();
		Self {
			textures,
		}
	}
}
impl ResourceManager<Texture> for TextureManager {
	fn get(&self, id: usize) -> &Texture {
		&self.textures.data[id]
	}
	fn get_byname(&self, name: &String) -> &Texture {
		&self.textures.data[self.textures.data_index[name]]
	}
	fn get_id_byname(&self, name: &String) -> usize {
		self.textures.data_index[name]
	}
	fn register(&mut self, name: &String, data: Texture) -> usize {
		self.textures.insert(name, data)
	}
}

