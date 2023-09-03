use std::collections::HashMap;

use slotmap::SlotMap;

use crate::{EntityIdentifier, TextureKey, BufferKey, RenderContextKey};



#[derive(Debug)]
pub struct RenderContext<T: EntityIdentifier> {
	pub name: String, // For debugging
	// Systems take this entity and extract data from it to make context data
	// Like camera buffer system, which looks for a camera component and then
	// Writes its data to the camera buffer (a context resource)
	pub entity: Option<T>,

	pub textures: HashMap<String, TextureKey>,
	pub buffers: HashMap<String, BufferKey>,
}
impl<T: EntityIdentifier> RenderContext<T> {
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			entity: None,
			textures: HashMap::new(),
			buffers: HashMap::new(),
		}
	}

	pub fn with_entity(mut self, entity_id: T) -> Self {
		self.entity = Some(entity_id);
		self
	}

	// Todo: make this rebuild materials
	pub fn insert_texture(&mut self, label: impl Into<String>, key: TextureKey) {
		if self.textures.insert(label.into(), key).is_some() {
			panic!("You've just replaced a context resource! Any materials that used it will not be notified and things will break. Have a nice day!");
		}
	}

	// Todo: make this rebuild materials
	pub fn insert_buffer(&mut self, label: impl Into<String>, key: BufferKey) {
		let label = label.into();
		info!("context buffer {label}");
		if self.buffers.insert(label, key).is_some() {
			panic!("You've just replaced a context resource! Any materials that used it will not be notified and things will break. Have a nice day!");
		}
	}
	
	pub fn texture(&self, id: impl Into<String>) -> Option<TextureKey> {
		self.textures.get(&id.into()).copied()
	}
}

// Context initialization systems
// Render systems which use context (all use context?)

// pub 

#[derive(Debug, Default)]
pub struct RenderContextManager<T: EntityIdentifier> {
	pub render_contexts: SlotMap<RenderContextKey, RenderContext<T>>,
}
impl<T: EntityIdentifier> RenderContextManager<T> {
	pub fn new() -> Self {
		Self {
			render_contexts: SlotMap::with_key(),
		}
	}

	pub fn insert(&mut self, context: RenderContext<T>) -> RenderContextKey {
		if self.render_contexts.len() != 0 {
			panic!("tell me why");
		}
		self.render_contexts.insert(context)
	}

	pub fn get(&self, key: RenderContextKey) -> Option<&RenderContext<T>> {
		self.render_contexts.get(key)
	}

	pub fn get_mut(&mut self, key: RenderContextKey) -> Option<&mut RenderContext<T>> {
		self.render_contexts.get_mut(key)
	}


}
