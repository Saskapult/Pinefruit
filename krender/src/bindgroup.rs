use std::{collections::HashMap, sync::atomic::{AtomicBool, Ordering}};
use arrayvec::ArrayVec;
use parking_lot::RwLock;
use slotmap::SlotMap;
use crate::{buffer::BufferManager, texture::TextureManager, BindGroupKey, BindGroupLayoutKey, BufferKey, MaterialKey, RenderContextKey, SamplerKey, TextureKey};



#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum BindGroupEntryContentDescriptor {
	Buffer(BufferKey),
	Buffers(Vec<BufferKey>),
	Texture(TextureKey),
	Textures(Vec<TextureKey>),
	Sampler(SamplerKey),
	// Samplers(Vec<idk>),
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
				slot.as_ref().map(|slot| (i, slot))
			})
			.map(|(i, slot)| {
				info!("Looking for {slot:?}");
				let resource = match slot {
					BindGroupEntryContentDescriptor::Buffer(key) => buffers.get(*key).expect("no buffer").binding.as_ref().expect("No binding").as_entire_binding(),
					BindGroupEntryContentDescriptor::Texture(key) => wgpu::BindingResource::TextureView({
						let t = textures.get(*key).unwrap().view().unwrap();
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
pub struct BindGroupEntry {
	pub descriptor: BindGroupDescriptor,
	pub binding: Option<wgpu::BindGroup>,
	// A list of all materials that use this bind group
	// This could be a counter, but a list is better for debugging 
	// Should maybe be a mutex instead
	used_by_materials: RwLock<Vec<(MaterialKey, RenderContextKey)>>, 
	// When a resource changes, it sets the dirty flag of all bind groups that have registered with it
	pub dirty: AtomicBool,
}
impl BindGroupEntry {
	pub(crate) fn add_material_usage(&self, material: MaterialKey, context: RenderContextKey) {
		let mut materials = self.used_by_materials.write();
		if !materials.contains(&(material, context)) {
			materials.push((material, context));
		}
	}

	pub(crate) fn remove_material_usage(&self, material: MaterialKey, context: RenderContextKey) {
		self.used_by_materials.write().retain(|&(m, c)| (m, c) != (material, context));
	}

	pub fn mark_dirty(&self) {
		self.dirty.store(true, Ordering::Relaxed);
	}
}
impl From<([Option<BindGroupEntryContentDescriptor>; 4], BindGroupLayoutKey)> for BindGroupEntry {
	fn from((slots, layout_key): ([Option<BindGroupEntryContentDescriptor>; 4], BindGroupLayoutKey)) -> Self {
		Self {
			descriptor: BindGroupDescriptor {slots, layout_key}, binding: None, used_by_materials: RwLock::new(Vec::new()), dirty: AtomicBool::new(true),
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
	pub fn get_or_create_layout(&mut self, layout_entries: ArrayVec::<wgpu::BindGroupLayoutEntry, 4>) -> BindGroupLayoutKey {
		if let Some(&g) = self.bind_group_layouts_by_content.get(&layout_entries) {
			g
		} else {
			let k = self.bind_group_layouts.insert(layout_entries.clone().into());
			self.bind_group_layouts_by_content.insert(layout_entries, k);
			k
		}
	}

	/// Gets or creates a bind group. 
	/// Does **not** flag the bind group as being used by any materials! 
	pub fn get_or_create_bind_group(
		&mut self, 
		binding_config: [Option<BindGroupEntryContentDescriptor>; 4], 
		layout_key: BindGroupLayoutKey,
		textures: &TextureManager, // Needed to add self as a dependent
		buffers: &BufferManager, // Needed to add self as a dependent
	) -> BindGroupKey {
		let e = (binding_config, layout_key);
		let key = if let Some(&key) = self.bind_group_by_slots.get(&e) {
			key
		} else {
			let binding_config = e.0.clone();
			
			let bg_key = self.bind_groups.insert(e.clone().into());
			self.bind_group_by_slots.insert(e, bg_key);

			for bgecd in binding_config.iter().filter_map(|e| e.as_ref()) {
				let buffer_add_dependent_bind_group = |k| {
					if let Some(b) = buffers.get(k) {
						b.add_dependent_bind_group(bg_key);
					} else {
						warn!("Tried to remove dependent bind group from nonexistent buffer");
					}
				};
				let texture_add_dependent_bind_group = |k| {
					if let Some(t) = textures.get(k) {
						t.add_dependent_bind_group(bg_key);
					} else {
						warn!("Tried to remove dependent bind group from nonexistent texture");
					}
				};
				match bgecd {
					&BindGroupEntryContentDescriptor::Buffer(b) => buffer_add_dependent_bind_group(b),
					BindGroupEntryContentDescriptor::Buffers(b) => b.iter().for_each(|&b| buffer_add_dependent_bind_group(b)),
					&BindGroupEntryContentDescriptor::Texture(t) => texture_add_dependent_bind_group(t),
					BindGroupEntryContentDescriptor::Textures(t) => t.iter().for_each(|&t| texture_add_dependent_bind_group(t)),
					BindGroupEntryContentDescriptor::Sampler(_) => {},
				}
			}

			bg_key
		};

		key
	}

	pub fn get_or_create_sampler(&mut self, descriptor: SamplerDescriptor) -> SamplerKey {
		if let Some(&(_, k)) = self.samplers_by_descriptor.iter().find(|(d, _)| descriptor.eq(d)) {
			k
		} else {
			let k = self.samplers.insert((descriptor, None));
			self.samplers_by_descriptor.push((descriptor, k));
			k
		}
	}

	pub fn get(&self, key: BindGroupKey) -> Option<&BindGroupEntry> {
		self.bind_groups.get(key)
	}

	pub fn update_bindings(
		&mut self,
		device: &wgpu::Device,
		textures: &TextureManager,
		buffers: &BufferManager,
	) {
		// Create samplers 
		for (d, b) in self.samplers.values_mut() {
			if b.is_none() {
				let desc: wgpu::SamplerDescriptor = (*d).into();
				*b = Some(device.create_sampler(&desc));
			}
		}

		// Remove those without references
		self.bind_groups.retain(|k, v| {
			if v.used_by_materials.read().len() == 0 {
				debug!("Removing bind group {:?} (used by 0 materials)", k);
				// assert!(!v.dirty.load(Ordering::Relaxed), "Bind group is dirty but is not used by any material!");
				false
			} else {
				true
			}
		});

		// Rebuild those with dirty flag
		for (key, entry) in self.bind_groups.iter_mut().filter(|(_, e)| e.dirty.load(Ordering::Relaxed)) {
			debug!("Creating binding for bind group {:?}", key);

			// Buffer and texture data must be extracted before creating any wgpu::BindingResource::BufferArray
			// because otherwise we cannot create a collection that both lives long enough and remains immutable 
			let buffer_array_data = entry.descriptor.slots.iter()
			.filter_map(|slot| slot.as_ref())
			.filter_map(|slot| match slot {
				BindGroupEntryContentDescriptor::Buffers(keys) => Some(keys),
				_ => None,
			}).flat_map(|keys| keys.iter().map(|&key| {
				buffers.get(key).unwrap().binding.as_ref().unwrap().as_entire_buffer_binding()
			})).collect::<Vec<_>>();

			let texture_array_data = entry.descriptor.slots.iter()
			.filter_map(|slot| slot.as_ref())
			.filter_map(|slot| match slot {
				BindGroupEntryContentDescriptor::Textures(keys) => Some(keys),
				_ => None,
			}).flat_map(|keys| keys.iter().map(|&key| {
				textures.get(key).unwrap().view().unwrap()
			})).collect::<Vec<_>>();

			let mut buffer_array_offset = 0;
			let mut texture_array_offset = 0;
			let entries = entry.descriptor.slots.iter().enumerate()
			.filter_map(|(i, slot)| {
				slot.as_ref().map(|slot| (i, slot))
			})
			.map(|(i, slot)| {
				info!("Looking for {slot:?}");
				let resource = match slot {
					BindGroupEntryContentDescriptor::Buffer(key) => {
						let b = buffers.get(*key).expect("no buffer");
						b.binding.as_ref().expect("No binding").as_entire_binding()
					},
					BindGroupEntryContentDescriptor::Buffers(keys) => {
						let br = wgpu::BindingResource::BufferArray(&buffer_array_data[buffer_array_offset..]);
						buffer_array_offset += keys.len();
						br
					},
					BindGroupEntryContentDescriptor::Texture(key) => wgpu::BindingResource::TextureView({
						let t = textures.get(*key).unwrap().view().unwrap();
						t
					}),
					BindGroupEntryContentDescriptor::Textures(keys) => {
						let br = wgpu::BindingResource::TextureViewArray(&texture_array_data[texture_array_offset..]);
						texture_array_offset += keys.len();
						br
					},
					BindGroupEntryContentDescriptor::Sampler(key) => wgpu::BindingResource::Sampler(self.samplers.get(*key).unwrap().1.as_ref().unwrap()),
				};
				
				wgpu::BindGroupEntry {
					binding: i as u32,
					resource,
				}
			}).collect::<Vec<_>>();
			
			entry.dirty.store(false, Ordering::Relaxed);
			entry.binding = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
				label: None,
				layout: self.bind_group_layouts.get(entry.descriptor.layout_key).unwrap().layout.as_ref().unwrap(),
				entries: entries.as_slice(),
			}));
		}
	}
}
