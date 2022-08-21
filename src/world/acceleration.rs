use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use crate::{world::*, octree::Octree};
use serde::Serialize;
use std::{ops::Range, collections::HashMap};




/// Fits things into a buffer
#[derive(Debug)]
pub struct SlabBuffer {
	slab_size: usize,
	slabs: Vec<bool>,
	pub buffer: wgpu::Buffer,
}
impl SlabBuffer {
	const ENFORCE_ALIGN_FOUR: bool = true;

	pub fn new(device: &wgpu::Device, slab_size: usize, slab_count: usize, usages: wgpu::BufferUsages) -> Self {
		info!("Creating slab buffer with {slab_count} slabs of size {slab_size} ({} bytes)", slab_count * slab_size);

		if slab_size % 4 != 0 {
			if Self::ENFORCE_ALIGN_FOUR {
				let message = "Slab size is not a multiple of 4, which is not allowed with current settings";
				error!("{message}");
				panic!("{message}");
			} else {
				warn!("buffer start offsets will not be 4 multiples, be sure to account for this in shaders");
			}
		}
		

		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: None,
			size: slab_size as u64 * slab_count as u64,
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE | usages,
			mapped_at_creation: false,
		});
		Self {
			slab_size,
			slabs: vec![false; slab_count],
			buffer,
		}
	}

	/// Storage space in the buffer.
	pub fn size(&self) -> usize {
		self.slab_size * self.slabs.len()
	}

	/// Remaining size in bytes.
	/// Does NOT imply that things that are more than slab size can be fit into this space.
	pub fn remaining_size(&self) -> usize {
		self.slabs.iter()
			.map(|&filled| if filled {0} else {self.slab_size})
			.fold(0, |a, v| a + v)
	}

	/// Fraction of storage space used.
	pub fn capacity_frac(&self) -> f32 {
		1.0 - self.remaining_size() as f32 / self.size() as f32
	}

	/// Returns start and end byte offsets.
	/// Item must be repr(C) to not mess stuff up in shader.
	pub fn insert<T: Serialize>(&mut self, queue: &wgpu::Queue, item: &T) -> [usize; 2] {
		let data = bincode::serialize(item).unwrap();
		self.insert_direct(queue, &data[..])
	}

	pub fn insert_direct(&mut self, queue: &wgpu::Queue, data: &[u8]) -> [usize; 2] {
		let length = data.len();
		let slabs_needed = length.div_ceil(self.slab_size);
		let st = self.select_first_fit(slabs_needed)
			.expect("No allocation fit!");
		let en = st + slabs_needed;
		queue.write_buffer(&self.buffer, st as u64 * self.slab_size as u64, data);
		for i in st..en {
			self.slabs[i] = true;
		}
		[st*self.slab_size, st*self.slab_size + length]
	}

	fn select_first_fit(&self, slabs_needed: usize) -> Option<usize> {
		let mut st = 0;
		let mut free_slabs = 0;
		for (i, slab) in self.slabs.iter().enumerate() {
			if !slab { // Slab free
				if free_slabs == slabs_needed {
					return Some(st)
				}
				free_slabs += 1;
			} else {
				st = i+1;
				free_slabs = 0;
			}
		}
		
		None
	}

	// Untested
	fn select_best_fit(&self, slabs_needed: usize) -> Option<usize> {
		let mut free_ranges = Vec::new();

		let mut st = 0;
		let mut free_slabs = 0;
		for (i, slab) in self.slabs.iter().enumerate() {
			if !slab {
				free_slabs += 1;
			} else {
				free_ranges.push((st, free_slabs));
				st = i+1;
				free_slabs = 0;
			}
		}

		let smallest = free_ranges.iter()
			.filter(|&&(_, l)| l >= slabs_needed)
			.reduce(|accum, item| {
				if item.1 <= accum.1 {
					item
				} else {
					accum
				}
			});

		smallest.and_then(|&(st, _)| Some(st))
	}

	pub fn remove(&mut self, range: Range<usize>) {
		let bytes_st = range.start;
		let bytes_en = range.end;
		let slab_st = bytes_st.div_floor(self.slab_size);
		let slab_en = bytes_en.div_ceil(self.slab_size);
		// println!("Removing from bytes {bytes_st} to {bytes_en} affects slabs {slab_st} to {slab_en}");

		for i in slab_st..slab_en {
			self.slabs[i] = false;
		}
	}
}



#[repr(C)]
#[derive(Debug, Pod, Zeroable, Copy, Clone)]
pub struct ChunkAcceleratorInfo {
	pub centre: [i32; 4], // One extra becuase I'm terrified of the number 3
	pub edge_length: u32, // In chunks, should be odd
	pub chunk_size: u32,
}



