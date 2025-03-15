use crossbeam_channel::{Receiver, TryRecvError};


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ThreadState {
	// We could add an Arc<State> to this but it'd be better to have another way to do that so that the function is marked as complete automatically 
	Working,
	Completed,
}


#[derive(Debug)]
pub struct PollThread<V> {
	state: ThreadState,
	receiver: Receiver<V>,
}
impl<V: Send + Sync + 'static> PollThread<V> {
	pub fn new(f: impl Fn() -> V + Send + 'static) -> Self {
		let (sender, receiver) = crossbeam_channel::bounded(1);
		rayon::spawn(move || sender.send(f()).unwrap());
		Self {
			state: ThreadState::Working,
			receiver,
		}
	}
	
	pub fn state(&self) -> ThreadState {
		self.state
	}
	
	// Values are never returned by a thing that takes self 
	// We could have a way to check for done-ness before retuning, but why? 
	pub fn poll(&mut self) -> Option<V> {
		match self.receiver.try_recv() {
			Ok(v) => {
				self.state = ThreadState::Completed;
				Some(v)
			},
			Err(e) => {
				match e {
					TryRecvError::Disconnected => {
						warn!("PollThread disconnected, marking as complete");
						self.state = ThreadState::Completed
					},
					TryRecvError::Empty => {},
				}
				None
			},
		}
	}
}
