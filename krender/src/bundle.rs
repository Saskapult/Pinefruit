use std::collections::HashMap;
use crate::{rendertarget::RenderTarget, ShaderKey, BindGroupKey, MeshKey, TextureKey, shader::ShaderManager, bindgroup::BindGroupManager, mesh::MeshManager, texture::TextureManager};


#[derive(Debug, Default)]
struct RenderBundleStage {
	pub targets: HashMap<
		RenderTarget, 
		HashMap<ShaderKey, Vec<(wgpu::Buffer, Vec<([Option<BindGroupKey>; 4], Option<MeshKey>, u32)>)>>
	>,
	pub computes: Vec<(ShaderKey, [Option<BindGroupKey>; 4], [u32; 3])>, // (shader, bgs, workgroup)
	// On target change check if clear exits for target index, clear if true
	pub clears: HashMap<TextureKey, wgpu::ImageSubresourceRange>, 
}


/// A render bundle is a collected form of a render input. 
/// It may be encoded onto a [wgpu::CommandEncoder]. 
#[derive(Debug, Default)]
pub struct RenderBundle {
	stages: Vec<RenderBundleStage>,
}
impl RenderBundle {
	#[profiling::function]
	pub fn execute(
		&self,
		shaders: &ShaderManager,
		bind_groups: &BindGroupManager,
		meshes: &MeshManager,
		textures: &TextureManager,
		encoder: &mut wgpu::CommandEncoder,
	) {
		for bundle in self.stages.iter() {
			profiling::scope!("stage");

			{
				profiling::scope!("Texture Clears");
				for (&key, range) in bundle.clears.iter() {
					info!("Clearing texture {key:?}");
					let texture = textures.get(key).unwrap();
					encoder.clear_texture(&texture.binding().unwrap().texture, range);
				}
			}

			{
				profiling::scope!("Compute Items");
				for &(key, bgs, [x,y,z]) in bundle.computes.iter() {
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
			}

			{
			profiling::scope!("Polygonal Items");
			for (target, groups) in bundle.targets.iter() {
				let color_attachments = target.colour_attachments.iter().map(|&(attachment, resolve)| {
					Some(wgpu::RenderPassColorAttachment {
						view: &textures.get(attachment).unwrap().binding().unwrap().view,
						resolve_target: resolve.and_then(|r| Some(&textures.get(r).unwrap().binding().unwrap().view)),
						ops: wgpu::Operations { 
							load: wgpu::LoadOp::Load, 
							store: true, // Assume that we will write to any colour attachement
						},
					})
				}).collect::<Vec<_>>();

				let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None, 
					color_attachments: color_attachments.as_slice(), 
					depth_stencil_attachment: target.depth_attachment.and_then(|d| {
						Some(wgpu::RenderPassDepthStencilAttachment {
							view: &textures.get(d).unwrap().binding().unwrap().view,
							depth_ops: Some(wgpu::Operations { 
								load: wgpu::LoadOp::Load, 
								store: true, // Another assumption that we will write
							}),
							stencil_ops: None,
						})
					}), 
				});

				for (&shader_key, batches) in groups.iter() {
					for (instance_buffer, items) in batches {
						pass.set_vertex_buffer(1, instance_buffer.slice(..));

						let shader = shaders.get(shader_key).unwrap();
						info!("Shader {}", shader.specification.name);
						info!("has layout {:?}", shader.pipeline_layout.as_ref().unwrap());
						pass.set_pipeline(shader.pipeline.as_ref().unwrap().polygon().unwrap());
	
						println!("{:#?}", bind_groups);
	
						let mut current_bgs = items[0].0;
						for (index, key) in current_bgs.iter().enumerate() {
							if let &Some(key) = key {
								let bind_group = bind_groups.get(key).unwrap();	
								trace!("Binding bind group {key:?} to index {index}");
								pass.set_bind_group(index as u32, bind_group, &[]);
							}
						}
						let mut current_count = 0;
						for &(bgs, mesh, count) in items {
							// Set bind groups if needed
							for (index, &key) in bgs.iter().enumerate() {
								if current_bgs[index] != key {
									if let Some(key) = key {
										let bind_group = bind_groups.get(key).unwrap();	
										trace!("Binding bind group {key:?} to index {index}");
	
										pass.set_bind_group(index as u32, bind_group, &[]);
										current_bgs[index] = Some(key);
									}
									
								}
							}
	
							let n_vertices = match mesh {
								Some(key) => {
									let mesh = meshes.get(key).unwrap();
									trace!("Binding mesh {:?}", mesh.name);
									let mesh_format_key = shader.mesh_format_key.unwrap();
									let mesh_buffer = mesh.bindings.as_ref().unwrap().vertex_buffers.get(mesh_format_key).unwrap();
									pass.set_vertex_buffer(0, mesh_buffer.slice(..));
									mesh.n_vertices
								},
								// If we haven't been given a mesh, then the shader is generative
								// Fullquad, for example, has 3 vertices
								None => shader.specification.base.polygonal().polygon_input.generative_vertices(),
							};
							
							trace!("Drawing {} thing(s)!", n_vertices);
							pass.draw(0..n_vertices, current_count..current_count+count);
							current_count += count;
						}
					}
					
				}

				trace!("End render target");
			}
			}
		}
		trace!("End execution");
	}
}