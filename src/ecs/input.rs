use crate::input::*;
use shipyard::*;




#[derive(Component, Debug)]
pub struct InputComponent {
	pub input: InputSegment,
}
impl InputComponent {
	pub fn new() -> Self {
		Self {
			input: InputSegment::new(),
		}
	}
}



#[derive(Component, Debug)]
pub struct MouseComponent {
	pub data: MouseData,
}
impl MouseComponent {
	pub fn new() -> Self {
		Self {
			data: MouseData::new(),
		}
	}
}



pub fn input_mouse_system(
	inputs: View<InputComponent>, 
	mut mice: ViewMut<MouseComponent>,
) {
	
	for (input, mouse) in (&inputs, &mut mice).iter() {
		mouse.data.clear();
		mouse.data.apply_segment(
			&input.input,
		);
	}
}



#[derive(Component)]
pub struct KeysComponent {
	pub data: KeyData,
}
impl KeysComponent {
	pub fn new() -> Self {
		Self {
			data: KeyData::new(),
		}
	}
}



pub fn input_keys_system(
	inputs: View<InputComponent>, 
	mut keys: ViewMut<KeysComponent>,
) {
	for (input, key) in (&inputs, &mut keys).iter() {
		key.data.clear();
		key.data.apply_segment(
			&input.input,
		);
	}
}



#[derive(Component)]
pub struct ControlComponent {
	pub map: ControlMap,
	pub data: ControlData,
}
impl ControlComponent {
	pub fn from_map(map: ControlMap) -> Self {
		Self {
			map,
			data: ControlData::new(),
		}
	}
}



pub fn input_control_system(
	inputs: View<InputComponent>, 
	mut controls: ViewMut<ControlComponent>,
) {
	for (input, control) in (&inputs, &mut controls).iter() {
		control.data.clear();
		control.data.apply_segment(
			&input.input,
			&control.map,
		);
		for cid in control.data.control_presses.keys() {
			let name = control.map.cid_name_map.get(&cid).unwrap();
			println!("\t{cid} - {name}");
		}
		// println!("-");
	}
}


/// Run input harvesters
pub fn input_workload() -> Workload {
	(
		input_mouse_system, 
		input_keys_system, 
		input_control_system,
	).into_workload()
}
