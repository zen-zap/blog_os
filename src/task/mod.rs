// in src/task/mod.rs

pub mod simple_executor;

use alloc::boxed::Box;
use core::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

pub struct Task {
	future: Pin<Box<dyn Future<Output = ()>>>,
	// methods on the Future are dynamically dispatched
}

impl Task {
	/// Pin<Box> type ensures that the value can never be moved in memory
	///
	/// It also prevents the creation of &mut references to it
	///
	/// The static lifetime is required because
	/// the Future can live for an arbitrary amount of time.
	pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
		Task { future: Box::pin(future) }
	}

	fn poll(
		&mut self,
		context: &mut Context,
	) -> Poll<()> {
		let _mutable_pinned_task_future = self.future.as_mut();
		self.future.as_mut().poll(context)
	}
}
