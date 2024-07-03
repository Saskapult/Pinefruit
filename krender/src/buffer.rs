use parking_lot::RwLock;
use slotmap::SlotMap;
use std::{collections::{HashMap, HashSet}, sync::atomic::{AtomicBool, Ordering}};
use crate::{BufferKey, MaterialKey, RenderContextKey, BindGroupKey, prelude::BindGroupManager};



#[derive(Debug)]
pub struct BufferManager {
	buffers: SlotMap<BufferKey, Buffer>,
	buffers_by_id: HashMap<String, BufferKey>,
}
impl BufferManager {
	pub fn new() -> Self {
		Self {
			buffers: SlotMap::with_key(),
			buffers_by_id: HashMap::new(),
		}
	}

	pub fn insert(&mut self, buffer: Buffer) -> BufferKey {
		let name = buffer.name.clone();
		let k = self.buffers.insert(buffer);
		self.buffers_by_id.insert(name, k);
		k
	}

	pub fn get(&self, key: BufferKey) -> Option<&Buffer> {
		self.buffers.get(key)
	}

	pub fn get_mut(&mut self, key: BufferKey) -> Option<&mut Buffer> {
		self.buffers.get_mut(key)
	}
	
	pub fn key_of(&self, id: impl Into<String>) -> Option<BufferKey> {
		self.buffers_by_id.get(&id.into()).cloned()
	}
	
	/// Bind unbound and dirty buffers.
	pub(crate) fn update_bindings(&mut self, device: &wgpu::Device, bind_groups: &BindGroupManager) {
		for (_, b) in self.buffers.iter_mut() {
			if b.dirty.load(Ordering::Relaxed) {
				b.rebind(device, bind_groups);
			}
		}
	}

	/// Copy data from queued writes and previous bindings.
	pub fn do_writes(&mut self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
		for (_, b) in self.buffers.iter_mut() {
			b.do_writes(queue, encoder);
		}
	}
}


#[derive(Debug)]
pub struct Buffer {
	pub name: String,
	size: u64,
	
	readable: bool, // Currently unused  
	writable: bool,
	persistent: bool, // Should buffer contents be persisted when rebinding? 
	queued_writes: Vec<(wgpu::BufferAddress, Vec<u8>)>, // This is used when trying to write to a buffer that is not yet bound

	base_usages: wgpu::BufferUsages,
	used_by_materials: RwLock<Vec<(MaterialKey, RenderContextKey, wgpu::BufferUsages)>>,
	// Set when Self::add_usages or Self::remove_usages detects a change in usages
	dirty: AtomicBool, 

	pub(crate) binding: Option<wgpu::Buffer>,
	// If this is Some, then we need to copy the buffer's contents into the current binding
	binding_previous: Option<wgpu::Buffer>,

	// Tracks what bind groups include this buffer 
	// If this is empty, then we can unload the buffer (TODO: that)
	bind_groups: RwLock<HashSet<BindGroupKey>>,
}
impl Buffer {
	pub fn new(
		name: impl Into<String>, 
		size: u64, 
		readable: bool, 
		writable: bool,
		persistent: bool, 
	) -> Self {
		let mut base_usages = wgpu::BufferUsages::empty();
		if writable { base_usages |= wgpu::BufferUsages::COPY_DST; }
		if readable { base_usages |= wgpu::BufferUsages::COPY_SRC; }
		if persistent { base_usages |= wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC; }

		Self {
			name: name.into(),
			size,
			base_usages,
			readable, 
			writable,
			persistent,
			queued_writes: Vec::new(),
			used_by_materials: RwLock::new(Vec::new()),
			dirty: AtomicBool::new(true),
			binding: None,
			binding_previous: None,
			bind_groups: RwLock::new(HashSet::new()),
		}
	}

	pub fn new_init(
		name: impl Into<String>, 
		data: &[u8], 
		readable: bool, 
		writable: bool,
		persistent: bool,
	) -> Self {
		let mut buffer = Self::new(
			name, 
			data.len() as u64, 
			readable, 
			writable, 
			persistent,
		).with_usages(wgpu::BufferUsages::COPY_DST);
		buffer.queued_writes.push((0, data.to_vec()));
		buffer
	}

	/// Adds these usages to the buffer's base usages. 
	/// This is usually not necessary but useful to have as an option.
	/// Because it takes self, it can only be done outside of the [BufferManager].
	pub fn with_usages(mut self, usages: wgpu::BufferUsages) -> Self {
		self.base_usages |= usages;
		self
	}

