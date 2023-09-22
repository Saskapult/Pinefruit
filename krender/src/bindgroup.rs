use std::{collections::HashMap, sync::atomic::{AtomicBool, Ordering}};

use arrayvec::ArrayVec;
use slotmap::SlotMap;

use crate::{BindGroupKey, BindGroupLayoutKey, BufferKey, TextureKey, texture::TextureManager, buffer::BufferManager, SamplerKey};



#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum BindGroupEntryContentDescriptor {
	Buffer(BufferKey),
	Buffers(Vec<BufferKey>),
	Texture(TextureKey),
	Textures(Vec<TextureKey>),
	Sampler(SamplerKey),
	// Samplers(Vec<idk>),
	// Don't need to worry about storage textures because that is built in to the layout, not this
}


#[derive(Debug)]
pub struct BindGroupDescriptor {
	pub slots: [Option<BindGroupEntryContentDescriptor>; 4],
	pub layout_key: BindGroupLayoutKey,
}
impl BindGroupDescriptor {
	pub fn entries<'a>(
		&self, 
		textures: &'a TextureManager,
		buffers: &'a BufferManager,
		samplers: &'a SlotMap<SamplerKey, (SamplerDescriptor, Option<wgpu::Sampler>)>
	) -> Vec<wgpu::BindGroupEntry<'a>> {
		// For each slot that has content, pull that content
		// This doesn't work for array entries because of the borrow checker
		// I am not sure how to get around that
		self.slots.iter().enumerate()
			.filter_map(|(i, slot)| {
				if let Some(slot) = slot {
					Some((i, slot))
				} else {
					None
				}
			})
			.map(|(i, slot)| {
				info!("Looking for {slot:?}");
				let resource = match slot {
					BindGroupEntryContentDescriptor::Buffer(key) => buffers.get(*key).unwrap().binding.as_ref().unwrap().as_entire_binding(),
					BindGroupEntryContentDescriptor::Texture(key) => wgpu::BindingResource::TextureView({
						let t= &textures.get(*key).unwrap().binding().unwrap().view;
						info!("{}", textures.get(*key).unwrap().label);
						t
					}),
					BindGroupEntryContentDescriptor::Sampler(key) => wgpu::BindingResource::Sampler(samplers.get(*key).unwrap().1.as_ref().unwrap()),
					// BindGroupResourceDescriptor::Buffers(keys) => {
					// 	// let st = buffer_array_entries.len();
					// 	// buffer_array_entries.extend(keys.iter().map(|&key| {
					// 	// 	buffers.get(key).unwrap().buffer().as_entire_buffer_binding()

					// 	// }));
					// 	// let en = buffer_array_entries.len();
					// 	// wgpu::BindingResource::BufferArray(&buffer_array_entries[st..en])
					// },
					_ => todo!(),
				};
				
				wgpu::BindGroupEntry {
					binding: i as u32,
					resource,
				}
			}).collect::<Vec<_>>()
	}

	// pub fn contains(&self, other: &Self) -> bool {
	// 	self.slots.iter().zip(other.slots.iter()).all(|(s, o)| {
	// 		if let Some(o) = o {
	// 			if let Some(s) = s {
	// 				// This comparison could be slow because it involves vectors
	// 				// If you ever need to improve performance then maybe consider just returning false if that happens
	// 				o.eq(s)
	// 			} else {
	// 				// if other is some and self is none then no 
	// 				false	
	// 			}
	// 		} else {
	// 			// if other is some then yes 
	// 			true
	// 		}
	// 	})
	// }
}


#[derive(Debug)]
struct BindGroupLayoutEntry {
	pub layout_entries: ArrayVec::<wgpu::BindGroupLayoutEntry, 4>,
	pub layout: Option<wgpu::BindGroupLayout>,
}
impl BindGroupLayoutEntry {
	pub fn build(&mut self, device: &wgpu::Device) {
		// Could store dependent shaders for more descriptiveness
		if self.layout.is_none() {
			info!("Creating bind group layout with entries {:?}", self.layout_entries);
		} else {
			warn!("Re-creating bind group layout with entries {:?}, that's probably not intended", self.layout_entries);
		}
		self.layout = Some(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: None,
			entries: &self.layout_entries,
		}));
	}
}
impl From<ArrayVec::<wgpu::BindGroupLayoutEntry, 4>> for BindGroupLayoutEntry {
	fn from(layout_entries: ArrayVec::<wgpu::BindGroupLayoutEntry, 4>) -> Self {
		Self {
			layout_entries, layout: None,
		}
	}
}


