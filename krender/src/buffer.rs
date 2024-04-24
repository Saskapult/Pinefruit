//! I've decided that if we ever need to know the contents of a buffer, it will have to be mapped.
//! 
//! A buffer entry is created when a buffer is requested.
//! This allows us to collect buffer usages.
//! When it is written to we create it.
//! I haven't fully thought this through.
//! Rebinding (with new usages) is the same thing?
//! What about the size thingy, how do we know that?
//! 
//! If we try to write to the buffer before it is created, we can just queue that write!
//! 
//! Does `Queue::write_buffer` require the buffer to be `CPY_DST`? 
//! 

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
	
	pub fn key(&self, id: impl Into<String>) -> Option<BufferKey> {
		self.buffers_by_id.get(&id.into()).cloned()
	}

	pub fn add_usages(&self, key: BufferKey, material: MaterialKey, context: RenderContextKey, usages: wgpu::BufferUsages) {
		if let Some(b) = self.buffers.get(key) {
			b.add_usages(material, context, usages);
		} else {
			warn!("Tried to add dependent material to nonexistent buffer");
		}
	}
	pub fn remove_usages(&self, key: BufferKey, material: MaterialKey, context: RenderContextKey) {
		if let Some(b) = self.buffers.get(key) {
			b.remove_usages(material, context);
		} else {
			warn!("Tried to remove dependent material from nonexistent buffer");
		}
	}

	pub fn add_dependent_bind_group(&self, key: BufferKey, bind_group: BindGroupKey) {
		if let Some(b) = self.buffers.get(key) {
			b.add_dependent_bind_group(bind_group);
		} else {
			warn!("Tried to add dependent bind group to nonexistent buffer");
		}
	}
	pub fn remove_dependent_bind_group(&self, key: BufferKey, bind_group: BindGroupKey) {
		if let Some(b) = self.buffers.get(key) {
			b.remove_dependent_bind_group(bind_group);
		} else {
			warn!("Tried to remove dependent bind group from nonexistent buffer");
		}
	}
	
	/// Bind or rebind buffers 
	pub fn update_bindings(&mut self, device: &wgpu::Device, bind_groups: &BindGroupManager) {
		for (_, b) in self.buffers.iter_mut() {
			if b.dirty.load(Ordering::Relaxed) {
				b.rebuild(device, bind_groups);
			}
		}
	}

	pub fn do_queued_writes(&mut self, queue: &wgpu::Queue) {
		info!("Doing queued writes");
		for (_, b) in self.buffers.iter_mut() {
			b.update(queue);
		}
	}
}


#[derive(Debug)]
pub struct Buffer {
	pub name: String,
	pub size: u64,
	
	readable: bool, 
	writable: bool,
	persistent: bool,
	queued_writes: Vec<(wgpu::BufferAddress, Vec<u8>)>, // This is used when trying to write to a buffer that is not yet bound

	// Some iff the buffer is meant to be read from
	staging: Option<Option<(wgpu::Buffer, AtomicBool)>>, // Bool is if mapped
	// Some iff the buffer is meant to be resized
	// previous: Option<Option<wgpu::Buffer>>,

	pub base_usages: wgpu::BufferUsages,
	derived_usages: RwLock<Vec<(MaterialKey, RenderContextKey, wgpu::BufferUsages)>>,

