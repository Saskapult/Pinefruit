


pub struct VoxelVolumeHeader {
	pub attributes: Vec<VoxelAttribute>,
	pub bits_per_voxel: u32,
}
impl VoxelVolumeHeader {
	pub fn new(attributes: Vec<VoxelAttribute>) -> Self {
		let bits_per_voxel = attributes.iter()
			.fold(0, |ac, at| ac + at.bits_per_element);
		Self { 
			attributes, 
			bits_per_voxel,
		}
	}
	pub fn add_attribute(&mut self, attribute: VoxelAttribute) {
		self.bits_per_voxel += attribute.bits_per_element;
		self.attributes.push(attribute);
	}
}



pub struct VoxelAttribute {
	pub name: String,
	pub bits_per_element: u32,
}
impl VoxelAttribute {
	// Offset in bits
	pub fn write_me(&self, value: &[u8], destination: &mut [u8], offset: u32) {
		for (i, dest_bit_offset) in (offset..(offset+self.bits_per_element)).enumerate() {
			let dest_byte_idx = dest_bit_offset / 8;
			let local_bit_offset = dest_bit_offset % 8;

			let src_byte = value[(i / 8) as usize];
			let dest_byte = &mut destination[dest_byte_idx as usize];

			let bit_mask = 1 << local_bit_offset;
			*dest_byte &= bit_mask ^ 1; // Set bit to 0
			*dest_byte |= bit_mask & src_byte; // Set bit to src bit
		}
	}
}



pub struct ArrayVoxelVolume {
	pub size: [u32; 3],
	pub header: VoxelVolumeHeader,
	pub contents: Vec<u8>,
}
impl ArrayVoxelVolume {
	pub fn new(header: VoxelVolumeHeader, size: [u32; 3]) -> Self {
		let bits_per_voxel = header.attributes.iter()
			.fold(0, |ac, at| ac + at.bits_per_element);
		// Not as efficient but certainly easier
		let bytes_per_voxel = bits_per_voxel.div_ceil(8);
		let bytes_needed = size.iter().fold(1, |a, v| a * v) * bytes_per_voxel;

		Self {
			size, 
			header, 
			contents: vec![0; bytes_needed as usize],
		}
	}
}