#[derive(Debug)]
pub struct ChunkAccelerator {
	pub info: ChunkAcceleratorInfo,
	pub info_uniform: wgpu::Buffer, 
	pub data_buffer: wgpu::Buffer, // Start byte+1 unless none
}
impl ChunkAccelerator {
	pub fn new(device: &wgpu::Device, edge_length: u32, chunk_size: u32) -> Self {

		let info = ChunkAcceleratorInfo {
			centre: [0,0,0,0],
			edge_length,
			chunk_size,
		};

		Self {
			info,
			info_uniform: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("chunk acceleration uniform buffer"),
				contents: bytemuck::bytes_of(&info),
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
			}),
			data_buffer: device.create_buffer(&wgpu::BufferDescriptor {
				label: Some("chunk acceleration data buffer"),
				size: edge_length.pow(3) as u64 * std::mem::size_of::<u32>() as u64,
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
				mapped_at_creation: false,
			}),
		}
	}

	pub fn rebuild(
		&mut self, 
		queue: &wgpu::Queue, 
		centre: [i32; 3], 
		// edge_length: u32, 
		// chunk_size: u32,
		chunks: &HashMap<[i32; 3], (usize, usize)>,
	) {
		let mut data = vec![0; self.info.edge_length.pow(3) as usize];

		let el = self.info.edge_length as usize;
		let el2 = el.pow(2);
		let hel = self.info.edge_length as i32 / 2;
		
		for x in 0..el {
			let cx = x as i32 - hel + centre[0];
			for y in 0..el {
				let cy = y as i32 - hel + centre[1];
				for z in 0..el {
					let cz = z as i32 - hel + centre[2];
					let idx = match chunks.get(&[cx, cy, cz]) {
						Some(&(st, _)) => st as u32 + 1,
						None => 0,
					};
					data[x * el2 + y * el + z] = idx;
				}
			}
		}

		queue.write_buffer(&self.data_buffer, 0, bytemuck::cast_slice(&data[..]));

		self.info.centre = [centre[0], centre[1], centre[2], 0];
		// self.info.edge_length = edge_length;
		// self.info.chunk_size = chunk_size;
		queue.write_buffer(&self.info_uniform, 0, bytemuck::bytes_of(&self.info));
	}
}



#[derive(Debug)]
pub struct TracingChunkManager {
	pub storage: SlabBuffer,
	pub accelerator: ChunkAccelerator,
	pub chunks: HashMap<[i32; 3], (usize, usize)>,
}
impl TracingChunkManager {
	// 1GB is 1073741824B
	// 1MB is 1048576B
	const BUFFER_SIZE: usize = 1048576 * 64;
	const SLAB_SIZE: usize = 256; // 64 uints

	pub fn new(device: &wgpu::Device) -> Self {
		let slab_count = Self::BUFFER_SIZE.div_ceil(Self::SLAB_SIZE);

		assert!(Self::BUFFER_SIZE < u32::MAX as usize, "Buffer is too large to be addressed by u32!");
		
		Self {
			storage: SlabBuffer::new(device, Self::SLAB_SIZE, slab_count, wgpu::BufferUsages::STORAGE),
			accelerator: ChunkAccelerator::new(device, 5, 16),
			chunks: HashMap::new(),
		}
	}

	pub fn make_bg(&self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("chunk accelerator bind group"),
			layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: self.accelerator.info_uniform.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: self.accelerator.data_buffer.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: self.storage.buffer.as_entire_binding(),
				},
			],
		})
	}

	pub fn rebuild(
		&mut self, 
		queue: &wgpu::Queue, 
		centre: [i32; 3],
	) {
		self.accelerator.rebuild(queue, centre, &self.chunks);
	}

	pub fn pull_chunks(
		&mut self, 
		queue: &wgpu::Queue, 
		map: &Map,
	) {
		let centre = self.accelerator.info.centre;
		let el = self.accelerator.info.edge_length as usize;
		let hel = el as i32 / 2;
		for x in 0..=el {
			let cx = x as i32 - hel + centre[0];
			for y in 0..=el {
				let cy = y as i32 - hel + centre[1];
				for z in 0..=el {
					let cz = z as i32 - hel + centre[2];

					self.pull_chunk(queue, map, [cx, cy, cz]);
				}
			}
		}
	}

	pub fn pull_chunk(
		&mut self,
		queue: &wgpu::Queue,
		map: &Map,
		chunk_position: [i32; 3],
	) {
		self.accelerator.info.chunk_size = map.chunk_size[0];
		if let Ok(c) = map.chunk(chunk_position) {
			let data = c.contents
				.iter()
				.map(|v| {
					match v {
						&Voxel::Block(i) => i as u32 + 1,
						Voxel::Empty => 0,
					}
				}).collect::<Vec<_>>();
			let [st, en] = self.storage.insert_direct(queue, bytemuck::cast_slice(&data[..]));
			self.chunks.insert(chunk_position, (st, en));
		}
	}

	pub fn insert_chunk(
		&mut self, 
		queue: &wgpu::Queue,
		position: [i32; 3],
		chunk: &Chunk,
	) {
		if let Some((st, en)) = self.chunks.remove(&position) {
			warn!("Replacing chunk {position:?} in accelerator");
			self.storage.remove(st..en);
		}

		let data = chunk.contents
			.iter()
			.map(|v| {
				match v {
					&Voxel::Block(i) => i as u32 + 1,
					Voxel::Empty => 0,
				}
			}).collect::<Vec<_>>();
		let [st, en] = self.storage.insert_direct(queue, bytemuck::cast_slice(&data[..]));
		self.chunks.insert(position, (st, en));
	}

	pub fn insert_octree(
		&mut self, 
		queue: &wgpu::Queue,
		position: [i32; 3],
		octree: &Octree<usize>, // Octree holding block indices
	) {
		if let Some((st, en)) = self.chunks.remove(&position) {
			warn!("Replacing chunk {position:?} in accelerator");
			self.storage.remove(st..en);
		}

		let nodes = octree.nodes.iter()
			.map(|n| AccelOctreeNode {
				octants: n.octants,
				content: n.content,
				leaf_mask: n.leaf_mask as u32,
			})
			.collect::<Vec<_>>();

		let data = bytemuck::cast_slice(&nodes[..]);
		let [st, en] = self.storage.insert_direct(queue, data);
		self.chunks.insert(position, (st, en));
	}

	pub fn discard_chunk(
		&mut self,
		chunk_position: [i32; 3],
	) {
		if let Some((st, en)) = self.chunks.remove(&chunk_position) {
			self.storage.remove(st..en);
		}
	}
}


/// Octree node as seen by the shader.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
struct AccelOctreeNode {
	octants: [u32; 8],
	content: u32,
	leaf_mask: u32, // Should be u8 but I'm paranoid
}
