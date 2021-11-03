

pub trait Drawable {
	pub fn draw<'a>(rp: &mut wgpu::RenderPass<'a>, uniforms: &'a wgpu::BindGroup) -> Result<()>;
}

