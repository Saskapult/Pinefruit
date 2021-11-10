
use std::collections::HashMap;
use crate::model::{
	texture::*,
	mesh::*,
	material::*,
};
use anyhow::*;





/*
LoaderBot swaps textures on the gpu based on their usage
It helps with memory efficiency

How many textures should be stored at a given time?
Larger than texture for each chunk rendered because mobs too
*/
const NRAMTEXTURES: u32 = 2000;
const NGPUTEXTURES: u32 = 20;	// The number of textures to be stored on the GPU at a given time
pub struct TextureLoader {
	texture_entries: HashMap<String, TextureEntry>,
	textures_DISK: HashMap<String, TextureDISK>,
	textures_RAM: HashMap<String, TextureRAM>,
	textures_GPU: HashMap<String, TextureGPU>,		// Texture buffers and the name of the texture currently stored in them
}
impl TextureLoader {
	// Gets a texture buffer
	pub fn get_tetxure(&mut self, name: &String) -> Result<Texture> {
		return match self.get_texture_GPU(name) {
			Some(tgpu) => {
				tgpu.texture
			},
			Err(e) => Err(e),
		}	
	}
	// Looks for a texture in gpu memory, loads if not found
	pub fn get_texture_GPU(&mut self, name: &String) -> Result<TextureGPU> {
		// Look in loaded stuff
		if self.textures_GPU.contains_key(&name) {
			return Ok(self.textures_GPU[&name])
		}
		// Try to load from disk
		return match self.get_tetxure_DISK(name) {
			Some(tram) => {
				// Swap into gpu memory (todo)
				Ok(TextureGPU::from_ram(tram))
			},
			Err(e) => Err(e),
		}
	}

	pub fn get_tetxure_RAM(&mut self, name: &String) -> Result<TextureRAM> {		
		// Look in loaded stuff
		if self.textures_RAM.contains_key(&name) {
			return Ok(self.textures_RAM[&name])
		}
		// Try to load from disk
		return match self.get_tetxure_DISK(name) {
			Some(tdisk) => {
				// Swap into ram (todo)
				Ok(TextureRAM::from_disk(tdisk))
			},
			Err(e) => Err(e),
		}
	}

	pub fn get_tetxure_DISK(&mut self, name: &String) -> Result<TextureDISK> {
		if self.textures_DISK.contains_key(&name) {
			return Ok(self.textures_DISK[&name])
		}
		Err("Texture not on disk")
	}

}











