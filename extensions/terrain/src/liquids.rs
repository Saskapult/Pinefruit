

pub enum LiquidContents {
	Empty, 
	Filled, 
	Pallete4(Vec<bool>, Vec<u8>),
	// Pallete12(Vec<bool>, Vec<u16>),
	// Pallete28(Vec<bool>, Vec<u32>),
}

