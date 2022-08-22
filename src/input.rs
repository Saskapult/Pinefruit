use winit::event::*;
use std::time::{Instant, Duration};
use std::collections::{HashMap, HashSet};
use enumflags2::{bitflags, BitFlags};


#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
	KeyEvent((KeyKey, KeyState)),
	ReleaseKeys,
	CursorMoved([f64; 2]),
	MouseMotion([f64; 2]),
	Scroll([f32; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
	Pressed,
	Released,
}
impl From<ElementState> for KeyState {
	fn from(state: ElementState) -> Self {
		match state {
			ElementState::Pressed => Self::Pressed,
			ElementState::Released => Self::Released,
		}
	}
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum KeyKey {
	BoardKey(VirtualKeyCode),
	MouseKey(MouseButton),
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



/// Its only job is to produce InputSegment.
#[derive(Debug)]
pub struct InputFilter {
	pub input_segment: InputSegment,
	pressed: HashSet<KeyKey>,
}
impl InputFilter {
	pub fn new() -> Self {
		Self {
			input_segment: InputSegment::new(),
			pressed: HashSet::new(),
		}
	}
	pub fn start(&mut self) {
		let now = Instant::now();
		self.input_segment.start = now;
		self.input_segment.event_record.clear();
		self.input_segment.event_record.extend(
			self.pressed.iter().map(|&k| 
				(InputEvent::KeyEvent((k, KeyState::Pressed)), now)
			)
		);
	}
	// pub fn set_initial_keys(&mut self, initial_keys: impl Into<HashSet<KeyKey>>) {
	// 	self.input_segment.initial_keys = initial_keys.into();
	// }
	pub fn event(&mut self, event: (InputEvent, Instant)) {
		let (input, when) = event;
		match input {
			InputEvent::KeyEvent((key, state)) => {
				match state {
					KeyState::Pressed => { self.pressed.insert(key); },
					KeyState::Released => { self.pressed.remove(&key); },
				}
			},
			InputEvent::ReleaseKeys => {
				self.pressed.drain().for_each(|key| {
					self.input_segment.event_record.push((
						InputEvent::KeyEvent((key, KeyState::Released)),
						when,
					));
				});
				return;
			}
			_ => {},
		};
		self.input_segment.event_record.push(event);
	}
	pub fn finish(&mut self) {
		self.input_segment.end = Instant::now();
	}
}



#[derive(Debug)]
pub struct MouseData {
	pub positions: Vec<[f64; 2]>,
	pub movement: Vec<[f64; 2]>,
	pub total_movement: [f64; 2],
	pub total_scroll: [f32; 2],
}
impl MouseData {
	pub fn new() -> Self {
		Self {
			positions: Vec::new(),
			movement: Vec::new(),
			total_movement: [0.0; 2],
			total_scroll: [0.0; 2],
		}
	}

	pub fn apply_segment(&mut self, segment: &InputSegment) {
		for (event, _) in segment.event_record.iter() {
			match event {
				&InputEvent::CursorMoved(new_position) => {
					self.positions.push(new_position);
				},
				&InputEvent::MouseMotion(motion) => {
					self.total_movement[0] += motion[0];
					self.total_movement[1] += motion[1];
					self.movement.push(motion);
				},
				&InputEvent::Scroll(scroll) => {
					self.total_scroll[0] += scroll[0];
					self.total_scroll[1] += scroll[1];
				},
				_ => {},
			};
		}		
	}

	pub fn clear(&mut self) {
		self.positions.clear();
		self.movement.clear();
		self.total_movement = [0.0; 2];
		self.total_scroll = [0.0; 2];
	}
}



#[derive(Debug)]
pub enum InputState {
	Ongoing,	// Still pressed at the end of its segment
	Complete,	// Released within the segment
}



// Intended to be kept across iterations
#[derive(Debug)]
pub struct KeyData {
	pub key_presses: HashMap<KeyKey, Vec<(Instant, Duration, InputState)>>,
	pub key_submits: HashMap<KeyKey, u32>, // Only works for keyboard keys because they resubmit
	pressed: HashMap<KeyKey, Instant>,
}
impl KeyData {
	pub fn new() -> Self {
		Self {
			key_presses: HashMap::new(),
			key_submits: HashMap::new(),
			pressed: HashMap::new(),
		}
	}

	pub fn clear(&mut self) {
		self.key_presses.clear();
		self.key_submits.clear();
	}

	pub fn key_duration(&self, key: KeyKey)-> Option<Duration> {
		self.key_presses.get(&key).and_then(|v| 
			v.iter()
				.map(|&(_, d, _)| d)
				.reduce(|a, v| a + v)
		)
	}

	pub fn apply_segment(&mut self, segment: &InputSegment) {
		for &(event, when) in segment.event_record.iter() {
			match event {
				InputEvent::KeyEvent((key, state)) => {
					match state {
						KeyState::Pressed => {
							if !self.pressed.contains_key(&key) {
								self.pressed.insert(key, when);
							}
							if let Some(count) = self.key_submits.get_mut(&key) {
								*count += 1;
							} else {
								self.key_submits.insert(key, 1);
							}
						},
						KeyState::Released => {
							if let Some(pressed_at) = self.pressed.remove(&key) {
								let pressed_for = when - pressed_at;
								let entry = (pressed_at, pressed_for, InputState::Complete);
								if let Some(ds) = self.key_presses.get_mut(&key) {
									ds.push(entry);
								} else {
									self.key_presses.insert(key, vec![entry]);
								}
							}
						},
					}
				},
				_ => {},
			};
		}
		for (&key, &pressed_at) in self.pressed.iter() {
			let pressed_for = segment.end - pressed_at;
			let entry = (pressed_at, pressed_for, InputState::Ongoing);
			if let Some(ds) = self.key_presses.get_mut(&key) {
				ds.push(entry);
			} else {
				self.key_presses.insert(key, vec![entry]);
			}
		}
	}
}



#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyModifiers {
	LShift,
	LCtrl,
	LAlt,
	RShift,
	RCtrl,
	RAlt,
}
impl KeyModifiers {
	pub const EMPTY: BitFlags<Self> = BitFlags::EMPTY;

	pub fn try_from_key(key: KeyKey) -> Option<Self> {
		match key {
			KeyKey::BoardKey(key) => {
				match key {
					VirtualKeyCode::LShift => Some(Self::LShift),
					VirtualKeyCode::LControl => Some(Self::LCtrl),
					VirtualKeyCode::LAlt => Some(Self::LAlt),
					VirtualKeyCode::RShift => Some(Self::RShift),
					VirtualKeyCode::RControl => Some(Self::RCtrl),
					VirtualKeyCode::RAlt => Some(Self::RAlt),
					_ => None,
				}
			}
			_ => None,
		}
	}
}



#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo {
	pub modifiers: BitFlags<KeyModifiers>,
	pub key: KeyKey,
}
impl KeyCombo {
	pub fn dummy() -> Self {
		Self {
			modifiers: KeyModifiers::LShift.into(),
			key: KeyKey::BoardKey(VirtualKeyCode::A),
		}
	}
}



const MAX_CID_COMBOS: usize = 5; // Maximum number of combos bound to one cid
pub type ControlId = u32;
pub struct ControlMap {
	name_cid_map: HashMap<String, ControlId>,
	pub cid_name_map: HashMap<ControlId, String>,
	cid_description_map: HashMap<ControlId, String>,

	pub cid_key_map: HashMap<ControlId, (u8, [KeyCombo; MAX_CID_COMBOS])>,
	pub key_cid_map: HashMap<KeyCombo, ControlId>,
	
	cid_count: u32,
}
impl ControlMap {
	pub fn new() -> Self {
		Self {
			name_cid_map: HashMap::new(),
			cid_name_map: HashMap::new(),
			cid_description_map: HashMap::new(),
			cid_key_map: HashMap::new(),
			key_cid_map: HashMap::new(),
			cid_count: 0,
		}
	}

	pub fn new_cid(&mut self, name: impl Into<String>, description: impl Into<String>) -> ControlId {
		let name = name.into();
		let description = description.into();
		if let Some(&cid) = self.name_cid_map.get(&name) {
			warn!("Cid already exists: {cid} - '{name}' - '{description}'");
			return cid;
		}
		
		let cid = self.cid_count;
		self.cid_count += 1;
		self.cid_key_map.insert(cid, (0, [KeyCombo::dummy(); MAX_CID_COMBOS]));

		self.name_cid_map.insert(name.clone(), cid);
		self.cid_name_map.insert(cid, name);
		self.cid_description_map.insert(cid, description);

		cid
	}

	pub fn add_cid_key(&mut self, cid: ControlId, key: KeyCombo) {
		if let Some((count, cid_keys)) = self.cid_key_map.get_mut(&cid) {
			if *count as usize + 1 >= MAX_CID_COMBOS {
				panic!("Too many keys to bind to this cid!");
			}
			
			cid_keys[*count as usize] = key;
			*count += 1;

			if !self.key_cid_map.contains_key(&key) {
				self.key_cid_map.insert(key, cid);
			}
		} else {
			panic!("Cid does not exist!")
		}
	}
}



pub struct ControlData {
	pub control_presses: HashMap<ControlId, Vec<(Instant, Duration, InputState)>>,
	active_combos: HashMap<KeyCombo, Instant>,
	modifiers: BitFlags<KeyModifiers>,
}
impl ControlData {
	pub fn new() -> Self {
		Self {
			control_presses: HashMap::new(),
			active_combos: HashMap::new(),
			modifiers: KeyModifiers::EMPTY,
		}
	}

	pub fn clear(&mut self) {
		self.control_presses.clear();
	}

	pub fn apply_segment(
		&mut self,
		segment: &InputSegment,
		map: &ControlMap, 
	) {
		for &(event, when) in segment.event_record.iter() {
			match event {
				InputEvent::KeyEvent((key, state)) => {
					match state {
						KeyState::Pressed => {
							if let Some(flags) = KeyModifiers::try_from_key(key) {
								
								// Finish all combos using old modifers
								self.active_combos.drain_filter(|&k, _| {
									k.modifiers == self.modifiers
								})
								.for_each(|(combo, started)| {
									let cid = map.key_cid_map.get(&combo).unwrap();
									let pressed_for = when - started;
									let entry = (started, pressed_for, InputState::Complete);
									// Record duration
									if let Some(g) = self.control_presses.get_mut(cid) {
										g.push(entry);
									} else {
										self.control_presses.insert(*cid, vec![entry]);
									}
								});

								self.modifiers = self.modifiers | flags;
							}

							// See if any combos can be started
							let combo = KeyCombo {
								modifiers: self.modifiers,
								key,
							};
							if map.key_cid_map.contains_key(&combo) {
								self.active_combos.insert(combo, when);
							} else if self.modifiers != KeyModifiers::EMPTY {
								let combo = KeyCombo {
									modifiers: KeyModifiers::EMPTY,
									key,
								};
								if map.key_cid_map.contains_key(&combo) {
									println!("st combo {combo:?}");
									self.active_combos.insert(combo, when);
								}
							}
						},
						KeyState::Released => {
							if let Some(flags) = KeyModifiers::try_from_key(key) {
								// Finish all combos using old modifers
								self.active_combos.drain_filter(|&k, _| {
									k.modifiers == self.modifiers
								})
								.for_each(|(combo, started)| {
									let cid = map.key_cid_map.get(&combo).unwrap();
									let pressed_for = when - started;
									let entry = (started, pressed_for, InputState::Complete);
									// Record duration
									if let Some(g) = self.control_presses.get_mut(cid) {
										g.push(entry);
									} else {
										self.control_presses.insert(*cid, vec![entry]);
									}
								});
								
								self.modifiers.remove(flags);
							}

							// Finish all combos using this key
							self.active_combos.drain_filter(|&k, _| {
								k.key == key
							})
							.for_each(|(combo, started)| {
								let cid = map.key_cid_map.get(&combo).unwrap();
								let pressed_for = when - started;
								let entry = (started, pressed_for, InputState::Complete);
								// Record duration
								if let Some(g) = self.control_presses.get_mut(cid) {
									g.push(entry);
								} else {
									self.control_presses.insert(*cid, vec![entry]);
								}
							});
						},
					}
				},
				_ => continue,
			}
		}
		for (&combo, &pressed_at) in self.active_combos.iter() {
			let pressed_for = segment.end - pressed_at;
			let entry = (pressed_at, pressed_for, InputState::Ongoing);
			let cid = map.key_cid_map.get(&combo).unwrap();
			if let Some(g) = self.control_presses.get_mut(cid) {
				g.push(entry);
			} else {
				self.control_presses.insert(*cid, vec![entry]);
			}
		}
	}

	pub fn control_duration(&self, cid: ControlId)-> Option<Duration> {
		self.control_presses.get(&cid).and_then(|v| 
			v.iter()
				.map(|&(_, d, _)| d)
				.reduce(|a, v| a + v)
		)
	}
}
