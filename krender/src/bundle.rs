use std::collections::HashMap;
use wgpu_profiler::GpuProfiler;
use crate::{rendertarget::SpecificRenderTarget, ShaderKey, BindGroupKey, MeshKey, TextureKey, shader::ShaderManager, bindgroup::BindGroupManager, mesh::{MeshManager, IndexBufferType}, texture::TextureManager};



#[derive(Debug)]
pub enum MaybeOwned<'a, T> {
	Owned(T),
	Borrowed(&'a T),
}


#[derive(Debug, Default)]
pub(crate) struct RenderBundleStage<'a> {
	pub targets: Vec<(
		SpecificRenderTarget, 
		Vec<(ShaderKey, Vec<(MaybeOwned<'a, wgpu::Buffer>, Vec<([Option<BindGroupKey>; 4], Option<MeshKey>, u32)>)>)>
	)>,
	pub computes: Vec<(ShaderKey, [Option<BindGroupKey>; 4], [u32; 3])>, // (shader, bgs, workgroup)
	pub attachment_clears: HashMap<TextureKey, wgpu::Color>, 
	pub depth_clears: HashMap<TextureKey, f32>, 
}


/// A render bundle is a collected form of a render input. 
/// It may be encoded onto a [wgpu::CommandEncoder]. 
#[derive(Debug, Default)]
pub struct RenderBundle<'a> {
	pub(crate) stages: Vec<RenderBundleStage<'a>>,
}
impl<'a> RenderBundle<'a> {

	// This is inaccurate, as it does not account for draw calls to clar textures
	// At the time of writing, however, I haven't implemented that, so it is accurate for now
	pub fn draw_count(&self) -> usize {
		let mut draw_count = 0;
		for stage in self.stages.iter() {
			for (_, shaders) in stage.targets.iter() {
				for (_, a) in shaders {
					for (_, g) in a {
						draw_count += g.len();
					}
				}
			}
		}
		draw_count
	}

	#[profiling::function]
	pub fn execute(
		&self,
		device: &wgpu::Device,
		shaders: &ShaderManager,
		bind_groups: &BindGroupManager,
		meshes: &MeshManager,
		textures: &TextureManager,
		encoder: &mut wgpu::CommandEncoder,
		profiler: &mut GpuProfiler,
	) {
		for (i, bundle) in self.stages.iter().enumerate() {
			profiling::scope!("stage");
			profiler.begin_scope(&*format!("Stage {i}"), encoder, device);
	
			for (&key, range) in bundle.attachment_clears.iter() {
				profiling::scope!("clear");
				info!("Clearing texture {key:?}");
				// let texture = textures.get(key).unwrap();
				// encoder.clear_texture(&texture.binding().unwrap().texture, range);
				todo!("do another render pass to clear the texture?")
			}
			for (key, value) in bundle.depth_clears.iter() {
				profiling::scope!("clear");
				info!("Clearing texture {key:?}");
				todo!("do another render pass to clear the texture?")
			}
		
			for &(key, bgs, [x,y,z]) in bundle.computes.iter() {
				profiling::scope!("compute");
				let shader = shaders.get(key).unwrap();
				let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: None,
				});

				pass.set_pipeline(shader.pipeline.as_ref().unwrap().compute().unwrap());
				for (index, key) in bgs.iter().enumerate() {
					if let &Some(key) = key {
						let bind_group = bind_groups.get(key).unwrap();
						pass.set_bind_group(index as u32, bind_group, &[]);
					}
				}

				pass.dispatch_workgroups(x, y, z);
			}
		
			for (target, groups) in bundle.targets.iter() {
				profiling::scope!("target");
				let color_attachments = target.colour_attachments.iter().map(|&(attachment, resolve, ops)| {
					Some(wgpu::RenderPassColorAttachment {
						view: &textures.get(attachment)
							.expect("texture not found")
							.binding()
							.expect("texture not bound").view,
						resolve_target: resolve.and_then(|r| Some(&textures.get(r)
							.unwrap()
							.binding()
							.unwrap().view)),
						ops,
					})
				}).collect::<Vec<_>>();

				let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None, 
					color_attachments: color_attachments.as_slice(), 
					depth_stencil_attachment: target.depth_attachment.and_then(|(d, depth_ops)| {
						Some(wgpu::RenderPassDepthStencilAttachment {
							view: &textures.get(d)
								.unwrap()
								.binding()
								.unwrap().view,
							depth_ops: Some(depth_ops),
							stencil_ops: None,
						})
					}), 
				});

				for (shader_key, batches) in groups.iter() {
					profiling::scope!("shader");
					let shader = shaders.get(*shader_key).unwrap();
					trace!("Shader {}", shader.specification.name);
					profiler.begin_scope(&*format!("Shader '{}'", shader.specification.name), &mut pass, device);

					pass.set_pipeline(shader.pipeline.as_ref().unwrap().polygon().unwrap());

					let mut current_bgs = [None; 4];
					for (instance_buffer, items) in batches {
						// Set instance buffer
						let buffer_slice = match instance_buffer {
							MaybeOwned::Owned(b) => b.slice(..),
							MaybeOwned::Borrowed(b) => b.slice(..),
						};
						pass.set_vertex_buffer(1, buffer_slice);
	
						// Draw
						let mut current_count = 0;
						for &(bgs, mesh, count) in items {
							// Set bind groups
							for (index, key) in bgs.iter().enumerate() {
								// If some and not the same
								if let &Some(key) = key {
									if current_bgs[index] != Some(key) {
										current_bgs[index] = Some(key);
										let bind_group = bind_groups.get(key).unwrap();
										pass.set_bind_group(index as u32, bind_group, &[]);
									}
								}
							}

							// Set mesh
							let (indexed, n) = match mesh {
								Some(key) => {
									let mesh = meshes.get(key).unwrap();
									// trace!("Binding mesh {:?}", mesh.name);
									let mesh_format_key = shader.mesh_format_key.unwrap();
									let mesh_buffer = meshes.vertex_bindings.get(mesh_format_key)
										.expect("mesh bindings doesn't contain this format!")
										.get(key)
										.expect("Mesh was not queued for binding!")
										.as_ref()
										.expect("Mesh not bound!");
									pass.set_vertex_buffer(0, mesh_buffer.slice(..));

									let index_buffer = meshes.index_bindings.get(key)
										.expect("Mesh was not queued for binding!")
										.as_ref()
										.expect("Mesh not bound!");
									match index_buffer {
										IndexBufferType::U32(b) => {
											pass.set_index_buffer(b.slice(..), wgpu::IndexFormat::Uint32);
											(true, mesh.indices.as_ref().unwrap().len() as u32)
										},
										IndexBufferType::U16(b) => {
											pass.set_index_buffer(b.slice(..), wgpu::IndexFormat::Uint16);
											(true, mesh.indices.as_ref().unwrap().len() as u32)
										}
										IndexBufferType::None => (false, mesh.n_vertices),
									}
								},
								// If we haven't been given a mesh, then the shader is generative
								// Fullquad, for example, has 3 vertices
								None => (false, shader.specification.base.polygonal().polygon_input.generative_vertices()),
							};

							// Draw
							if indexed {
								pass.draw_indexed(0..n, 0, current_count..current_count+count)
							} else {
								pass.draw(0..n, current_count..current_count+count);
							}
							current_count += count;
						}
					}
					profiler.end_scope(&mut pass);
				}
			}
			profiler.end_scope(encoder);
		}
	}
}