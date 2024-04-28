use std::{time::{Instant, Duration}, collections::{HashMap, HashSet}};

use arrayvec::ArrayVec;
use crossbeam_channel::{Receiver, Sender};
use enumflags2::{bitflags, BitFlags};
use slotmap::{new_key_type, SlotMap};
use ekstensions::prelude::*;
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::input::{KeyKey, InputEvent, ActiveState};



/// An upgraded* [InputEvent]. 
/// 
/// *key event => control event
#[derive(Debug, Clone, Copy)]
pub enum ControlEvent {
	KeyEvent((ControlKey, ActiveState)),
	CursorMoved([f64; 2]),
	MouseMotion([f64; 2]),
	Scroll([f32; 2]),
}


#[derive(Debug, Component)]
pub struct ControlComponent {
	pub control_sequence: Vec<(ControlEvent, Instant)>,
	start: Instant,
	end: Instant,
	
	// State
	pressed: HashSet<KeyKey>,
	modifiers: BitFlags<KeyModifiers>,
	active: HashMap<ControlKey, Instant>, // Slotmap has no drain_filter() >:(
}
impl ControlComponent {
	pub fn new() -> Self {
		Self {
			control_sequence: Vec::new(),
			start: Instant::now(),
			end: Instant::now(),
			pressed: HashSet::new(),
			modifiers: KeyModifiers::EMPTY,
			active: HashMap::new(),
		}
	}

	pub fn last_tick_mouse_movement(&self) -> [f64; 2] {
		self.control_sequence.iter().filter_map(|(event, _)| match event {
			ControlEvent::MouseMotion(delta) => Some(delta),
			_ => None,
		}).fold([0.0; 2], |[ax, ay], &[vx, vy]| [ax+vx, ay+vy])
	}

	/// For how long a control was pressed since the last tick. 
	pub fn last_tick_duration(&self, control: ControlKey) -> Option<Duration> {
		let mut d = None;
		let mut st = self.active.contains_key(&control).then(|| self.start);
		for &(event, when) in self.control_sequence.iter() {
			if let ControlEvent::KeyEvent((next_control, state)) = event {
				if next_control == control {
					match state {
						ActiveState::Active => if st.is_none() {
							st = Some(when);
						} else {
							warn!("Control activated while still active");
						},
						ActiveState::Inactive => if let Some(start) = st {
							if let Some(d) = d.as_mut() {
								*d += when.duration_since(start);
							} else {
								d = Some(when.duration_since(start));
							}
							st = None;
						} else {
							warn!("Control released without being active");
						},
					}
				}
			}
		}
		if let Some(start) = st {
			if let Some(d) = d.as_mut() {
				*d += self.end.duration_since(start);
			} else {
				d = Some(self.end.duration_since(start));
			}
		}

		d
	}

	/// How many times a control was predded in the last tick. 
	pub fn last_tick_presses(&self, control: ControlKey) -> u32 {
		let mut n = 0;
		for &(event, _) in self.control_sequence.iter() {
			if let ControlEvent::KeyEvent((next_control, state)) = event {
				if next_control == control {
					if state == ActiveState::Active {
						n += 1;
					}
				}
			}
		}
		n
	}

	/// Was the control pressed in the last tick? 
	pub fn last_tick_pressed(&self, control: ControlKey) -> bool {
		if self.active.contains_key(&control) {
			return true;
		}
		for &(event, _) in self.control_sequence.iter() {
			if let ControlEvent::KeyEvent((next_control, _)) = event {
				if next_control == control {
					return true
				}
			}
		}
		false
	}

	pub fn next_tick(&mut self, start: Instant) {
		self.control_sequence.clear();
		self.start = self.end;
		self.end = start;
	}

	fn check_active_controls(&mut self, when: Instant, map: &ControlMap) {
		fn state_has_combo(combo: &KeyCombo, pressed: &HashSet<KeyKey>, modifiers: BitFlags<KeyModifiers>) -> bool {
			modifiers == combo.modifiers && combo.keys.iter().all(|key| pressed.contains(key))
		}

		for (control, entry) in map.controls.iter() {
			let mut active = entry.combos.iter().any(|combo| state_has_combo(combo, &self.pressed, self.modifiers));
			// If not there, then try with default modifiers
			if !active && self.modifiers != KeyModifiers::EMPTY {
				active = entry.combos.iter().any(|combo| state_has_combo(combo, &self.pressed, KeyModifiers::EMPTY));
			}

			if !active && self.active.contains_key(&control) {
				trace!("End control '{}' ({control:?})", entry.name);
				// This control is no longer active
				self.active.remove(&control);
				self.control_sequence.push((ControlEvent::KeyEvent((control, ActiveState::Inactive)), when));
				
			} else if active && !self.active.contains_key(&control) {
				trace!("Activate control '{}' ({control:?})", entry.name);
				// This control should be active
				self.active.insert(control, when);
				self.control_sequence.push((ControlEvent::KeyEvent((control, ActiveState::Active)), when));
			}
		}
	}

