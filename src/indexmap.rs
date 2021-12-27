use std::collections::HashMap;
use std::hash::Hash;



/*
These are crude little things which I made to access things by index or key
*/



#[derive(Debug)]
pub struct IndexMap<T> {
	pub data: Vec<T>,
	pub data_index: HashMap<String, usize>,
}
impl<T> IndexMap<T> {
	pub fn new() -> Self {
		let data = Vec::new();
		let data_index = HashMap::new();
		Self {
			data,
			data_index,
		}
	}

	pub fn insert(&mut self, name: &String, data: T) -> usize {
		// If name exists, update
		if self.data_index.contains_key(name) {
			let idx = self.data_index[name];
			self.data[idx] = data;
			return idx
		}
		// Else load
		let idx = self.data.len();
		self.data_index.insert(name.clone(), idx);
		self.data.push(data);
		idx
	}

	// Get resource by name
	pub fn get_name(&self, name: &String) -> &T {
		let idx = self.data_index[name];
		self.get_index(idx)
	}

	// Get resource by index
	pub fn get_index(&self, idx: usize) -> &T {
		&self.data[idx]
	}
}



#[derive(Debug)]
pub struct SonOfIndexMap<K, V> {
	pub data: Vec<V>,
	pub index_map: HashMap<K, usize>,
}
impl<K: Hash + Eq + Clone, V> SonOfIndexMap<K, V> {

	pub fn new() -> Self {
		let data = Vec::new();
		let index_map = HashMap::new();
		Self {
			data,
			index_map,
		}
	}

	pub fn insert(&mut self, name: &K, data: V) -> usize {
		if self.index_map.contains_key(name) {
			// If name exists, update
			let idx = self.index_map[name];
			self.data[idx] = data;
			idx
		} else {
			// Else load
			let idx = self.data.len();
			self.index_map.insert(name.clone(), idx);
			self.data.push(data);
			idx
		}
	}

	// Tests if a key has been insterted
	pub fn contains_key(&self, key: &K) -> bool {
		self.index_map.contains_key(key)
	}

	// Get value by key
	pub fn key(&self, name: &K) -> &V {
		let idx = self.index_of(name);
		&self.data[idx]
	}

	// Get mutable value by key
	pub fn key_mut(&self, name: &K) -> &V {
		let idx = self.index_of(name);
		&self.data[idx]
	}

	// Get value by index
	pub fn index(&self, index: usize) -> &V {
		&self.data[index]
	}

	// Get mutable value by index
	pub fn index_mut(&mut self, index: usize) -> &mut V {
		&mut self.data[index]
	}

	// Get index of value
	pub fn index_of(&self, name: &K) -> usize {
		self.index_map[name]
	}
}