#[derive(Debug)]
struct BindGroupEntry {
	pub descriptor: BindGroupDescriptor,
	pub binding: Option<wgpu::BindGroup>,
	// Incremented for every material using this bind group
	pub references: u32, 
	pub dirty: AtomicBool,
}
impl From<([Option<BindGroupEntryContentDescriptor>; 4], BindGroupLayoutKey)> for BindGroupEntry {
	fn from((slots, layout_key): ([Option<BindGroupEntryContentDescriptor>; 4], BindGroupLayoutKey)) -> Self {
		Self {
			descriptor: BindGroupDescriptor {slots, layout_key}, binding: None, references: 1, dirty: AtomicBool::new(true),
		}
	}
}


#[derive(thiserror::Error, Debug)]
pub enum BindGroupError {
	#[error("Invalid key!")]
	InvalidKeyError,
	#[error("Layout not built!")]
	LayoutUnbuilt,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerDescriptor {
	pub address: wgpu::AddressMode,
	pub mag_filter: wgpu::FilterMode,
	pub min_filter: wgpu::FilterMode,
	pub mipmap_filter: wgpu::FilterMode,
	pub lod_min_clamp: f32,
	pub lod_max_clamp: f32,
}
impl Into<wgpu::SamplerDescriptor<'static>> for SamplerDescriptor {
	fn into(self) -> wgpu::SamplerDescriptor<'static> {
		wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: self.address,
			address_mode_v: self.address,
			address_mode_w: self.address,
			mag_filter: self.mag_filter,
			min_filter: self.min_filter,
			mipmap_filter: self.mipmap_filter,
			lod_min_clamp: self.lod_min_clamp,
			lod_max_clamp: self.lod_max_clamp,
			..Default::default()
		}
	}
}


#[derive(Debug)]
pub struct BindGroupManager {
	bind_group_layouts: SlotMap<BindGroupLayoutKey, BindGroupLayoutEntry>,
	bind_group_layouts_by_content: HashMap<ArrayVec::<wgpu::BindGroupLayoutEntry, 4>, BindGroupLayoutKey>,

	bind_groups: SlotMap<BindGroupKey, BindGroupEntry>,
	bind_group_by_slots: HashMap<([Option<BindGroupEntryContentDescriptor>; 4], BindGroupLayoutKey), BindGroupKey>,

	samplers: SlotMap<SamplerKey, (SamplerDescriptor, Option<wgpu::Sampler>)>,
	samplers_by_descriptor: Vec<(SamplerDescriptor, SamplerKey)>,
}
impl BindGroupManager {
	pub fn new() -> Self {
		Self {
			bind_group_layouts: SlotMap::with_key(),
			bind_group_layouts_by_content: HashMap::new(),

			bind_groups: SlotMap::with_key(),
			bind_group_by_slots: HashMap::new(),

			samplers: SlotMap::with_key(),
			samplers_by_descriptor: Vec::new(),
		}
	}

	pub fn build_layouts(&mut self, device: &wgpu::Device) {
		for (_, v) in self.bind_group_layouts.iter_mut() {
			if v.layout.is_none() {
				v.build(device);
			}
		}
	}

	pub fn layout(&self, key: BindGroupLayoutKey) -> Result<&wgpu::BindGroupLayout, BindGroupError> {
		self.bind_group_layouts.get(key)
			.ok_or(BindGroupError::InvalidKeyError)?
			.layout.as_ref()
			.ok_or(BindGroupError::LayoutUnbuilt)
	}

	/// Gets or creates a layout. 
	pub fn i_need_a_layout(&mut self, layout_entries: ArrayVec::<wgpu::BindGroupLayoutEntry, 4>) -> BindGroupLayoutKey {
		if let Some(&g) = self.bind_group_layouts_by_content.get(&layout_entries) {
			g
		} else {
			let k = self.bind_group_layouts.insert(layout_entries.clone().into());
			self.bind_group_layouts_by_content.insert(layout_entries, k);
			k
		}
	}

