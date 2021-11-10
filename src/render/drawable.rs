use anyhow::*;

pub trait Drawable {
	fn draw<'a>(rp: &mut wgpu::RenderPass<'a>, uniforms: &'a wgpu::BindGroup) -> Result<()>;
}

// Set bind group 0 to render
// Set vertex buffer 0 to mesh vert buffer.slice(..)
// Set vertex buffer 1 to instance buffer
// Set index buffer to mesh index buffer