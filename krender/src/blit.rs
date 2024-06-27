

#[derive(Debug)]
pub struct Blitter {
	pipeline: wgpu::RenderPipeline,
	bgl: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
}
impl Blitter {
	pub fn new(
		device: &wgpu::Device,
		output_format: wgpu::TextureFormat,
	) -> Self {

		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("Blit sampler"),
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("blit bgl"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture { 
						sample_type: wgpu::TextureSampleType::Float { 
							filterable: true,
						}, 
						view_dimension: wgpu::TextureViewDimension::D2, 
						multisampled: false, 
					},
					count: std::num::NonZeroU32::new(1),
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: std::num::NonZeroU32::new(1),
				},
			]
		});

		let pld = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("blit pipeline descriptor"),
			bind_group_layouts: &[
				&bgl,
			],
			push_constant_ranges: &[],
		});

		let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("blit shader"),
			source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
		});

		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("blit pipeline"),
			layout: Some(&pld),
			vertex: wgpu::VertexState {
				module: &module,
				entry_point: "vs_main",
				buffers: &[],
			},
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: None,
				unclipped_depth: false,
				polygon_mode: wgpu::PolygonMode::Fill,
				conservative: false,
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			fragment: Some(wgpu::FragmentState {
				module: &module,
				entry_point: "fs_main",
				targets: &[Some(output_format.into()),],
			}),
			multiview: None,
		});

		Self {
			pipeline, bgl, sampler, 
		}
	}

	pub fn blit(
		&self,
		device: &wgpu::Device,
		encoder: &mut wgpu::CommandEncoder,
		source_view: &wgpu::TextureView,
		destination_view: &wgpu::TextureView,
	) {

		let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("blit bg"),
			layout: &self.bgl,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(source_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
		});

		let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("blit"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: destination_view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: true,
				}
			})],
			depth_stencil_attachment: None,
		});

		rp.set_pipeline(&self.pipeline);
		rp.set_bind_group(0, &bg, &[]);
		rp.draw(0..3, 0..1);
	}
}