	dirty: AtomicBool, // true iff needs to be rebound
	pub binding: Option<wgpu::Buffer>,
	bind_groups: RwLock<HashSet<BindGroupKey>>,
}
impl Buffer {
	pub fn new(
		name: impl Into<String>, 
		size: u64, 
		readable: bool, 
		writable: bool,
		persistent: bool, // Retain data when rebound
	) -> Self {
		let mut base_usages = wgpu::BufferUsages::empty();
		if writable { base_usages |= wgpu::BufferUsages::COPY_DST; }
		if readable { base_usages |= wgpu::BufferUsages::COPY_SRC; }
		if persistent { base_usages |= wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_DST; }

		Self {
			name: name.into(),
			size,
			base_usages,
			readable, 
			writable,
			persistent,
			queued_writes: Vec::new(),
			staging: readable.then(|| None),
			derived_usages: RwLock::new(Vec::new()),
			dirty: AtomicBool::new(true),
			binding: None,
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

	pub fn read(&self) -> (wgpu::BufferSlice, crossbeam_channel::Receiver<Result<(), wgpu::BufferAsyncError>>) {
		assert!(self.readable, "Buffer '{}' is not readable!", self.name);
		if let Some(binding) = self.binding.as_ref() {
			let slice = binding.slice(..);
			let (sender, receiver) = crossbeam_channel::unbounded();
			slice.map_async(wgpu::MapMode::Read, move |v: Result<(), wgpu::BufferAsyncError>| sender.send(v).unwrap());
			(slice, receiver)
		} else {
			todo!()
		}
	}

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
		self.derived_usages.read()
			.iter()
			.map(|(_, _, u)| u)
			.copied()
			.fold(self.base_usages, |a, u| a | u)
	}

	fn add_usages(&self, material: MaterialKey, context: RenderContextKey, usages: wgpu::BufferUsages) {
		let current_usages = self.usages();
		if current_usages | usages != current_usages {
			trace!("Buffer '{}' is made invalid by an added material", self.name);
			self.dirty.store(true, Ordering::Relaxed);
		}
		self.derived_usages.write().push((material, context, usages));
	}

	fn remove_usages(&self, material: MaterialKey, context: RenderContextKey) {
		let old_usages = self.usages();
		self.derived_usages.write().retain(|&(m, c, _)| (m, c) != (material, context));
		let new_usages = self.usages();
		if old_usages != new_usages {
			trace!("Buffer '{}' is made invalid by a removed material", self.name);
			self.dirty.store(true, Ordering::Relaxed);
		}
	}

	fn add_dependent_bind_group(&self, bind_group: BindGroupKey) {
		self.bind_groups.write().insert(bind_group);
	}

	fn remove_dependent_bind_group(&self, bind_group: BindGroupKey) {
		self.bind_groups.write().remove(&bind_group);
	}

	pub fn update(&mut self, queue: &wgpu::Queue) {
		if let Some(Some((b, mapped))) = self.staging.as_ref() {
			if mapped.swap(false, Ordering::Relaxed) {
				debug!("Unmapping '{}' staging buffer", self.name);
				b.unmap();
			}
		}
		if let Some(binding) = self.binding.as_ref() {
			for (i, (offset, data)) in self.queued_writes.drain(..).enumerate() {
				debug!("Writing queued write {i} for buffer '{}'", self.name);
				queue.write_buffer(binding, offset, data.as_slice());
			}
		} else {
			warn!("Skipping queued writes for buffer '{}' because binding does not exist", self.name);
		}
		
		// If settings differ then rebind idk
		// Pass signal to rebuild dependents if you do that
	}

	// Wipes buffer contents.
	// We could keep an old buffer and use copy_buffer_to_buffer to move the stuff over.
	// Then we can drop the old one.
	// We will need an encoder for that though. 
	pub fn rebuild(
		&mut self, 
		device: &wgpu::Device, 
		bind_groups: &BindGroupManager,
	) {
		if self.derived_usages.read().is_empty() {
			return;
		}

		let mut usages = self.usages();
		debug!("Buffer '{}' binds with usages {:?}", self.name, usages);

		// If we have a staging buffer then add CPY_SRC to usages and create staging buffer
		if let Some(staging) = self.staging.as_mut() {
			let mapped_at_creation = true;
			let _ = staging.insert((device.create_buffer(&wgpu::BufferDescriptor {
				label: Some(&*format!("{} staging buffer", self.name)), 
				size: self.size, 
				usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
				mapped_at_creation,
			}), AtomicBool::new(mapped_at_creation)));

			usages = usages | wgpu::BufferUsages::COPY_SRC
		}
		

		if self.binding.is_some() {
			trace!("Marking {} dependent bind groups as invalid", self.bind_groups.read().len());
			self.bind_groups.read().iter().for_each(|&key| bind_groups.mark_dirty(key));
		}

		let new_binding = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some(&*self.name), 
			size: self.size, 
			usage: usages,
			mapped_at_creation: false,
		});

		if self.persistent {
			if let Some(_) = self.binding.as_ref() {
				warn!("Copy buffer contents to new binding");
				println!("old usages {:?}", self.binding.as_ref().unwrap().usage());
				todo!("Oh no that needs a command encoder!");
			}
		}

		self.dirty.store(false, Ordering::Relaxed);
		self.binding = Some(new_binding);
	}
}


#[derive(thiserror::Error, Debug)]
pub enum BufferError {
	#[error("buffer not bound")]
	BufferUnbound,
	#[error("index out of bounds")]
	OutOfBounds,
	#[error("index out of bounds")]
	WriteInactiveSlab,
}