	// TODO: this will not work becuase wgpu::BufferSlice has the liftime of this buffer reference 
	// Perhaps we can create a new buffer, queue a copy to it, and then return it for mapping? 
	// Can we find a way to make this work for read-write mappings? 
	// pub fn read(&self) -> (wgpu::BufferSlice, crossbeam_channel::Receiver<Result<(), wgpu::BufferAsyncError>>) {
	// 	assert!(self.readable, "Buffer '{}' is not readable!", self.name);
	// 	if let Some(binding) = self.binding.as_ref() {
	// 		let slice = binding.slice(..);
	// 		let (sender, receiver) = crossbeam_channel::unbounded();
	// 		slice.map_async(wgpu::MapMode::Read, move |v: Result<(), wgpu::BufferAsyncError>| sender.send(v).unwrap());
	// 		(slice, receiver)
	// 	} else {
	// 		todo!()
	// 	}
	// }

	pub fn write(&mut self, queue: &wgpu::Queue, offset: wgpu::BufferAddress, data: &[u8]) {
		assert!(self.writable, "Buffer '{}' is not writable!", self.name);
		if let Some(buffer) = self.binding.as_ref() {
			queue.write_buffer(buffer, offset, data);
		} else {
			warn!("Tried to write to unbound buffer '{}', adding to write queue at index {}", self.name, self.queued_writes.len());
			self.queued_writes.push((offset, data.to_vec()));
		}
	}

	pub fn write_queued(&mut self, offset: wgpu::BufferAddress, data: &[u8]) {
		assert!(self.writable, "Buffer '{}' is not writable!", self.name);
		warn!("Buffer '{}' adds to write queue at index {} with length {}", self.name, self.queued_writes.len(), data.len());
		self.queued_writes.push((offset, data.to_vec()));
	}

	pub fn usages(&self) -> wgpu::BufferUsages {
		self.used_by_materials.read()
			.iter()
			.map(|(_, _, u)| u)
			.copied()
			.fold(self.base_usages, |a, u| a | u)
	}

	pub(crate) fn add_material_usage(&self, material: MaterialKey, context: RenderContextKey, usages: wgpu::BufferUsages) {
		let current_usages = self.usages();
		if current_usages | usages != current_usages {
			trace!("Buffer '{}' is made invalid by an added material", self.name);
			self.dirty.store(true, Ordering::Relaxed);
		}
		self.used_by_materials.write().push((material, context, usages));
	}

	pub(crate) fn remove_material_usage(&self, material: MaterialKey, context: RenderContextKey) {
		let old_usages = self.usages();
		self.used_by_materials.write().retain(|&(m, c, _)| (m, c) != (material, context));
		let new_usages = self.usages();
		if old_usages != new_usages {
			trace!("Buffer '{}' is made invalid by a removed material", self.name);
			self.dirty.store(true, Ordering::Relaxed);
		}
	}

	pub(crate) fn add_dependent_bind_group(&self, bind_group: BindGroupKey) {
		self.bind_groups.write().insert(bind_group);
	}

	pub(crate) fn remove_dependent_bind_group(&self, bind_group: BindGroupKey) {
		self.bind_groups.write().remove(&bind_group);
	}

	pub(crate) fn do_writes(&mut self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
		if let Some(binding) = self.binding.as_ref() {
			if let Some(b) = self.binding_previous.take() {
				warn!("Copy buffer '{}' previous binding to new", self.name);
				encoder.copy_buffer_to_buffer(&b, 0, binding, 0, b.size());
			}

			for (i, (offset, data)) in self.queued_writes.drain(..).enumerate() {
				debug!("Writing queued write {i} for buffer '{}'", self.name);
				queue.write_buffer(binding, offset, data.as_slice());
			}
		} else {
			warn!("Skipping update for buffer '{}' because binding does not exist", self.name);
		}
	}

	pub(crate) fn rebind(
		&mut self, 
		device: &wgpu::Device, 
		bind_groups: &BindGroupManager,
	) {
		if self.used_by_materials.read().is_empty() {
			return;
		}

		let usages = self.usages();
		debug!("Buffer '{}' binds with usages {:?}", self.name, usages);

		if self.binding.is_some() {
			trace!("Rebind marks {} dependent bind groups as invalid", self.bind_groups.read().len());
			self.bind_groups.read().iter().for_each(|&key| {
				if let Some(e) = bind_groups.get(key) {
					e.mark_dirty();
				} else {
					warn!("Tried to mark a nonexistent bind group as dirty");
				}
			});
		}

		let new_binding = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some(&*self.name), 
			size: self.size, 
			usage: usages,
			mapped_at_creation: false,
		});

		let old_binding = self.binding.replace(new_binding);
		if self.persistent {
			self.binding_previous = old_binding;
			if self.binding_previous.is_some() {
				warn!("Storing old binding for copy");
			}
		}

		self.dirty.store(false, Ordering::Relaxed);
	}
}


// #[derive(thiserror::Error, Debug)]
// pub enum BufferError {
// 	#[error("buffer not bound")]
// 	BufferUnbound,
// 	#[error("index out of bounds")]
// 	OutOfBounds,
// 	#[error("index out of bounds")]
// 	WriteInactiveSlab,
// }
