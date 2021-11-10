
use std::collections::HashMap;
use std::num::NonZeroU32;
use image::DynamicImage;
use crate::render::*;
use std::path::PathBuf;



pub struct BlockTexturesManager {
	pub texture_data: Vec<DynamicImage>, 			// Texture data in ram
	pub texture_data_index: HashMap<String, u32>,	// Maps texture names to index in texture_data
	pub texture_bind_group_layout: wgpu::BindGroupLayout,
	pub texture_bind_group: wgpu::BindGroup,
}
impl BlockTexturesManager {
	pub fn new(device: &wgpu::Device, queue: &wgpu::Queue,) -> Self {
		let texture_data = Vec::new();
		let texture_data_index = HashMap::new();
		let (texture_bind_group_layout, texture_bind_group) = make_array_texture(&device, &queue, &texture_data);

		Self {
			texture_data,
			texture_data_index,
			texture_bind_group_layout,
			texture_bind_group,
		}
	}

	pub fn add_from_disk(&mut self, name: &String, path: &PathBuf) {
		let data = image::open(path).expect("Fug");
		// These two must happen atomicly
		self.texture_data_index.insert(name.clone(), self.texture_data.len() as u32);
		self.texture_data.push(data);
	}

	pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue,) {
		let (bgl, bg) = make_array_texture(&device, &queue, &self.texture_data);
		self.texture_bind_group_layout = bgl;
		self.texture_bind_group = bg;
	}
	
}


// Remakes the array texture
fn make_array_texture(
	device: &wgpu::Device, 
	queue: &wgpu::Queue, 
	texture_data: &Vec<DynamicImage>,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
	// Cheat and load test data
	let r = image::open("resources/blockfaces/dirt.png").expect("Fug");
	let g = image::open("resources/blockfaces/sand.png").expect("Fug");
	let TEMPY = [r, g].to_vec();
	let (width, height) = TEMPY[0].to_rgba8().dimensions();

	let texture_size = wgpu::Extent3d {
		width,
		height,
		depth_or_array_layers: TEMPY.len() as u32,
	};

	// Create the texture stuff
	let texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("textures"),
		size: texture_size,
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8UnormSrgb,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
	});
	let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
		dimension: Some(wgpu::TextureViewDimension::D2Array),
		..Default::default()
	});
	let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

	// Concatenate the texture data
	let mut collected_rgba = Vec::new();
	for tdata in &TEMPY {
		let rgba = tdata.to_rgba8();
		let mut raw = rgba.into_raw();
		collected_rgba.append(&mut raw);
	}
	// Pass the texture data to the gpu
	queue.write_texture(
		texture.as_image_copy(),
		&collected_rgba[..], // &[u8]
		wgpu::ImageDataLayout {
			offset: 0,
			bytes_per_row: Some(NonZeroU32::new(4*width).unwrap()), // r, g, b, a
			rows_per_image: Some(NonZeroU32::new(height).unwrap()),
		},
		texture_size,
	);

	let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		entries: &[
			wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					multisampled: false,
					view_dimension: wgpu::TextureViewDimension::D2Array,
					sample_type: wgpu::TextureSampleType::Float { filterable: true },
				},
				count: None,
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
		label: Some("texture_bind_group_layout"),
	});
	let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		layout: &texture_bind_group_layout,
		entries: &[
			wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::TextureView(&texture_view),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: wgpu::BindingResource::Sampler(&sampler),
			},
		],
		label: Some("texture_bind_group"),
	});

	(texture_bind_group_layout, texture_bind_group)
}
