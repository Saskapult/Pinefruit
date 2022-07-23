use std::{collections::HashMap, time::{Instant, Duration}};
use winit::{
	event::*,
};

/*
on mouse moved apply to all windows with mouse inside


	GLOBAL
mouse movement
mouse wheel

	PER-WINDOW
keys
cursor inside
*/


// Holds input data
pub struct InputResource {
	// The press percentages for all keys pressed during a timestep
	// It is possible for a percentage to be greater than 100%
	// This happends if startt is after the earliest queue value
	pub board_keys: HashMap<VirtualKeyCode, Duration>,
	pub board_presscache: Vec<VirtualKeyCode>,
	pub mouse_keys: HashMap<MouseButton, Duration>,
	pub mouse_presscache: Vec<MouseButton>,
	pub mx: f64,
	pub my: f64,
	pub mdx: f64,
	pub mdy: f64,
	pub dscrollx: f32,
	pub dscrolly: f32,
	pub last_read: Instant,
	pub last_updated: Instant,
	// controlmap: HashMap<VirtualKeyCode, (some kind of enum option?)>
}
impl InputResource {
	pub fn new() -> Self {
		Self {
			board_keys: HashMap::new(),
			board_presscache: Vec::new(),
			mouse_keys: HashMap::new(),
			mouse_presscache: Vec::new(),
			mx: 0.0,
			my: 0.0,
			mdx: 0.0, 
			mdy: 0.0,
			dscrollx: 0.0,
			dscrolly: 0.0,
			last_read: Instant::now(),
			last_updated: Instant::now(),
		}
	}
}