	pub fn input(
		&mut self,
		event: InputEvent,
		when: Instant,
		map: &ControlMap, 
	) {
		match event {
			InputEvent::KeyEvent((key, state)) => {
				match state {
					ActiveState::Active => {
						if let Some(flags) = KeyModifiers::try_from_key(key) {
							self.modifiers |= flags;
						}
						self.pressed.insert(key);
					},
					ActiveState::Inactive => {
						if let Some(flags) = KeyModifiers::try_from_key(key) {
							self.modifiers ^= flags;
						}
						self.pressed.remove(&key);
					},
				}
				self.check_active_controls(when, map);
			},
			InputEvent::CursorMoved(position) => self.control_sequence.push((ControlEvent::CursorMoved(position), when)),
			InputEvent::MouseMotion(delta) => self.control_sequence.push((ControlEvent::MouseMotion(delta), when)),
			InputEvent::Scroll(delta) => self.control_sequence.push((ControlEvent::Scroll(delta), when)),
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
					PhysicalKey::Code(KeyCode::ShiftLeft) => Some(Self::LShift),
					PhysicalKey::Code(KeyCode::ControlLeft) => Some(Self::LCtrl),
					PhysicalKey::Code(KeyCode::AltLeft) => Some(Self::LAlt),
					PhysicalKey::Code(KeyCode::ShiftRight) => Some(Self::RShift),
					PhysicalKey::Code(KeyCode::ControlRight) => Some(Self::RCtrl),
					PhysicalKey::Code(KeyCode::AltRight) => Some(Self::RAlt),
					_ => None,
				}
			}
			_ => None,
		}
	}
}


/// A key combo is just one (1) key and some modifiers. 
/// It could be extended to have multiple keys and some modifiers. 
/// Right now, however, it will not be that way. 
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
	pub keys: ArrayVec<KeyKey, 2>, // 
	pub modifiers: BitFlags<KeyModifiers>,
}


new_key_type! { pub struct ControlKey; }


#[derive(Debug, Resource)]
struct ControlMapEntry {
	pub name: String,
	pub description: String,
	pub combos: ArrayVec<KeyCombo, 5>, // Each control can have 5 triggering key combinations
}


#[derive(Debug, Resource)]
pub struct ControlMap {
	name_cid_map: HashMap<String, ControlKey>,

	controls: SlotMap<ControlKey, ControlMapEntry>,
}
impl ControlMap {
	pub fn new() -> Self {
		Self {
			name_cid_map: HashMap::new(),
			controls: SlotMap::with_key(),
		}
	}

	pub fn new_control(&mut self, name: impl Into<String>, description: impl Into<String>) -> ControlKey {
		let name = name.into();
		let description = description.into();
		if let Some(&cid) = self.name_cid_map.get(&name) {
			warn!("Cid of name '{name}' already exists");
			return cid;
		}
		
		self.controls.insert_with_key(|key| {
			self.name_cid_map.insert(name.clone(), key);
			ControlMapEntry { name, description, combos: ArrayVec::new() }
		})
	}

	pub fn add_control_binding(&mut self, control: ControlKey, combo: KeyCombo) {
		if let Some(entry) = self.controls.get_mut(control) {
			entry.combos.try_push(combo).unwrap();
		} else {
			panic!("Cid does not exist!")
		}
	}
}


#[derive(Debug, Component)]
pub struct LocalInputComponent {
	receiver: Receiver<(InputEvent, Instant)>,
}
impl LocalInputComponent {
	pub fn new() -> (Self, Sender<(InputEvent, Instant)>) {
		let (sender, receiver) = crossbeam_channel::unbounded();
		(Self { receiver }, sender)
	}
}


#[profiling::function]
pub fn local_control_system(
	mut inputs: CompMut<LocalInputComponent>,
	mut controls: CompMut<ControlComponent>,
	map: Res<ControlMap>,
) {
	for (input, control) in (&mut inputs, &mut controls).iter() {
		let events = input.receiver.try_iter().collect::<Vec<_>>();
		control.next_tick(Instant::now());
		events.iter().for_each(|&(event, when)| control.input(event, when, &map));
	}
}