	/// Gets or creates a bind group. 
	/// Increments the bind group's reference counter. 
	pub fn i_need_a_bind_group(
		&mut self, 
		binding_config: [Option<BindGroupEntryContentDescriptor>; 4], 
		layout_key: BindGroupLayoutKey,
		textures: &TextureManager, // Needed to add self as a dependent
		buffers: &BufferManager, // Needed to add self as a dependent
	) -> BindGroupKey {
		let e = (binding_config, layout_key);
		let key = if let Some(&g) = self.bind_group_by_slots.get(&e) {
			g
		} else {
			let binding_config = e.0.clone();
			
			let key = self.bind_groups.insert(e.clone().into());
			self.bind_group_by_slots.insert(e, key);

			// Register as dependent
			for bgecd in binding_config.iter().filter_map(|e| e.as_ref()) {
				match bgecd {
					&BindGroupEntryContentDescriptor::Buffer(b) => buffers.add_dependent_bind_group(b, key),
					BindGroupEntryContentDescriptor::Buffers(b) => b.iter().for_each(|&b| buffers.add_dependent_bind_group(b, key)),
					&BindGroupEntryContentDescriptor::Texture(t) => textures.add_dependent_bind_group(t, key),
					BindGroupEntryContentDescriptor::Textures(t) => t.iter().for_each(|&t| textures.add_dependent_bind_group(t, key)),
					BindGroupEntryContentDescriptor::Sampler(_) => {},
				}
			}

			key
		};
		self.increment_counter(key);

		

		key
	}

	pub fn increment_counter(&mut self, key: BindGroupKey) {
		self.bind_groups.get_mut(key).unwrap().references += 1;
	}

	pub fn decrement_counter(&mut self, key: BindGroupKey) {
		let t = self.bind_groups.get_mut(key).unwrap();
		t.references -= 1;
		if t.references == 0 {
			// Maybe send a message to a removal queue?
			todo!("Unload bind group")
		}
	}

	pub fn make_or_fetch_sampler(&mut self, descriptor: SamplerDescriptor) -> SamplerKey {
		if let Some(&(_, k)) = self.samplers_by_descriptor.iter().find(|(d, _)| descriptor.eq(d)) {
			k
		} else {
			let k = self.samplers.insert((descriptor, None));
			self.samplers_by_descriptor.push((descriptor, k));
			k
		}
	}

	pub fn get(&self, key: BindGroupKey) -> Option<&wgpu::BindGroup> {
		self.bind_groups.get(key).and_then(|e| e.binding.as_ref())
	}

	pub fn mark_dirty(&self, key: BindGroupKey) {
		if let Some(e) = self.bind_groups.get(key) {
			e.dirty.store(true, Ordering::Relaxed);
		} else {
			warn!("Tried to mark a nonexistent bind group as dirty");
		}
	}

	pub fn update_bindings(
		&mut self,
		device: &wgpu::Device,
		textures: &TextureManager,
		buffers: &BufferManager,
	) {
		// Do sampler stuff
		for (d, b) in self.samplers.values_mut() {
			if b.is_none() {
				let desc: wgpu::SamplerDescriptor = (*d).into();
				*b = Some(device.create_sampler(&desc));
			}
		}

		// Remove those without references
		self.bind_groups.retain(|_, v| v.references != 0);

		// Rebuild those with dirty flag
		self.bind_groups.iter_mut()
			.filter(|(_, e)| e.dirty.load(Ordering::Relaxed))
			.for_each(|(k, e)| {
				info!("Creating binding for bind group {:?}", k);
				trace!("{:#?}", e.descriptor.slots.iter().filter(|o| o.is_some()).count());
				let entries = e.descriptor.entries(textures, buffers, &self.samplers);

				e.dirty.store(false, Ordering::Relaxed);
				e.binding = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
					label: None,
					layout: self.bind_group_layouts.get(e.descriptor.layout_key).unwrap().layout.as_ref().unwrap(),
					entries: entries.as_slice(),
				}));
			});
	}
}
