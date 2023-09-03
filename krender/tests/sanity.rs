// use std::collections::HashMap;

// use krender::{prelude::*, prepare_for_render};

// mod display;
// use crate::display::show_image;


// fn get_devq() -> (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue) {
// 	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
// 		backends: wgpu::Backends::all(),
// 		dx12_shader_compiler: Default::default(),
// 	});
// 	let adapter = pollster::block_on(instance.request_adapter(
// 		&wgpu::RequestAdapterOptions {
// 			power_preference: wgpu::PowerPreference::HighPerformance, // Dedicated GPU
// 			compatible_surface: None,
// 			force_fallback_adapter: false, // Don't use software renderer
// 		},
// 	)).unwrap();
// 	let (device, queue) = pollster::block_on(adapter.request_device(
// 		&wgpu::DeviceDescriptor {
// 			label: Some("kkraft device descriptor"),
// 			features: wgpu::Features::default() 
// 				| wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
// 			limits: wgpu::Limits::default(),
// 		},
// 		None,
// 	)).unwrap();
// 	(instance, adapter, device, queue)
// }


// #[derive(Debug, Default)]
// struct TestingStorageProvider {
// 	storages: HashMap<String, TestingComponentProvider>,
// 	resources: HashMap<String, Vec<u8>>,
// }
// impl InstanceDataProvider<u32> for TestingStorageProvider {
// 	fn get_storage(&self, component_id: impl Into<String>) -> Option<&TestingComponentProvider> {
// 		self.storages.get(&component_id.into())
// 	}
// 	fn get_resource(&self, resource_id: impl Into<String>) -> Option<&[u8]> {
// 		self.resources.get(&resource_id.into()).and_then(|g| Some(g.as_slice()))
// 	}
// 	fn fetch_source(&self, attribute: &InstanceAttributeSource) -> Option<FetchedInstanceAttributeSource<u32>> {
// 		match attribute {
// 			InstanceAttributeSource::Component(component_id) => self.get_storage(component_id).and_then(|s| Some(FetchedInstanceAttributeSource::Component(Box::new(s)))),
// 			InstanceAttributeSource::Resource(resource_id) => self.get_resource(resource_id).and_then(|v| Some(FetchedInstanceAttributeSource::Resource(v.to_vec()))),
// 		}
// 	}
// }


// #[derive(Debug, Default)]
// struct TestingComponentProvider {
// 	data: HashMap<u32, Vec<u8>>,
// }
// impl InstanceComponentProvider<u32> for TestingComponentProvider {
// 	fn get_component(&self, entity_id: u32) -> Option<&[u8]> {
// 		self.data.get(&entity_id).and_then(|v| Some(v.as_slice()))
// 	}
// }


// #[test]
// fn execute() {
// 	env_logger::init();

// 	let (instance, adapter, device, queue) = get_devq();

// 	let mut shaders = ShaderManager::new();
// 	let mut materials = MaterialManager::new();
// 	let mut meshes = MeshManager::new();
// 	let mut textures = TextureManager::new();
// 	let mut buffers = BufferManager::new();
// 	let mut bind_groups = BindGroupManager::new();
// 	let mut meshes = MeshManager::new();
// 	let mut contexts = RenderContextManager::new();

// 	let context = {
// 		let mut context = RenderContext::new("default context")
// 		.with_entity(0_u32);
// 		let t = Texture::new("albedo", TextureFormat::Bgra8Unorm, 400, 300, 1, true);
// 		context.insert_texture("albedo", textures.insert(t));
// 		contexts.insert(context)
// 	};

// 	let _shader = shaders.insert(ShaderEntry::from_path("./tests/resources/shader.ron"));

// 	let material = materials.insert_specification(MaterialSpecification::read("./tests/resources/material.ron").unwrap());

// 	let mut storage_provider = TestingStorageProvider::default();
// 	storage_provider.resources.insert(
// 		"colour".to_string(), 
// 		bytemuck::bytes_of(&[0.5, 0.5, 0.75, 0.0]).to_vec(),
// 	);	

// 	let _colour_buffer = buffers.insert(Buffer::new("colour", 16, false));

// 	let mut input = RenderInput::new();
// 	input.insert_item("idk", material, None, 0);

// 	prepare_for_render(
// 		&device, 
// 		&queue, 
// 		&mut shaders, 
// 		&mut materials, 
// 		&mut meshes,
// 		&mut textures, 
// 		&mut buffers, 
// 		&mut bind_groups,
// 		&contexts,
// 	);

// 	let bundle = input.bundle(&device, &materials, &shaders, context, &storage_provider);

// 	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
// 		label: None,
// 	});

// 	bundle.execute(&shaders, &bind_groups, &meshes, &textures, &mut encoder);

// 	println!("Submit");
// 	let index = queue.submit(Some(encoder.finish()));

// 	println!("Wait");
// 	device.poll(wgpu::Maintain::WaitForSubmissionIndex(index));

// 	let k = contexts.get(context).unwrap().texture("albedo").unwrap();
// 	let t = textures.get(k).unwrap();

// 	show_image(instance, adapter, device, queue, &t.binding().unwrap().texture);
// }
