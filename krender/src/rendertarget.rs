use arrayvec::ArrayVec;
use crate::TextureKey;



#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct RenderTarget {
	pub colour_attachments: ArrayVec<(TextureKey, Option<TextureKey>), 4>, // attachment, resolve 
	pub depth_attachment: Option<TextureKey>, // Can derive 'store' based on associated shaders
}
impl RenderTarget {
	pub fn new() -> Self {
		Self {
			colour_attachments: ArrayVec::new(),
			depth_attachment: None,
		}
	}
	
	pub fn with_colour(mut self, attachement: TextureKey, resolve: Option<TextureKey>) -> Self {
		self.colour_attachments.push((attachement, resolve));
		self
	}

	pub fn with_depth(mut self, depth: TextureKey) -> Self {
		self.depth_attachment = Some(depth);
		self
	}
}


/// Same thign with operations
#[derive(Debug, Clone)]
pub(crate) struct RenderTargetOperations {
	pub colour_attachments: Vec<(TextureKey, Option<TextureKey>, wgpu::Operations<wgpu::Color>)>,
	// Operations should be optional and also we should have another one for the stencil ops
	pub depth_attachment: Option<(TextureKey, wgpu::Operations<f32>)>,
}
impl RenderTargetOperations {
	pub fn from_rt(rt: &RenderTarget) -> Self {
		let d = wgpu::Operations {
			load: wgpu::LoadOp::Load,
			store: true,
		};
		let d2 = wgpu::Operations {
			load: wgpu::LoadOp::Load,
			store: true,
		};
		Self {
			colour_attachments: rt.colour_attachments.iter().copied().map(|(k, v)| (k, v, d)).collect(),
			depth_attachment: rt.depth_attachment.and_then(|k| Some((k, d2))),
		}
	}
}
