use arrayvec::ArrayVec;
use crate::{TextureKey, rendercontext::RenderContext, EntityIdentifier, texture::TextureManager, buffer::BufferManager, BufferKey};



/// Used to identify a (potentially context-specific) render resource. 
/// 
/// In the future, this could use ArrayStrings instead of Strings. 
/// This may give performance improvments, but benchmarking would be needed to know that for sure.
/// 
/// It's called RRID and not RenderResourceIdentifier becuse that made everything way too verbose. 
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum RRID {
	Global(String),
	Context(String),
}
impl RRID {
	pub fn global(id: impl Into<String>) -> Self {
		Self::Global(id.into())
	}
	
	pub fn context(id: impl Into<String>) -> Self {
		Self::Context(id.into())
	}

	pub fn texture<T: EntityIdentifier>(&self, context: &RenderContext<T>, textures: &TextureManager) -> Option<TextureKey> {
		match self {
			RRID::Global(id) => textures.key_by_name(id),
			RRID::Context(id) => context.texture(id),
		}
	}

	pub fn buffer<T: EntityIdentifier>(&self, context: &RenderContext<T>, buffers: &BufferManager) -> Option<BufferKey> {
		match self {
			RRID::Global(id) => buffers.key(id),
			RRID::Context(id) => todo!(),
		}
	}
}


/// Specifies render targets using resource identifier, not using their keys. 
/// This makes it more abstract. 
/// When supplied with a [RenderContext], this can produce a [SpecificRenderTarget]
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct AbstractRenderTarget {
	pub colour_attachments: ArrayVec<(RRID, Option<RRID>), 4>, // attachment, resolve 
	pub depth_attachment: Option<RRID>, // Can derive 'store' based on associated shaders
}
impl AbstractRenderTarget {
	pub fn new() -> Self {
		Self {
			colour_attachments: ArrayVec::new(),
			depth_attachment: None,
		}
	}
	
	pub fn with_colour(mut self, attachment: RRID, resolve: Option<RRID>) -> Self {
		self.colour_attachments.push((attachment, resolve));
		self
	}

	pub fn with_depth(mut self, depth: RRID) -> Self {
		self.depth_attachment = Some(depth);
		self
	}

	pub(crate) fn specify<T: EntityIdentifier>(
		&self, 
		context: &RenderContext<T>,
		textures: &TextureManager,
	) -> SpecificRenderTarget {
		// let map_thing = |t: &RRID| match t {
		// 	RRID::Global(id) => textures.key_by_name(id),
		// 	RRID::Context(id) => context.texture(id),
		// };

		let colour_attachments = self.colour_attachments.iter()
			.map(|(t, r)| {
				let t = t.texture(context, textures).expect("Failed to locate target texture (todo: give more information)");
				let r = r.as_ref().and_then(|t| 
					Some(t.texture(context, textures).expect("Failed to locate target texture (todo: give more information)")));
				(t, r, wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: true,
				})
			}).collect::<ArrayVec<_, 4>>();

		let depth_attachment = self.depth_attachment.as_ref().and_then(|t| 
			Some((t.texture(context, textures).expect("Failed to locate target texture (todo: give more information)"), wgpu::Operations {
				load: wgpu::LoadOp::Load,
				store: true,
			})));

		SpecificRenderTarget { colour_attachments, depth_attachment, }
	}
}


/// This is [AbstrctRenderTarget] given a [RenderContext]. 
/// Also includes load operations, which just load the texture by default. 
/// When preparing to render, you should look ahead and set them to clear if it must be cleared. 
#[derive(Debug, Clone)]
pub(crate) struct SpecificRenderTarget {
	pub colour_attachments: ArrayVec<(TextureKey, Option<TextureKey>, wgpu::Operations<wgpu::Color>), 4>,
	// Operations should be optional and also we should have another one for the stencil ops
	pub depth_attachment: Option<(TextureKey, wgpu::Operations<f32>)>,
}
