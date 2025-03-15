use std::marker::PhantomData;
use arrayvec::ArrayVec;
use slotmap::SecondaryMap;
use smallvec::SmallVec;


// Functionality shared across backings 
// Has to be public or clippy will complain...
pub trait SignalMapBacking<T>: Clone {
	fn signal_insert(&mut self, v: T) -> bool;
	fn signal_remove(&mut self, v: T) -> bool;
	fn signal_empty(&self) -> bool;
	fn signal_slice(&self) -> &[T];
}


impl<T: PartialEq + Clone, const CAP: usize> SignalMapBacking<T> for SmallVec<[T; CAP]> {
	fn signal_insert(&mut self, v: T) -> bool {
		let contained = self.iter().position(|e| *e == v).is_some();
		if !contained {
			self.push(v);
		}
		contained
	}
	fn signal_remove(&mut self, v: T) -> bool {
		let index = self.iter().position(|e| *e == v);
		if let Some(i) = index {
			self.swap_remove(i);
		}
		index.is_some()
	}
	fn signal_empty(&self) -> bool {
		self.is_empty()
	}
	fn signal_slice(&self) -> &[T] {
		self.as_slice()
	}
}


impl<T: PartialEq + Clone, const CAP: usize> SignalMapBacking<T> for ArrayVec<T, CAP> {
	fn signal_insert(&mut self, v: T) -> bool {
		let contained = self.iter().position(|e| *e == v).is_some();
		if !contained {
			self.push(v);
		}
		contained
	}
	fn signal_remove(&mut self, v: T) -> bool {
		let index = self.iter().position(|e| *e == v);
		if let Some(i) = index {
			self.swap_remove(i);
		}
		index.is_some()
	}
	fn signal_empty(&self) -> bool {
		self.is_empty()
	}
	fn signal_slice(&self) -> &[T] {
		self.as_slice()
	}
}


struct SignalMapEntry<T, S: SignalMapBacking<T> + Default> {
	watches: S,
	watched_by: S,
	dirty: bool,
	pd: PhantomData<T>,
}
impl<T, S: SignalMapBacking<T> + Default> SignalMapEntry<T, S> {
	pub fn new() -> Self {
		Self {
			watches: S::default(),
			watched_by: S::default(),
			dirty: false,
			pd: PhantomData,
		}
	}
}


// The point is to provide notification when something cahnges 
// If nothing is watching an entry then it might as well not be there 
// EXCEPT that it can still be watching other things and we need that information
// So we can only remove something either explicitly or when it is neither watching nor being watched 
pub struct SignalMap<T: slotmap::Key, S: SignalMapBacking<T> + Default> {
	entries: SecondaryMap<T, SignalMapEntry<T, S>>,
}
impl<T: slotmap::Key, S: SignalMapBacking<T> + Default> SignalMap<T, S> {
	pub fn new() -> Self {
		Self {
			entries: SecondaryMap::new(),
		}
	}

	pub fn watch(&mut self, a: T, b: T) {
		// SecondaryMap lacks the entry api :(
		if !self.entries.contains_key(a) {
			self.entries.insert(a, SignalMapEntry::new());
		}
		let entry_a = self.entries.get_mut(a).unwrap();
		entry_a.watches.signal_insert(b);
		
		if !self.entries.contains_key(b) {
			self.entries.insert(b, SignalMapEntry::new());
		}
		let entry_b = self.entries.get_mut(b).unwrap();
		entry_b.watches.signal_insert(a);

		// a watches b 
		// add b to a's watches 
			// If not exist then do it anyway 
		// add a to b's watched_by 
			// If not exist then add entry for b? But then how to unload? Just remove if anver 
	}
	
	// Nodes without edges are pruned 
	pub fn unwatch(&mut self, a: T, b: T) {
		// If a exists, remove b from it 
		if let Some(e) = self.entries.get_mut(a) {
			e.watches.signal_remove(b);
			if e.watches.signal_empty() && e.watched_by.signal_empty() {
				self.entries.remove(a);
			}
			// Remove b from watches, maybe remove the whole entry 
		}
		if let Some(e) = self.entries.get_mut(b) {
			e.watches.signal_remove(a);
			if e.watches.signal_empty() && e.watched_by.signal_empty() {
				self.entries.remove(b);
			}
			// Remove a from watched_by, maybe remove the whole entry 
		}
	}
	
	// A more powerful form of unwatch, just deletes a node and connecting edges
	pub fn remove(&mut self, key: T) {
		if let Some(e) = self.entries.remove(key) {
			for w in e.watches.signal_slice() {
				self.entries[*w].watched_by.signal_remove(key);
			}
			for w in e.watched_by.signal_slice() {
				self.entries[*w].watches.signal_remove(key);
			}
		}
		// Remove entry 
		// Remove from what it watches 
			// Remove it from their watched_by 
	}
	
	/// Signal that some node has changed. 
	/// This will propagate to all nodes watching it. 
	/// 
	/// It would be useful to have a generation to track versions, but that can/should be external. 
	pub fn signal(&mut self, key: T) {
		if let Some(e) = self.entries.get_mut(key) {
			// Assumed to watch itself implicitly 
			e.dirty = true;
			// Data is cloned into a heap-allocated vec becuase of the borrow checker 
			// TODO: do it unsafely? 
			for k in e.watched_by.signal_slice().to_vec() {
				if let Some(e) = self.entries.get_mut(k) {
					e.dirty = true;
				}
			}
		}
	}
	
	pub fn signals<'s>(&'s mut self) -> SignalIterator<'s, T, S> {
		SignalIterator { entries: self.entries.iter_mut() }
	}
}


pub type ArraySignalMap<T, const CAP: usize> = SignalMap::<T, ArrayVec<T, CAP>>;
pub type SmallSignalMap<T, const CAP: usize> = SignalMap::<T, SmallVec<[T; CAP]>>;


pub struct SignalIterator<'a, T: slotmap::Key, S: SignalMapBacking<T> + Default> {
	entries: slotmap::secondary::IterMut<'a, T, SignalMapEntry<T, S>>,
}
impl<'a, T: slotmap::Key, S: SignalMapBacking<T> + Default> Iterator for SignalIterator<'a, T, S> {
	type Item = T;
	fn next(&mut self) -> Option<Self::Item> {
		// Fetch this and then mark it as not dirty becuase it was seen 
		// Position is retianed by the entries iterator 
		if let Some((k, e)) = self.entries.find(|(_, e)| e.dirty) {
			e.dirty = false;
			Some(k)
		} else {
			None
		}
	}
}


#[cfg(test)]
pub mod tests {
	use slotmap::SlotMap;
	use super::*;

	// This does assume the ordering of keys
	#[test]
	fn test_noise_normalization() {
		let mut map = SlotMap::new();
		let a = map.insert("a");
		let b = map.insert("b");
		let c = map.insert("c");

		// let mut sigmap = SignalMap::<_, ArrayVec<_, 4>>::new();
		let mut sigmap = ArraySignalMap::<_, 4>::new();
		sigmap.watch(a, b);
		sigmap.watch(b, c);

		sigmap.signal(a);
		assert!([a].into_iter().eq(sigmap.signals()));

		// Should be empty now 
		assert_eq!(0, sigmap.signals().count());

		sigmap.signal(b);
		assert!([a, b].into_iter().eq(sigmap.signals()));
	}
}
