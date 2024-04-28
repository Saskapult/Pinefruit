use winit::event::*;
use winit::keyboard::PhysicalKey;
use std::time::Instant;
use std::collections::HashSet;



/// The game should take a sequence of these along with when they happened. 
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
	KeyEvent((KeyKey, ActiveState)),
	CursorMoved([f64; 2]),
	MouseMotion([f64; 2]),
	Scroll([f32; 2]),
}
impl<K: Into<KeyKey>, S: Into<ActiveState>> From<(K, S)> for InputEvent {
	fn from((k, s): (K, S)) -> Self {
		Self::KeyEvent((k.into(), s.into()))
	}
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveState {
	Active,
	Inactive,
}
impl From<ElementState> for ActiveState {
	fn from(state: ElementState) -> Self {
		match state {
			ElementState::Pressed => Self::Active,
			ElementState::Released => Self::Inactive,
		}
	}
}


/// Board or mouse key. 
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum KeyKey {
	BoardKey(PhysicalKey),
	MouseKey(MouseButton),
}
impl Into<KeyKey> for PhysicalKey {
	fn into(self) -> KeyKey {
		KeyKey::BoardKey(self)
	}
}
impl Into<KeyKey> for MouseButton {
	fn into(self) -> KeyKey {
		KeyKey::MouseKey(self)
	}
}


/// Input but includes initial keys as being pressed at the beginning.
/// Not filtered, can have resubmitted keys.
#[derive(Debug, Clone)]
pub struct InputSegment {
	pub event_record: Vec<(InputEvent, Instant)>,
	pub start: Instant,
	pub end: Instant,
}
impl InputSegment {
	pub fn new() -> Self {
		Self {
			event_record: Vec::new(),
			start: Instant::now(),
			end: Instant::now(),
		}
	}
}


/// Deduplicates key down/up events. 
#[derive(Debug)]
pub struct KeyDeduplicator {
	pressed: HashSet<KeyKey>,
}
impl KeyDeduplicator {
	pub fn new() -> Self {
		Self { pressed: HashSet::new() }
	}

	pub fn event(&mut self, key: impl Into<KeyKey>, state: ActiveState) -> Option<KeyKey> {
		let key = key.into();
		match state {
			ActiveState::Active => if self.pressed.contains(&key) {
				None
			} else {
				self.pressed.insert(key);
				Some(key)
			},
			ActiveState::Inactive => if self.pressed.contains(&key) {
				self.pressed.remove(&key);
				Some(key)
			} else {
				None
			},
		}
	}

	pub fn clear(&mut self) -> impl Iterator<Item = KeyKey> + '_ {
		self.pressed.drain()
	}

	pub fn pressed(&self) -> impl Iterator<Item = KeyKey> + '_ {
		self.pressed.iter().copied()
	}
}
